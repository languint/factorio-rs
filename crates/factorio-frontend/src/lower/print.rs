#[cfg(feature = "tracing")]
use syn::Item;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, ExprMacro, LitStr, Path, Stmt};

use crate::error::{FrontendError, FrontendResult};

use super::{
    context::LowerContext, expressions::lower_expression, serde_json::reject_serde_json_macro,
    util::location,
};

struct FormatMacroInput {
    format: LitStr,
    args: Vec<Expr>,
}

impl Parse for FormatMacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let format = input.parse::<LitStr>()?;
        let mut args = Vec::new();

        while input.parse::<syn::Token![,]>().is_ok() {
            args.push(input.parse::<Expr>()?);
        }

        Ok(Self { format, args })
    }
}

pub fn lower_macro_expression(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    if let Some(error) = reject_serde_json_macro(mac) {
        return Err(error);
    }

    #[cfg(feature = "tracing")]
    if let Some(level) = tracing_level_from_path(&mac.mac.path) {
        return lower_tracing_macro(mac, ctx, self_type, level);
    }

    let name = macro_name(&mac.mac.path);

    match name.as_str() {
        "println" => lower_println_macro(mac, ctx, self_type),
        "format" | "format_args" => lower_format_macro(mac, ctx, self_type),
        "matches" => super::statements::lower_matches_macro(mac, ctx, self_type),
        "assert" | "assert_eq" | "assert_ne" | "panic" => {
            let stmts = super::assert_macros::lower_assert_macro_statements(mac, ctx, self_type)?;
            ctx.try_hoists.extend(stmts);
            Ok(factorio_ir::expression::Expression::Literal(
                factorio_ir::literal::Literal::Nil,
            ))
        }
        _ => Err(FrontendError::UnsupportedMacro {
            name,
            location: location(mac),
        }),
    }
}

/// Lower rustc-expanded `println!` / `format!` forms (`_print(format_args!(...))`, etc.).
pub fn try_lower_expanded_std_format_call(
    call: &syn::ExprCall,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> Option<FrontendResult<factorio_ir::expression::Expression>> {
    let Expr::Path(func_path) = call.func.as_ref() else {
        return None;
    };
    let name = func_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())?;

    match name.as_str() {
        "_print" | "print" => {
            let arg = call.args.first()?;
            let mac = format_args_macro(arg)?;
            Some(
                lower_format_macro_message(mac, ctx, self_type)
                    .map(|message| game_print_call(message, None)),
            )
        }
        "format" => {
            let arg = call.args.first()?;
            let mac = format_args_macro(arg)?;
            Some(lower_format_macro_message(mac, ctx, self_type))
        }
        "must_use" => {
            let arg = call.args.first()?;
            let inner = peel_block_expr(arg)?;
            Some(lower_expression(inner, ctx, self_type))
        }
        _ => None,
    }
}

fn format_args_macro(expr: &Expr) -> Option<&ExprMacro> {
    let Expr::Macro(mac) = expr else {
        return None;
    };
    if macro_name(&mac.mac.path) == "format_args" {
        Some(mac)
    } else {
        None
    }
}

fn peel_block_expr(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Block(block) => {
            let mut stmts = block.block.stmts.iter();
            let stmt = stmts.next()?;
            if stmts.next().is_some() {
                return None;
            }
            match stmt {
                syn::Stmt::Expr(inner, _) => Some(inner),
                _ => None,
            }
        }
        other => Some(other),
    }
}

fn macro_name(path: &Path) -> String {
    path.get_ident().map_or_else(
        || {
            path.segments
                .last()
                .map_or_else(|| "macro".to_string(), |segment| segment.ident.to_string())
        },
        ToString::to_string,
    )
}

fn lower_println_macro(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let message = lower_format_macro_message(mac, ctx, self_type)?;
    Ok(game_print_call(message, None))
}

fn lower_format_macro(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    lower_format_macro_message(mac, ctx, self_type)
}

