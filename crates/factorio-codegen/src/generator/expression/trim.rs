use factorio_ir::expression::Expression;

/// Drop trailing `nil` literals from call/method argument lists.
pub fn trim_trailing_nils(args: &[Expression]) -> &[Expression] {
    let mut end = args.len();
    while end > 0 {
        match &args[end - 1] {
            Expression::Literal(factorio_ir::literal::Literal::Nil) => end -= 1,
            _ => break,
        }
    }
    &args[..end]
}
