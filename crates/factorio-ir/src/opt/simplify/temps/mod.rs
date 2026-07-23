mod analysis;
mod rewrite;

use crate::{expression::Expression, literal::Literal, statement::Statement};

use analysis::{
    count_reads, count_straight_line_reads, is_cheap_to_rematerialize, is_compiler_temp, is_pure,
    is_written,
};
use rewrite::{replace_ident_in_expr, replace_ident_in_statement};

/// Fold `local __tmp = <pure|single-use>; ...` away for compiler temps.
pub(super) fn eliminate_copy_temps(mut stmts: Vec<Statement>) -> Vec<Statement> {
    stmts = fold_temp_init_overwrite(stmts);

    let mut i = 0;
    while i < stmts.len() {
        let Statement::VariableDecl { name, value, .. } = &stmts[i] else {
            i += 1;
            continue;
        };
        if !is_compiler_temp(name) {
            i += 1;
            continue;
        }

        let name = name.clone();
        let value = value.clone();
        let rest = &stmts[i + 1..];
        if is_written(&name, rest) {
            i += 1;
            continue;
        }

        let reads = count_reads(&name, rest);

        let can_subst = reads == 0
            || is_cheap_to_rematerialize(&value)
            || (reads == 1 && (is_pure(&value) || count_straight_line_reads(&name, rest) == 1));
        if !can_subst {
            i += 1;
            continue;
        }

        stmts.remove(i);
        if reads > 0 {
            for statement in &mut stmts[i..] {
                replace_ident_in_statement(statement, &name, &value);
            }
        }
        // Restart from i: a later temp may now be eligible, and indices shifted.
    }
    fold_temp_into_single_copy(stmts)
}

/// `local t = init; t = expr;` -> `local t = expr[t:=init]` for compiler temps.
///
/// Unlocks map/collect lowering (`__iter_value = item; __iter_value = f(__iter_value)`).
fn fold_temp_init_overwrite(mut stmts: Vec<Statement>) -> Vec<Statement> {
    let mut i = 0;
    while i + 1 < stmts.len() {
        let Statement::VariableDecl {
            name, value: init, ..
        } = &stmts[i]
        else {
            i += 1;
            continue;
        };
        if !is_compiler_temp(name) {
            i += 1;
            continue;
        }
        let Statement::Assignment {
            target: Expression::Identifier(dest),
            value: overwrite,
        } = &stmts[i + 1]
        else {
            i += 1;
            continue;
        };
        if dest != name {
            i += 1;
            continue;
        }

        let name = name.clone();
        let init = init.clone();
        let mut new_init = overwrite.clone();
        replace_ident_in_expr(&mut new_init, &name, &init);
        if let Statement::VariableDecl { value, .. } = &mut stmts[i] {
            *value = new_init;
        }
        stmts.remove(i + 1);
        // Stay at i: chained overwrites (`t = f(t); t = g(t)`) fold in subsequent passes.
    }
    stmts
}

/// Fold a compiler temp into its sole destination binding:
/// - `local __t = v; ...; local x = __t;`
/// - `local x = nil; local __t = v; ...; x = __t;` (IIFE collect after hoist)
fn fold_temp_into_single_copy(mut stmts: Vec<Statement>) -> Vec<Statement> {
    let mut i = 0;
    while i < stmts.len() {
        if try_fold_nil_then_temp_assign(&mut stmts, i) {
            continue;
        }

        let Statement::VariableDecl { name: tmp, .. } = &stmts[i] else {
            i += 1;
            continue;
        };
        if !is_compiler_temp(tmp) {
            i += 1;
            continue;
        }
        let tmp = tmp.clone();

        let Some((copy_idx, dest)) = find_final_ident_copy(&stmts, i, &tmp) else {
            i += 1;
            continue;
        };

        // `dest` must not already appear in the rename window (would clash).
        let window = &stmts[i..copy_idx];
        if count_reads(&dest, window) > 0 || is_written(&dest, window) {
            i += 1;
            continue;
        }

        if let Statement::VariableDecl { name, .. } = &mut stmts[i] {
            *name = dest.clone();
        }
        let dest_expr = Expression::Identifier(dest.clone());
        for statement in &mut stmts[i + 1..copy_idx] {
            replace_ident_in_statement(statement, &tmp, &dest_expr);
        }
        stmts.remove(copy_idx);
        // Retry at i: the renamed binding may unlock further folds.
    }
    stmts
}