#[cfg(feature = "tracing")]
#[derive(Clone, Copy)]
enum TracingLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[cfg(feature = "tracing")]
impl TracingLevel {
    const fn label(self) -> &'static str {
        match self {
            Self::Error => "[ERROR] ",
            Self::Warn => "[WARN] ",
            Self::Info => "[INFO] ",
            Self::Debug => "[DEBUG] ",
            Self::Trace => "[TRACE] ",
        }
    }

    /// RGBA color for Factorio chat (`game.print` / `PrintSettings`).
    const fn color(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Error => (1.0, 0.25, 0.25, 1.0),
            Self::Warn => (1.0, 0.75, 0.2, 1.0),
            Self::Info => (0.55, 0.85, 1.0, 1.0),
            Self::Debug => (0.65, 0.65, 0.65, 1.0),
            Self::Trace => (0.45, 0.45, 0.45, 1.0),
        }
    }
}

#[cfg(feature = "tracing")]
fn tracing_level_from_path(path: &Path) -> Option<TracingLevel> {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();

    let level = match segments.as_slice() {
        [name] => name.as_str(),
        // `tracing::info!` macro path
        [.., mid, name] if mid == "tracing" => name.as_str(),
        // Expanded `::tracing::Level::INFO` (and `tracing_core::Level::INFO`)
        [.., mid, name] if mid == "Level" => name.as_str(),
        _ => return None,
    };

    match level {
        "error" | "ERROR" => Some(TracingLevel::Error),
        "warn" | "WARN" => Some(TracingLevel::Warn),
        "info" | "INFO" => Some(TracingLevel::Info),
        "debug" | "DEBUG" => Some(TracingLevel::Debug),
        "trace" | "TRACE" => Some(TracingLevel::Trace),
        _ => None,
    }
}

#[cfg(feature = "tracing")]
fn lower_tracing_macro(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
    level: TracingLevel,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let message = lower_format_macro_message(mac, ctx, self_type)?;
    let prefixed = prepend_literal(level.label(), message);
    Ok(game_print_call(
        prefixed,
        Some(print_settings_color(level.color())),
    ))
}

fn game_print_call(
    message: factorio_ir::expression::Expression,
    settings: Option<factorio_ir::expression::Expression>,
) -> factorio_ir::expression::Expression {
    let mut args = vec![message];
    if let Some(settings) = settings {
        args.push(settings);
    }
    factorio_ir::expression::Expression::Call {
        func: Box::new(factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(factorio_ir::expression::Expression::Identifier(
                "game".to_string(),
            )),
            field: "print".to_string(),
        }),
        args,
    }
}

#[cfg(feature = "tracing")]
fn prepend_literal(
    prefix: &str,
    message: factorio_ir::expression::Expression,
) -> factorio_ir::expression::Expression {
    match message {
        factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::String(
            value,
        )) => factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::String(
            format!("{prefix}{value}"),
        )),
        factorio_ir::expression::Expression::FormatConcat { mut parts } => {
            if let Some(factorio_ir::expression::Expression::Literal(
                factorio_ir::literal::Literal::String(value),
            )) = parts.first_mut()
            {
                *value = format!("{prefix}{value}");
            } else {
                parts.insert(
                    0,
                    factorio_ir::expression::Expression::Literal(
                        factorio_ir::literal::Literal::String(prefix.to_string()),
                    ),
                );
            }
            factorio_ir::expression::Expression::FormatConcat { parts }
        }
        other => factorio_ir::expression::Expression::FormatConcat {
            parts: vec![
                factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(prefix.to_string()),
                ),
                other,
            ],
        },
    }
}

