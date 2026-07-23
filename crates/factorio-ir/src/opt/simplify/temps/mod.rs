mod analysis;
mod rewrite;

use crate::{expression::Expression, statement::Statement};

use analysis::{
    count_reads, count_straight_line_reads, is_cheap_to_rematerialize, is_compiler_temp, is_pure,
    is_written,
};
use rewrite::replace_ident_in_statement;

/// Fold `local __tmp = <pure|single-use>; ...` away for compiler temps.
pub(super) fn eliminate_copy_temps(mut stmts: Vec<Statement>) -> Vec<Statement> {
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

/// `local __t = v; ...writes/reads __t...; local x = __t;` (no further `__t` uses)
/// -> rename `__t` to `x` and drop the trailing copy.
///
/// Needed after unwrap_or simplify, which rewrites onto a hoist temp and leaves
/// `local boots = __h_N`.
fn fold_temp_into_single_copy(mut stmts: Vec<Statement>) -> Vec<Statement> {
    let mut i = 0;
    while i < stmts.len() {
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
