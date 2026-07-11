use syn::parse::{Parse, ParseStream};
use syn::{Expr, ExprMacro, LitStr};

use crate::error::{FrontendError, FrontendResult};

use super::{context::LowerContext, expressions::lower_expression, util::location};

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
    let name = mac.mac.path.get_ident().map_or_else(
        || {
            mac.mac
                .path
                .segments
                .last()
                .map_or_else(|| "macro".to_string(), |segment| segment.ident.to_string())
        },
        ToString::to_string,
    );

    match name.as_str() {
        "println" => lower_println_macro(mac, ctx, self_type),
        "format" => lower_format_macro(mac, ctx, self_type),
        _ => Err(FrontendError::UnsupportedMacro {
            name,
            location: location(mac),
        }),
    }
}

fn lower_println_macro(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let message = lower_format_macro_message(mac, ctx, self_type)?;
    Ok(factorio_ir::expression::Expression::Call {
        func: Box::new(factorio_ir::expression::Expression::FieldAccess {
            base: Box::new(factorio_ir::expression::Expression::Identifier(
                "game".to_string(),
            )),
            field: "print".to_string(),
        }),
        args: vec![message],
    })
}

fn lower_format_macro(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    lower_format_macro_message(mac, ctx, self_type)
}

fn lower_format_macro_message(
    mac: &ExprMacro,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let input = syn::parse2::<FormatMacroInput>(mac.mac.tokens.clone())?;
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
