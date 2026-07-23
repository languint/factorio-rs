mod analysis;
mod rewrite;

use crate::statement::Statement;

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
    stmts
}