#[cfg(feature = "tracing")]
fn print_settings_color((r, g, b, a): (f64, f64, f64, f64)) -> factorio_ir::expression::Expression {
    let color = factorio_ir::expression::Expression::StructLiteral {
        struct_name: Some("Color".to_string()),
        fields: vec![
            ("r".to_string(), float_lit(r)),
            ("g".to_string(), float_lit(g)),
            ("b".to_string(), float_lit(b)),
            ("a".to_string(), float_lit(a)),
        ],
    };
    factorio_ir::expression::Expression::StructLiteral {
        struct_name: Some("PrintSettings".to_string()),
        fields: vec![("color".to_string(), color)],
    }
}

#[cfg(feature = "tracing")]
const fn float_lit(value: f64) -> factorio_ir::expression::Expression {
    factorio_ir::expression::Expression::Literal(factorio_ir::literal::Literal::Float(value))
}

/// rustc-expanded `tracing::info!` / `error!` / ... becomes a block with inner `use`
/// + callsite statics. Recognize that shape and lower to colored `game.print`.
#[cfg(feature = "tracing")]
pub fn try_lower_expanded_tracing_event_block(
    stmts: &[Stmt],
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> Option<FrontendResult<Vec<factorio_ir::statement::Statement>>> {
    if !is_expanded_tracing_event_block(stmts) {
        return None;
    }
    Some(lower_expanded_tracing_event_block(stmts, ctx, self_type))
}

#[cfg(not(feature = "tracing"))]
pub const fn try_lower_expanded_tracing_event_block(
    _stmts: &[Stmt],
    _ctx: &mut LowerContext<'_>,
    _self_type: Option<&str>,
) -> Option<FrontendResult<Vec<factorio_ir::statement::Statement>>> {
    None
}

#[cfg(feature = "tracing")]
fn is_expanded_tracing_event_block(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        Stmt::Item(Item::Use(item_use)) => use_mentions_tracing_callsite(item_use),
        _ => false,
    })
}

#[cfg(feature = "tracing")]
fn walk_use_tree_for_tracing_callsite(
    tree: &syn::UseTree,
    path: &mut Vec<String>,
    found: &mut bool,
) {
    match tree {
        syn::UseTree::Path(p) => {
            path.push(p.ident.to_string());
            walk_use_tree_for_tracing_callsite(&p.tree, path, found);
            path.pop();
        }
        syn::UseTree::Name(n) => {
            path.push(n.ident.to_string());
            if path.iter().any(|s| s == "tracing" || s == "tracing_core")
                && path
                    .iter()
                    .any(|s| s == "Callsite" || s == "__macro_support")
            {
                *found = true;
            }
            path.pop();
        }
        syn::UseTree::Rename(r) => {
            path.push(r.ident.to_string());
            if path.iter().any(|s| s == "tracing" || s == "tracing_core")
                && (r.ident == "Callsite"
                    || path
                        .iter()
                        .any(|s| s == "Callsite" || s == "__macro_support"))
            {
                *found = true;
            }
            path.pop();
        }
        syn::UseTree::Glob(_) => {
            if path.iter().any(|s| s == "tracing" || s == "tracing_core") {
                *found = true;
            }
        }
        syn::UseTree::Group(g) => {
            for item in &g.items {
                walk_use_tree_for_tracing_callsite(item, path, found);
            }
        }
    }
}

#[cfg(feature = "tracing")]
fn use_mentions_tracing_callsite(item_use: &syn::ItemUse) -> bool {
    let mut path = Vec::new();
    let mut found = false;
    walk_use_tree_for_tracing_callsite(&item_use.tree, &mut path, &mut found);
    found
}

#[cfg(feature = "tracing")]
fn lower_expanded_tracing_event_block(
    stmts: &[Stmt],
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let level =
        find_tracing_level_in_stmts(stmts).ok_or_else(|| FrontendError::UnsupportedMacro {
            name: "tracing::event".to_string(),
            location: factorio_ir::span::SourceLoc::default()
                .with_note("expanded tracing event (could not find Level::*)"),
        })?;
    let mac =
        find_format_args_macro_in_stmts(stmts).ok_or_else(|| FrontendError::UnsupportedMacro {
            name: "tracing::event".to_string(),
            location: factorio_ir::span::SourceLoc::default()
                .with_note("expanded tracing event (could not find format_args! message)"),
        })?;
    let message = lower_format_macro_message(mac, ctx, self_type)?;
    let prefixed = prepend_literal(level.label(), message);
    Ok(vec![factorio_ir::statement::Statement::Expr(
        game_print_call(prefixed, Some(print_settings_color(level.color()))),
    )])
}

