use syn::parse::{Parse, ParseStream};
use syn::{Expr, ExprMacro, LitStr, Path};

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
        "format" => lower_format_macro(mac, ctx, self_type),
        _ => Err(FrontendError::UnsupportedMacro {
            name,
            location: location(mac),
        }),
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
        [.., mid, name] if mid == "tracing" => name.as_str(),
        _ => return None,
    };

    match level {
        "error" => Some(TracingLevel::Error),
        "warn" => Some(TracingLevel::Warn),
        "info" => Some(TracingLevel::Info),
        "debug" => Some(TracingLevel::Debug),
        "trace" => Some(TracingLevel::Trace),
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
    lower_format_template(&template, &lowered_args, location(mac))
}

fn lower_format_template(
    template: &str,
    args: &[factorio_ir::expression::Expression],
    location: String,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let pieces = parse_format_pieces(template);
    let sequential_count = pieces
        .iter()
        .filter(|piece| matches!(piece, FormatPiece::PositionalArg))
        .count();

    if sequential_count > args.len() {
        return Err(FrontendError::FormatArgumentMismatch {
            template: template.to_string(),
            expected: sequential_count,
            found: args.len(),
            location,
        });
    }

    for piece in &pieces {
        if let FormatPiece::PositionalIndex(index) = piece
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

    let mut parts = Vec::new();
    let mut sequential_index = 0;

    for piece in pieces {
        match piece {
            FormatPiece::Literal(value) => {
                parts.push(factorio_ir::expression::Expression::Literal(
                    factorio_ir::literal::Literal::String(value),
                ));
            }
            FormatPiece::PositionalArg => {
                let Some(arg) = args.get(sequential_index).cloned() else {
                    return Err(FrontendError::FormatArgumentMismatch {
                        template: template.to_string(),
                        expected: sequential_index + 1,
                        found: args.len(),
                        location,
                    });
                };
                parts.push(arg);
                sequential_index += 1;
            }
            FormatPiece::PositionalIndex(index) => {
                parts.push(args[index].clone());
            }
            FormatPiece::NamedCapture(name) => {
                parts.push(factorio_ir::expression::Expression::Identifier(name));
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

enum FormatPiece {
    Literal(String),
    PositionalArg,
    PositionalIndex(usize),
    NamedCapture(String),
}

fn parse_format_pieces(template: &str) -> Vec<FormatPiece> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if matches!(chars.peek(), Some('{')) {
                    chars.next();
                    literal.push('{');
                    continue;
                }

                if !literal.is_empty() {
                    pieces.push(FormatPiece::Literal(std::mem::take(&mut literal)));
                }

                let mut contents = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    contents.push(c);
                }

                if !closed {
                    literal.push('{');
                    literal.push_str(&contents);
                    continue;
                }

                pieces.push(parse_format_placeholder(&contents));
            }
            '}' => {
                if matches!(chars.peek(), Some('}')) {
                    chars.next();
                }
                literal.push('}');
            }
            other => literal.push(other),
        }
    }

    if !literal.is_empty() {
        pieces.push(FormatPiece::Literal(literal));
    }

    pieces
}

fn parse_format_placeholder(contents: &str) -> FormatPiece {
    if contents.is_empty() {
        return FormatPiece::PositionalArg;
    }

    let name = contents.split(':').next().unwrap_or(contents);

    if name.is_empty() {
        return FormatPiece::PositionalArg;
    }

    if let Ok(index) = name.parse::<usize>() {
        return FormatPiece::PositionalIndex(index);
    }

    if is_format_ident(name) {
        return FormatPiece::NamedCapture(name.to_string());
    }

    FormatPiece::PositionalArg
}

fn is_format_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}