/// `local dest = nil; local __tmp = …; …; dest = __tmp` → rename `__tmp` to `dest`.
fn try_fold_nil_then_temp_assign(stmts: &mut Vec<Statement>, nil_idx: usize) -> bool {
    let Statement::VariableDecl {
        name: dest,
        ty,
        source_type,
        value: Expression::Literal(Literal::Nil),
    } = &stmts[nil_idx]
    else {
        return false;
    };
    let dest = dest.clone();
    let ty = ty.clone();
    let source_type = source_type.clone();

    let Some(Statement::VariableDecl { name: tmp, .. }) = stmts.get(nil_idx + 1) else {
        return false;
    };
    if !is_compiler_temp(tmp) {
        return false;
    }
    let tmp = tmp.clone();
    let tmp_decl_idx = nil_idx + 1;

    let Some(assign_idx) = find_final_ident_assign(stmts, tmp_decl_idx, &tmp, &dest) else {
        return false;
    };

    let window = &stmts[tmp_decl_idx..assign_idx];
    if count_reads(&dest, window) > 0 || is_written(&dest, window) {
        return false;
    }

    if let Statement::VariableDecl {
        name,
        ty: tmp_ty,
        source_type: tmp_source,
        ..
    } = &mut stmts[tmp_decl_idx]
    {
        *name = dest.clone();
        *tmp_ty = ty;
        *tmp_source = source_type;
    }
    let dest_expr = Expression::Identifier(dest);
    for statement in &mut stmts[tmp_decl_idx + 1..assign_idx] {
        replace_ident_in_statement(statement, &tmp, &dest_expr);
    }
    stmts.remove(assign_idx);
    stmts.remove(nil_idx);
    true
}

/// Find `local dest = tmp` that is the last use of `tmp` after `tmp_decl_idx`.
fn find_final_ident_copy(
    stmts: &[Statement],
    tmp_decl_idx: usize,
    tmp: &str,
) -> Option<(usize, String)> {
    for (offset, statement) in stmts[tmp_decl_idx + 1..].iter().enumerate() {
        let Statement::VariableDecl {
            name: dest,
            value: Expression::Identifier(src),
            ..
        } = statement
        else {
            continue;
        };
        if src != tmp {
            continue;
        }
        let copy_idx = tmp_decl_idx + 1 + offset;
        let after = &stmts[copy_idx + 1..];
        if count_reads(tmp, after) == 0 && !is_written(tmp, after) {
            return Some((copy_idx, dest.clone()));
        }
        return None;
    }
    None
}

/// Find `dest = tmp` that is the last use of `tmp` after `tmp_decl_idx`.
fn find_final_ident_assign(
    stmts: &[Statement],
    tmp_decl_idx: usize,
    tmp: &str,
    dest: &str,
) -> Option<usize> {
    for (offset, statement) in stmts[tmp_decl_idx + 1..].iter().enumerate() {
        let Statement::Assignment {
            target: Expression::Identifier(assign_dest),
            value: Expression::Identifier(src),
        } = statement
        else {
            continue;
        };
        if src != tmp || assign_dest != dest {
            continue;
        }
        let assign_idx = tmp_decl_idx + 1 + offset;
        let after = &stmts[assign_idx + 1..];
        if count_reads(tmp, after) == 0 && !is_written(tmp, after) {
            return Some(assign_idx);
        }
        return None;
    }
    None
}