#[cfg(feature = "tracing")]
fn find_tracing_level_in_stmts(stmts: &[Stmt]) -> Option<TracingLevel> {
    for stmt in stmts {
        if let Some(level) = find_tracing_level_in_stmt(stmt) {
            return Some(level);
        }
    }
    None
}

#[cfg(feature = "tracing")]
fn find_tracing_level_in_stmt(stmt: &Stmt) -> Option<TracingLevel> {
    match stmt {
        Stmt::Local(local) => local
            .init
            .as_ref()
            .and_then(|init| find_tracing_level_in_expr(&init.expr)),
        Stmt::Item(Item::Static(item)) => find_tracing_level_in_expr(&item.expr),
        Stmt::Item(Item::Const(item)) => find_tracing_level_in_expr(&item.expr),
        Stmt::Expr(expr, _) => find_tracing_level_in_expr(expr),
        Stmt::Macro(_) | Stmt::Item(_) => None,
    }
}

#[cfg(feature = "tracing")]
fn find_tracing_level_in_expr(expr: &Expr) -> Option<TracingLevel> {
    match expr {
        Expr::Path(path) => tracing_level_from_path(&path.path),
        Expr::Call(call) => {
            if let Some(level) = find_tracing_level_in_expr(&call.func) {
                return Some(level);
            }
            call.args.iter().find_map(find_tracing_level_in_expr)
        }
        Expr::MethodCall(call) => {
            if let Some(level) = find_tracing_level_in_expr(&call.receiver) {
                return Some(level);
            }
            call.args.iter().find_map(find_tracing_level_in_expr)
        }
        Expr::Reference(reference) => find_tracing_level_in_expr(&reference.expr),
        Expr::Paren(paren) => find_tracing_level_in_expr(&paren.expr),
        Expr::Group(group) => find_tracing_level_in_expr(&group.expr),
        Expr::Block(block) => block
            .block
            .stmts
            .iter()
            .find_map(find_tracing_level_in_stmt),
        Expr::If(if_expr) => {
            if let Some(level) = find_tracing_level_in_expr(&if_expr.cond) {
                return Some(level);
            }
            if let Some(level) = if_expr
                .then_branch
                .stmts
                .iter()
                .find_map(find_tracing_level_in_stmt)
            {
                return Some(level);
            }
            match if_expr.else_branch.as_ref() {
                Some((_, else_expr)) => find_tracing_level_in_expr(else_expr),
                None => None,
            }
        }
        Expr::Binary(bin) => {
            find_tracing_level_in_expr(&bin.left).or_else(|| find_tracing_level_in_expr(&bin.right))
        }
        Expr::Unary(unary) => find_tracing_level_in_expr(&unary.expr),
        Expr::Field(field) => find_tracing_level_in_expr(&field.base),
        Expr::Tuple(tuple) => tuple.elems.iter().find_map(find_tracing_level_in_expr),
        Expr::Array(array) => array.elems.iter().find_map(find_tracing_level_in_expr),
        Expr::Repeat(repeat) => find_tracing_level_in_expr(&repeat.expr),
        Expr::Struct(s) => s
            .fields
            .iter()
            .find_map(|f| find_tracing_level_in_expr(&f.expr)),
        Expr::Closure(closure) => match closure.body.as_ref() {
            Expr::Block(block) => block
                .block
                .stmts
                .iter()
                .find_map(find_tracing_level_in_stmt),
            other => find_tracing_level_in_expr(other),
        },
        _ => None,
    }
}

#[cfg(feature = "tracing")]
fn find_format_args_macro_in_stmts(stmts: &[Stmt]) -> Option<&ExprMacro> {
    stmts.iter().find_map(find_format_args_macro_in_stmt)
}

#[cfg(feature = "tracing")]
fn find_format_args_macro_in_stmt(stmt: &Stmt) -> Option<&ExprMacro> {
    match stmt {
        Stmt::Local(local) => local
            .init
            .as_ref()
            .and_then(|init| find_format_args_macro_in_expr(&init.expr)),
        Stmt::Item(Item::Static(item)) => find_format_args_macro_in_expr(&item.expr),
        Stmt::Item(Item::Const(item)) => find_format_args_macro_in_expr(&item.expr),
        Stmt::Expr(expr, _) => find_format_args_macro_in_expr(expr),
        _ => None,
    }
}

#[cfg(feature = "tracing")]
fn find_format_args_macro_in_expr(expr: &Expr) -> Option<&ExprMacro> {
    match expr {
        Expr::Macro(mac) if macro_name(&mac.mac.path) == "format_args" => Some(mac),
        Expr::Call(call) => {
            if let Some(mac) = find_format_args_macro_in_expr(&call.func) {
                return Some(mac);
            }
            call.args.iter().find_map(find_format_args_macro_in_expr)
        }
        Expr::MethodCall(call) => {
            if let Some(mac) = find_format_args_macro_in_expr(&call.receiver) {
                return Some(mac);
            }
            call.args.iter().find_map(find_format_args_macro_in_expr)
        }
        Expr::Reference(reference) => find_format_args_macro_in_expr(&reference.expr),
        Expr::Paren(paren) => find_format_args_macro_in_expr(&paren.expr),
        Expr::Group(group) => find_format_args_macro_in_expr(&group.expr),
        Expr::Cast(cast) => find_format_args_macro_in_expr(&cast.expr),
        Expr::Block(block) => block
            .block
            .stmts
            .iter()
            .find_map(find_format_args_macro_in_stmt),
        Expr::If(if_expr) => if_expr
            .then_branch
            .stmts
            .iter()
            .find_map(find_format_args_macro_in_stmt)
            .or_else(|| {
                if_expr
                    .else_branch
                    .as_ref()
                    .and_then(|(_, else_expr)| find_format_args_macro_in_expr(else_expr))
            }),
        Expr::Binary(bin) => find_format_args_macro_in_expr(&bin.left)
            .or_else(|| find_format_args_macro_in_expr(&bin.right)),
        Expr::Unary(unary) => find_format_args_macro_in_expr(&unary.expr),
        Expr::Field(field) => find_format_args_macro_in_expr(&field.base),
        Expr::Tuple(tuple) => tuple.elems.iter().find_map(find_format_args_macro_in_expr),
        Expr::Array(array) => array.elems.iter().find_map(find_format_args_macro_in_expr),
        Expr::Struct(s) => s
            .fields
            .iter()
            .find_map(|f| find_format_args_macro_in_expr(&f.expr)),
        Expr::Closure(closure) => match closure.body.as_ref() {
            Expr::Block(block) => block
                .block
                .stmts
                .iter()
                .find_map(find_format_args_macro_in_stmt),
            other => find_format_args_macro_in_expr(other),
        },
        _ => None,
    }
}

fn lower_format_macro_message(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let input = syn::parse2::<FormatMacroInput>(mac.mac.tokens.clone()).map_err(|error| {
        FrontendError::Syn(format!(
            "unsupported format/tracing macro input at {}: {error} (format-string style only; structured fields are not supported)",
            location(mac)
        ))
    })?;
    let template = input.format.value();
    let lowered_args = input
        .args
        .iter()
        .map(|arg| lower_expression(arg, ctx, self_type))
        .collect::<FrontendResult<Vec<_>>>()?;
    lower_format_template(&template, &lowered_args, location(mac), &input.format, ctx)
}

pub fn lower_format_template(
    template: &str,
    args: &[factorio_ir::expression::Expression],
    location: factorio_ir::span::SourceLoc,
    format_lit: &LitStr,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let pieces = parse_format_pieces(template);
    let sequential_count = pieces
        .iter()
        .filter(|(piece, _)| matches!(piece, FormatPiece::PositionalArg { .. }))
        .count();

    if sequential_count > args.len() {
        return Err(FrontendError::FormatArgumentMismatch {
            template: template.to_string(),
            expected: sequential_count,
            found: args.len(),
            location,
        });
    }

    for (piece, _) in &pieces {
        if let FormatPiece::PositionalIndex { index, .. } = piece
            && *index >= args.len()
        {
            return Err(FrontendError::FormatArgumentMismatch {
                template: template.to_string(),
                expected: *index + 1,
                found: args.len(),
                location,
            });
        }
    }

    for (piece, placeholder_range) in &pieces {
        if let Some(spec) = piece.ignored_spec() {
            let loc = placeholder_range.as_ref().map_or_else(
                || location.clone(),
                |range| lit_str_subspan(format_lit, range.clone()),
            );
            ctx.emit_lint(
                factorio_ir::lint::LintId::FormatSpec,
                format!(
                    "format spec `:{spec}` is ignored when lowering (only `:?` / `:#?` are supported)"
                ),
                loc,
            )?;
        }
    }

    let mut parts = Vec::new();
    let mut sequential_index = 0;

    for (piece, _) in pieces {
        match piece {
            FormatPiece::Literal(value) => {
                parts.push(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(value),
                ));
            }
            FormatPiece::PositionalArg { debug, .. } => {
                let Some(arg) = args.get(sequential_index).cloned() else {
                    return Err(FrontendError::FormatArgumentMismatch {
                        template: template.to_string(),
                        expected: sequential_index + 1,
                        found: args.len(),
                        location,
                    });
                };
                parts.push(maybe_debug_format(arg, debug, ctx));
                sequential_index += 1;
            }
            FormatPiece::PositionalIndex { index, debug, .. } => {
                parts.push(maybe_debug_format(args[index].clone(), debug, ctx));
            }
            FormatPiece::NamedCapture { name, debug, .. } => {
                parts.push(maybe_debug_format(
                    factorio_ir::expression::Expression::Identifier(name),
                    debug,
                    ctx,
                ));
            }
        }
    }

    if parts.is_empty() {
        return Ok(factorio_ir::expression::Expression::Literal(
            factorio_ir::literal::Literal::String(String::new()),
        ));
    }

    if parts.len() == 1 {
        return Ok(parts.remove(0));
    }

    Ok(factorio_ir::expression::Expression::FormatConcat { parts })
}

/// Resolve a binding/expression type key for Debug format selection.
#[must_use]
pub fn infer_debug_type_key(
    value: &factorio_ir::expression::Expression,
    ctx: &LowerContext<'_>,
) -> Option<String> {
    match value {
        factorio_ir::expression::Expression::Identifier(name) => {
            ctx.binding_type(name).map(str::to_string)
        }
        factorio_ir::expression::Expression::FieldAccess { base, field } => {
            let base_key = infer_debug_type_key(base, ctx)?;
            factorio_api::debug_types::struct_field_type(&base_key, field).map(str::to_string)
        }
        factorio_ir::expression::Expression::StructLiteral {
            struct_name: Some(name),
            ..
        } => Some(name.clone()),
        factorio_ir::expression::Expression::EnumLiteral { enum_name, .. } => {
            Some(enum_name.clone())
        }
        factorio_ir::expression::Expression::FatPointer { data, .. } => {
            infer_debug_type_key(data, ctx)
        }
        factorio_ir::expression::Expression::If {
            then_expr,
            else_expr,
            ..
        } => {
            let then_key = infer_debug_type_key(then_expr, ctx)?;
            let else_key = infer_debug_type_key(else_expr, ctx)?;
            (then_key == else_key).then_some(then_key)
        }
        factorio_ir::expression::Expression::Call { func, args }
            if args.is_empty()
                && let factorio_ir::expression::Expression::Closure { body, .. } =
                    func.as_ref() =>
        {
            body.statements.iter().rev().find_map(|statement| {
                if let factorio_ir::statement::Statement::Return(Some(value)) = statement {
                    infer_debug_type_key(value, ctx)
                } else {
                    None
                }
            })
        }
        factorio_ir::expression::Expression::StructLiteral {
            struct_name: None, ..
        }
        | factorio_ir::expression::Expression::Array { .. } => Some("table".to_string()),
        factorio_ir::expression::Expression::Literal(literal) => match literal {
            factorio_ir::literal::Literal::String(_) => Some("string".to_string()),
            factorio_ir::literal::Literal::Int(_) | factorio_ir::literal::Literal::Float(_) => {
                Some("number".to_string())
            }
            factorio_ir::literal::Literal::Bool(_) => Some("boolean".to_string()),
            factorio_ir::literal::Literal::Nil => Some("nil".to_string()),
        },
        _ => None,
    }
}

/// `{:?}` / `{:#?}`: choose `helpers.table_to_json` or `tostring` from known types.
fn maybe_debug_format(
    value: factorio_ir::expression::Expression,
    debug: bool,
    ctx: &LowerContext<'_>,
) -> factorio_ir::expression::Expression {
    if !debug {
        return value;
    }

    if debug_uses_json(infer_debug_type_key(&value, ctx).as_deref()) {
        return factorio_ir::expression::Expression::MethodCall {
            receiver: Box::new(factorio_ir::expression::Expression::Identifier(
                "helpers".to_string(),
            )),
            method: "table_to_json".to_string(),
            args: vec![value],
        };
    }

    factorio_ir::expression::Expression::Call {
        func: Box::new(factorio_ir::expression::Expression::Identifier(
            "tostring".to_string(),
        )),
        args: vec![value],
    }
}

fn debug_uses_json(type_key: Option<&str>) -> bool {
    let Some(key) = type_key else {
        // Unknown -> `tostring` (safe for userdata; tables print poorly but won't crash).
        return false;
    };

    if key.starts_with("defines.") {
        return false;
    }

    match key {
        "table" => true,
        "string" | "number" | "boolean" | "nil" | "str" | "String" | "bool" | "char" | "LuaAny"
        | "LuaFunction" | "LuaStorage" | "Serpent" | "LocalisedString" | "i8" | "i16" | "i32"
        | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "f32"
        | "f64" | "uint8" | "uint16" | "uint32" | "uint64" | "uint" | "int8" | "int16"
        | "int32" | "int64" | "int" | "float" | "double" | "MapTick" | "Tick"
        | "ItemStackIndex" | "ItemCountType" => false,
        other if factorio_api::debug_types::is_userdata_class(other) => false,
        // Event data structs, concepts, and other plain Lua tables.
        _ => true,
    }
}

/// Map a byte range inside a format-string *value* onto the `LitStr` source span.
///
/// Assumes a normal `"..."` literal without escapes before/within the range (the
/// common case for `println!` / `format!` templates). Falls back to the whole
/// literal when the mapped range would be invalid.
fn lit_str_subspan(
    lit: &LitStr,
    value_range: std::ops::Range<usize>,
) -> factorio_ir::span::SourceLoc {
    let lit_range = lit.span().byte_range();
    // Content sits between the opening and closing `"`.
    let content_start = lit_range.start.saturating_add(1);
    let content_end = lit_range.end.saturating_sub(1);
    let start = content_start.saturating_add(value_range.start);
    let end = content_start.saturating_add(value_range.end);
    if start < end && end <= content_end && start >= content_start {
        return factorio_ir::span::SourceLoc::new(factorio_ir::span::SourceSpan::new(start, end));
    }
    location(lit)
}

enum FormatPiece {
    Literal(String),
    PositionalArg {
        debug: bool,
        ignored_spec: Option<String>,
    },
    PositionalIndex {
        index: usize,
        debug: bool,
        ignored_spec: Option<String>,
    },
    NamedCapture {
        name: String,
        debug: bool,
        ignored_spec: Option<String>,
    },
}

impl FormatPiece {
    fn ignored_spec(&self) -> Option<&str> {
        match self {
            Self::Literal(_) => None,
            Self::PositionalArg { ignored_spec, .. }
            | Self::PositionalIndex { ignored_spec, .. }
            | Self::NamedCapture { ignored_spec, .. } => ignored_spec.as_deref(),
        }
    }
}

/// Specs supported for Debug formatting; anything else is silently ignored today.
fn ignored_format_spec(spec: Option<&str>) -> Option<String> {
    match spec {
        None | Some("?" | "#?") => None,
        Some(other) => Some(other.to_string()),
    }
}

/// Parse format pieces, returning an optional byte range covering `{...}` when the
/// placeholder has an ignored format spec (for precise diagnostics).
fn parse_format_pieces(template: &str) -> Vec<(FormatPiece, Option<std::ops::Range<usize>>)> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut i = 0;

    while i < template.len() {
        let Some(ch) = template[i..].chars().next() else {
            break;
        };
        let ch_len = ch.len_utf8();
        match ch {
            '{' => {
                if template
                    .get(i + ch_len..)
                    .is_some_and(|rest| rest.starts_with('{'))
                {
                    literal.push('{');
                    i += ch_len + '{'.len_utf8();
                    continue;
                }

                if !literal.is_empty() {
                    pieces.push((FormatPiece::Literal(std::mem::take(&mut literal)), None));
                }

                let open = i;
                i += ch_len;
                let contents_start = i;
                let mut closed = false;
                while i < template.len() {
                    let Some(c) = template[i..].chars().next() else {
                        break;
                    };
                    if c == '}' {
                        let contents = &template[contents_start..i];
                        i += c.len_utf8();
                        let close_end = i;
                        let piece = parse_format_placeholder(contents);
                        let range = piece.ignored_spec().map(|_| open..close_end);
                        pieces.push((piece, range));
                        closed = true;
                        break;
                    }
                    i += c.len_utf8();
                }

                if !closed {
                    literal.push('{');
                    literal.push_str(&template[contents_start..]);
                    break;
                }
            }
            '}' => {
                if template
                    .get(i + ch_len..)
                    .is_some_and(|rest| rest.starts_with('}'))
                {
                    i += ch_len + '}'.len_utf8();
                } else {
                    i += ch_len;
                }
                literal.push('}');
            }
            other => {
                literal.push(other);
                i += ch_len;
            }
        }
    }

    if !literal.is_empty() {
        pieces.push((FormatPiece::Literal(literal), None));
    }

    pieces
}

fn parse_format_placeholder(contents: &str) -> FormatPiece {
    let (name, spec) = match contents.split_once(':') {
        Some((name, spec)) => (name, Some(spec)),
        None => (contents, None),
    };
    // `:?` / `:#?` -> JSON or tostring chosen at compile time.
    let debug = matches!(spec, Some("?" | "#?"));
    let ignored_spec = ignored_format_spec(spec);

    if name.is_empty() {
        return FormatPiece::PositionalArg {
            debug,
            ignored_spec,
        };
    }

    if let Ok(index) = name.parse::<usize>() {
        return FormatPiece::PositionalIndex {
            index,
            debug,
            ignored_spec,
        };
    }

    if is_format_ident(name) {
        return FormatPiece::NamedCapture {
            name: name.to_string(),
            debug,
            ignored_spec,
        };
    }

    FormatPiece::PositionalArg {
        debug,
        ignored_spec,
    }
}

fn is_format_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
