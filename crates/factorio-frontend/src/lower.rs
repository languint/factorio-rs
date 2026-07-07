use std::collections::BTreeMap;

use syn::spanned::Spanned;
use syn::{
    BinOp, Block, Expr, ExprBinary, ExprLit, ExprPath, Fields, File, ImplItem, Item, ItemFn,
    ItemUse, Lit, Member, PatType, Signature, Stmt, Type, UseGroup, UseName, UsePath, UseRename,
    UseTree, Visibility,
};

use crate::{
    error::{FrontendError, FrontendResult},
    paths::{require_local_name, split_crate_path},
};

/// Parse Rust source into a [`factorio_ir::module::Module`].
///
/// `module_name` is used as the module identifier in the resulting IR.
pub fn parse_module(
    source: &str,
    module_name: &str,
) -> FrontendResult<factorio_ir::module::Module> {
    let file = syn::parse_file(source)?;
    lower_module(&file, module_name)
}

/// Lower a parsed Rust file into a module.
fn lower_module(file: &File, module_name: &str) -> FrontendResult<factorio_ir::module::Module> {
    let mut body = Vec::new();
    let mut symbols = Vec::new();
    let mut use_imports = Vec::new();
    let mut inline_imports = Vec::new();
    let mut submodules = Vec::new();
    let mut structs = BTreeMap::<String, PendingStruct>::new();
    let mut ctx = LowerContext {
        imports: &mut inline_imports,
    };

    for item in &file.items {
        match item {
            Item::Fn(function) => {
                let lowered = factorio_ir::statement::Statement::FunctionDecl(lower_function(
                    function, &mut ctx,
                )?);
                match &function.vis {
                    Visibility::Public(_) => symbols.push(factorio_ir::module::Symbol {
                        scope: factorio_ir::scope::Scope::Public,
                        statement: lowered,
                    }),
                    _ => body.push(lowered),
                }
            }
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let entry = structs
                    .entry(name.clone())
                    .or_insert_with(|| PendingStruct::new(item_struct.vis.clone()));
                entry.visibility = item_struct.vis.clone();
                entry.fields = lower_struct_fields(&item_struct.fields)?;
            }
            Item::Impl(item_impl) => {
                if item_impl.trait_.is_some() {
                    return Err(FrontendError::UnsupportedItem {
                        item: "trait impl".to_string(),
                        location: location(item_impl),
                    });
                }

                let struct_name = impl_type_name(&item_impl.self_ty)?;
                let entry = structs
                    .entry(struct_name.clone())
                    .or_insert_with(|| PendingStruct::new(Visibility::Inherited));

                for impl_item in &item_impl.items {
                    match impl_item {
                        ImplItem::Fn(method) => {
                            entry
                                .methods
                                .push(lower_impl_method(method, &struct_name, &mut ctx)?);
                        }
                        ImplItem::Const(item) => {
                            let value = lower_expression(&item.expr, &mut ctx, Some(&struct_name))?;
                            entry.constants.push((item.ident.to_string(), value));
                        }
                        item => {
                            return Err(FrontendError::UnsupportedItem {
                                item: item_name_impl(item),
                                location: location(item),
                            });
                        }
                    }
                }
            }
            Item::Use(use_item) => use_imports.extend(lower_use(use_item)?),
            Item::Mod(item_mod) if item_mod.content.is_none() => {
                submodules.push(submodule_path(module_name, &item_mod.ident.to_string()));
            }
            Item::Mod(item_mod) => {
                return Err(FrontendError::UnsupportedItem {
                    item: "inline mod".to_string(),
                    location: location(item_mod),
                });
            }
            item => {
                return Err(FrontendError::UnsupportedItem {
                    item: item_name(item),
                    location: location(item),
                });
            }
        }
    }

    for (name, pending_struct) in structs {
        let lowered =
            factorio_ir::statement::Statement::StructDecl(factorio_ir::structure::Struct {
                name,
                fields: pending_struct.fields,
                constants: pending_struct.constants,
                methods: pending_struct.methods,
            });

        match &pending_struct.visibility {
            Visibility::Public(_) => symbols.push(factorio_ir::module::Symbol {
                scope: factorio_ir::scope::Scope::Public,
                statement: lowered,
            }),
            _ => body.push(lowered),
        }
    }

    let mut all_imports = use_imports;
    all_imports.extend(inline_imports);

    Ok(factorio_ir::module::Module {
        name: module_name.to_string(),
        body: factorio_ir::block::Block { statements: body },
        symbols,
        imports: merge_imports(all_imports),
        submodules,
    })
}

fn submodule_path(module_name: &str, child: &str) -> String {
    format!("{module_name}.{child}")
}

struct RawUseBinding {
    segments: Vec<String>,
    rename: Option<String>,
}

struct ImportFragment {
    module: String,
    require_local: String,
    item: Option<factorio_ir::module::ImportedItem>,
}

fn lower_use(item: &ItemUse) -> FrontendResult<Vec<ImportFragment>> {
    let mut bindings = Vec::new();
    collect_use_bindings(&item.tree, &mut Vec::new(), &mut bindings)?;

    let mut fragments = Vec::new();
    for binding in bindings {
        if let Some(fragment) = finalize_use_binding(binding)? {
            fragments.push(fragment);
        }
    }

    Ok(fragments)
}

fn collect_use_bindings(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    bindings: &mut Vec<RawUseBinding>,
) -> FrontendResult<()> {
    match tree {
        UseTree::Path(UsePath { ident, tree, .. }) => {
            prefix.push(ident.to_string());
            collect_use_bindings(tree, prefix, bindings)?;
            prefix.pop();
            Ok(())
        }
        UseTree::Name(UseName { ident, .. }) => {
            prefix.push(ident.to_string());
            bindings.push(RawUseBinding {
                segments: prefix.clone(),
                rename: None,
            });
            prefix.pop();
            Ok(())
        }
        UseTree::Rename(UseRename { ident, rename, .. }) => {
            prefix.push(ident.to_string());
            bindings.push(RawUseBinding {
                segments: prefix.clone(),
                rename: Some(rename.to_string()),
            });
            prefix.pop();
            Ok(())
        }
        UseTree::Glob(_) => Err(FrontendError::UnsupportedItem {
            item: "use glob".to_string(),
            location: location(tree),
        }),
        UseTree::Group(UseGroup { items, .. }) => {
            for item in items {
                collect_use_bindings(item, prefix, bindings)?;
            }
            Ok(())
        }
    }
}

fn finalize_use_binding(binding: RawUseBinding) -> FrontendResult<Option<ImportFragment>> {
    if binding.segments.first().map(String::as_str) != Some("crate") {
        return Ok(None);
    }

    let (module_path, item_segments) = split_crate_path(&binding.segments[1..]);
    if module_path.is_empty() {
        return Err(FrontendError::UnsupportedItem {
            item: format!("use {}", binding.segments.join("::")),
            location: "use".to_string(),
        });
    }

    if item_segments.is_empty() {
        return Ok(Some(ImportFragment {
            module: module_path.clone(),
            require_local: binding
                .rename
                .unwrap_or_else(|| require_local_name(&module_path)),
            item: None,
        }));
    }

    if item_segments.len() == 1 {
        return Ok(Some(ImportFragment {
            module: module_path.clone(),
            require_local: require_local_name(&module_path),
            item: Some(factorio_ir::module::ImportedItem {
                name: item_segments[0].clone(),
                local: binding.rename.unwrap_or_else(|| item_segments[0].clone()),
            }),
        }));
    }

    Err(FrontendError::UnsupportedItem {
        item: format!("use {}", binding.segments.join("::")),
        location: "use".to_string(),
    })
}

fn merge_imports(fragments: Vec<ImportFragment>) -> Vec<factorio_ir::module::ModuleImport> {
    let mut merged = BTreeMap::<String, factorio_ir::module::ModuleImport>::new();

    for fragment in fragments {
        let entry = merged.entry(fragment.module.clone()).or_insert_with(|| {
            factorio_ir::module::ModuleImport {
                module: fragment.module.clone(),
                local: require_local_name(&fragment.module),
                items: Vec::new(),
            }
        });

        if fragment.item.is_none() {
            entry.local = fragment.require_local;
        }

        if let Some(item) = fragment.item {
            if !entry
                .items
                .iter()
                .any(|existing| existing.local == item.local)
            {
                entry.items.push(item);
            }
        }
    }

    merged.into_values().collect()
}

struct LowerContext<'a> {
    imports: &'a mut Vec<ImportFragment>,
}

impl LowerContext<'_> {
    fn register_crate_module(&mut self, module: &str) {
        if self
            .imports
            .iter()
            .any(|fragment| fragment.module == module)
        {
            return;
        }

        self.imports.push(ImportFragment {
            module: module.to_string(),
            require_local: require_local_name(module),
            item: None,
        });
    }

    fn normalize_crate_path(&mut self, segments: &mut Vec<String>) -> FrontendResult<()> {
        if segments.first().map(String::as_str) != Some("crate") {
            return Ok(());
        }

        segments.remove(0);
        if segments.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: "crate".to_string(),
            });
        }

        let (module_path, rest) = split_crate_path(segments);
        if module_path.is_empty() {
            return Err(FrontendError::UnsupportedExpression {
                location: segments.join("::"),
            });
        }

        self.register_crate_module(&module_path);

        let local = require_local_name(&module_path);
        *segments = if rest.is_empty() {
            vec![local]
        } else {
            let mut rewritten = vec![local];
            rewritten.extend(rest);
            rewritten
        };

        Ok(())
    }
}

struct PendingStruct {
    visibility: Visibility,
    fields: Vec<factorio_ir::structure::StructField>,
    constants: Vec<(String, factorio_ir::expression::Expression)>,
    methods: Vec<factorio_ir::function::Function>,
}

impl PendingStruct {
    fn new(visibility: Visibility) -> Self {
        Self {
            visibility,
            fields: Vec::new(),
            constants: Vec::new(),
            methods: Vec::new(),
        }
    }
}

fn lower_struct_fields(
    fields: &Fields,
) -> FrontendResult<Vec<factorio_ir::structure::StructField>> {
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|field| {
                let name = field.ident.as_ref().ok_or_else(|| {
                    FrontendError::ExpectedIdentifierPattern {
                        location: location(field),
                    }
                })?;
                Ok(factorio_ir::structure::StructField {
                    name: name.to_string(),
                    ty: lower_type(&field.ty)?,
                })
            })
            .collect(),
        Fields::Unit => Ok(Vec::new()),
        Fields::Unnamed(_) => Err(FrontendError::UnsupportedItem {
            item: "tuple struct".to_string(),
            location: location(fields),
        }),
    }
}

fn impl_type_name(ty: &Type) -> FrontendResult<String> {
    match ty {
        Type::Path(path) if path.path.segments.len() == 1 => {
            Ok(path.path.segments[0].ident.to_string())
        }
        _ => Err(FrontendError::UnsupportedType {
            ty: "unsupported impl type".to_string(),
            location: location(ty),
        }),
    }
}

fn lower_function(
    function: &ItemFn,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Function> {
    lower_function_parts(&function.sig, &function.block, ctx, None)
}

fn lower_impl_method(
    method: &syn::ImplItemFn,
    self_type: &str,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<factorio_ir::function::Function> {
    lower_function_parts(&method.sig, &method.block, ctx, Some(self_type))
}

fn lower_function_parts(
    signature: &Signature,
    block: &Block,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::function::Function> {
    Ok(factorio_ir::function::Function {
        name: signature.ident.to_string(),
        params: lower_parameters(signature)?,
        body: lower_block(block, ctx, self_type)?,
    })
}

fn lower_parameters(
    signature: &Signature,
) -> FrontendResult<Vec<factorio_ir::function::Parameter>> {
    signature
        .inputs
        .iter()
        .map(lower_parameter)
        .collect::<FrontendResult<Vec<_>>>()
}

fn lower_parameter(input: &syn::FnArg) -> FrontendResult<factorio_ir::function::Parameter> {
    match input {
        syn::FnArg::Receiver(_receiver) => Ok(factorio_ir::function::Parameter {
            // `&self` and `&mut self` become a `self` parameter.
            name: "self".to_string(),
            r#type: factorio_ir::r#type::Type::Void,
        }),
        syn::FnArg::Typed(PatType { pat, ty, .. }) => {
            let name = lower_binding_pattern(pat)?;
            let r#type = lower_type(ty)?;

            Ok(factorio_ir::function::Parameter { name, r#type })
        }
    }
}

/// Lower a block of Rust statements into IR statements.
fn lower_block(
    block: &Block,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::block::Block> {
    let mut statements = Vec::new();
    let last_index = block.stmts.len().saturating_sub(1);

    for (index, statement) in block.stmts.iter().enumerate() {
        let is_tail = index == last_index;
        statements.extend(lower_statement(statement, is_tail, ctx, self_type)?);
    }

    Ok(factorio_ir::block::Block { statements })
}

/// Lower a single Rust statement into zero or more IR statements.
fn lower_statement(
    statement: &Stmt,
    is_tail: bool,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match statement {
        Stmt::Local(local) => {
            let (name, annotated_type) = lower_binding(&local.pat)?;
            let init = local
                .init
                .as_ref()
                .ok_or_else(|| FrontendError::MissingLetInitializer {
                    location: location(local),
                })?;
            let value = lower_expression(&init.expr, ctx, self_type)?;
            let ty = match annotated_type {
                Some(ty) => ty,
                None => {
                    infer_type_from_expression(&value).unwrap_or(factorio_ir::r#type::Type::Void)
                }
            };

            Ok(vec![factorio_ir::statement::Statement::VariableDecl {
                name,
                ty,
                value,
            }])
        }
        Stmt::Item(syn::Item::Fn(function)) => {
            Ok(vec![factorio_ir::statement::Statement::FunctionDecl(
                lower_function(function, ctx)?,
            )])
        }
        Stmt::Item(item) => Err(FrontendError::UnsupportedItem {
            item: item_name(item),
            location: location(item),
        }),
        Stmt::Expr(expression, semi) => {
            lower_expression_statement(expression, semi.is_some(), is_tail, ctx, self_type)
        }
        _ => Err(FrontendError::UnsupportedStatement {
            location: location(statement),
        }),
    }
}

fn lower_expression_statement(
    expression: &Expr,
    has_semi: bool,
    is_tail: bool,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    if has_semi {
        return Ok(vec![lower_semicolon_expression(
            expression, ctx, self_type,
        )?]);
    }

    if is_tail {
        return Ok(vec![lower_tail_expression(expression, ctx, self_type)?]);
    }

    Err(FrontendError::UnsupportedStatement {
        location: location(expression),
    })
}

/// Lower a tail expression without a trailing semicolon.
///
/// Control-flow expressions such as `if` remain statements. Other expressions
/// become implicit `return` values.
fn lower_tail_expression(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    match expression {
        Expr::If(if_expression) => lower_if_expression(if_expression, ctx, self_type),
        Expr::Return(return_expression) => Ok(factorio_ir::statement::Statement::Return(
            match return_expression.expr.as_deref() {
                Some(value) => Some(lower_expression(value, ctx, self_type)?),
                None => None,
            },
        )),
        _ => Ok(factorio_ir::statement::Statement::Return(Some(
            lower_expression(expression, ctx, self_type)?,
        ))),
    }
}

fn lower_semicolon_expression(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    match expression {
        Expr::Return(return_expression) => Ok(factorio_ir::statement::Statement::Return(
            match return_expression.expr.as_deref() {
                Some(value) => Some(lower_expression(value, ctx, self_type)?),
                None => None,
            },
        )),
        Expr::Assign(assign) => Ok(factorio_ir::statement::Statement::Assignment {
            target: lower_assignment_target(&assign.left, ctx, self_type)?,
            value: lower_expression(&assign.right, ctx, self_type)?,
        }),
        Expr::If(if_expression) => lower_if_expression(if_expression, ctx, self_type),
        Expr::Call(_) | Expr::MethodCall(_) => Ok(factorio_ir::statement::Statement::Expr(
            lower_expression(expression, ctx, self_type)?,
        )),
        _ => Err(FrontendError::UnsupportedStatement {
            location: location(expression),
        }),
    }
}

fn lower_if_expression(
    if_expression: &syn::ExprIf,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::statement::Statement> {
    let condition = lower_expression(&if_expression.cond, ctx, self_type)?;
    let then_block = lower_block_statements(&if_expression.then_branch.stmts, ctx, self_type)?;
    let else_block = match &if_expression.else_branch {
        Some((_, else_branch)) => lower_branch_statements(else_branch, ctx, self_type)?,
        None => Vec::new(),
    };

    Ok(factorio_ir::statement::Statement::Conditional {
        condition,
        then_block,
        else_block,
    })
}

fn lower_branch_statements(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    match expression {
        Expr::Block(block) => lower_block_statements(&block.block.stmts, ctx, self_type),
        _ => Err(FrontendError::UnsupportedStatement {
            location: location(expression),
        }),
    }
}

fn lower_block_statements(
    statements: &[Stmt],
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<Vec<factorio_ir::statement::Statement>> {
    let mut lowered = Vec::new();
    let last_index = statements.len().saturating_sub(1);

    for (index, statement) in statements.iter().enumerate() {
        let is_tail = index == last_index;
        lowered.extend(lower_statement(statement, is_tail, ctx, self_type)?);
    }

    Ok(lowered)
}

/// Lower a Rust expression into IR.
fn lower_expression(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    match expression {
        Expr::Binary(binary) => lower_binary_expression(binary, ctx, self_type),
        Expr::Lit(literal) => lower_literal_expression(literal),
        Expr::Path(path) => lower_path_expression(path, ctx, self_type),
        Expr::Field(field) => lower_field_expression(field, ctx, self_type),
        Expr::Call(call) => {
            let func = lower_expression(&call.func, ctx, self_type)?;
            let args = call
                .args
                .iter()
                .map(|arg| lower_expression(arg, ctx, self_type))
                .collect::<FrontendResult<Vec<_>>>()?;
            Ok(factorio_ir::expression::Expression::Call {
                func: Box::new(func),
                args,
            })
        }
        Expr::MethodCall(call) => {
            let receiver = lower_expression(&call.receiver, ctx, self_type)?;
            let args = call
                .args
                .iter()
                .map(|arg| lower_expression(arg, ctx, self_type))
                .collect::<FrontendResult<Vec<_>>>()?;
            Ok(factorio_ir::expression::Expression::MethodCall {
                receiver: Box::new(receiver),
                method: call.method.to_string(),
                args,
            })
        }
        Expr::Struct(item) => lower_struct_expression(item, ctx, self_type),
        _ => Err(FrontendError::UnsupportedExpression {
            location: location(expression),
        }),
    }
}

fn lower_struct_expression(
    item: &syn::ExprStruct,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let fields = item
        .fields
        .iter()
        .map(|field| {
            let name = match &field.member {
                Member::Named(ident) => ident.to_string(),
                Member::Unnamed(index) => {
                    return Err(FrontendError::UnsupportedExpression {
                        location: location(index),
                    });
                }
            };
            Ok((name, lower_expression(&field.expr, ctx, self_type)?))
        })
        .collect::<FrontendResult<Vec<_>>>()?;

    Ok(factorio_ir::expression::Expression::StructLiteral { fields })
}

fn lower_field_expression(
    field: &syn::ExprField,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let base = lower_expression(&field.base, ctx, self_type)?;
    let field_name = match &field.member {
        Member::Named(ident) => ident.to_string(),
        Member::Unnamed(index) => {
            return Err(FrontendError::UnsupportedExpression {
                location: location(index),
            });
        }
    };

    Ok(factorio_ir::expression::Expression::FieldAccess {
        base: Box::new(base),
        field: field_name,
    })
}

fn lower_binary_expression(
    binary: &ExprBinary,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let lhs = lower_expression(&binary.left, ctx, self_type)?;
    let op = lower_binary_operator(&binary.op)?;
    let rhs = lower_expression(&binary.right, ctx, self_type)?;

    Ok(factorio_ir::expression::Expression::BinaryOp {
        lhs: Box::new(lhs),
        op,
        rhs: Box::new(rhs),
    })
}

fn lower_binary_operator(operator: &BinOp) -> FrontendResult<factorio_ir::operator::Operator> {
    let operator = match operator {
        BinOp::Add(_) => factorio_ir::operator::Operator::Add,
        BinOp::Sub(_) => factorio_ir::operator::Operator::Sub,
        BinOp::Mul(_) => factorio_ir::operator::Operator::Mul,
        BinOp::Div(_) => factorio_ir::operator::Operator::Div,
        BinOp::Eq(_) => factorio_ir::operator::Operator::Eq,
        _ => {
            return Err(FrontendError::UnsupportedOperator {
                location: location(operator),
            });
        }
    };

    Ok(operator)
}

fn lower_literal_expression(
    literal: &ExprLit,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let literal = match &literal.lit {
        Lit::Int(value) => {
            let parsed = value
                .base10_parse::<i64>()
                .map_err(|error| FrontendError::Syn(format!("invalid integer literal: {error}")))?;
            factorio_ir::literal::Literal::Int(parsed)
        }
        Lit::Float(value) => {
            let parsed = value
                .base10_parse::<f64>()
                .map_err(|error| FrontendError::Syn(format!("invalid float literal: {error}")))?;
            factorio_ir::literal::Literal::Float(parsed)
        }
        Lit::Str(value) => factorio_ir::literal::Literal::String(value.value()),
        _ => {
            return Err(FrontendError::UnsupportedExpression {
                location: location(literal),
            });
        }
    };

    Ok(factorio_ir::expression::Expression::Literal(literal))
}

fn lower_path_expression(
    path: &ExprPath,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    let mut segments = lower_path_segments(path, self_type)?;
    ctx.normalize_crate_path(&mut segments)?;

    match segments.len() {
        1 => Ok(factorio_ir::expression::Expression::Identifier(
            segments[0].clone(),
        )),
        _ => Ok(factorio_ir::expression::Expression::QualifiedPath { segments }),
    }
}

fn lower_path_segments(path: &ExprPath, self_type: Option<&str>) -> FrontendResult<Vec<String>> {
    path.path
        .segments
        .iter()
        .map(|segment| resolve_path_segment(&segment.ident, self_type))
        .collect()
}

fn resolve_path_segment(ident: &syn::Ident, self_type: Option<&str>) -> FrontendResult<String> {
    if ident == "Self" {
        return self_type
            .map(str::to_string)
            .ok_or_else(|| FrontendError::UnsupportedExpression {
                location: location(ident),
            });
    }

    Ok(ident.to_string())
}

fn lower_assignment_target(
    expression: &Expr,
    ctx: &mut LowerContext<'_>,
    self_type: Option<&str>,
) -> FrontendResult<factorio_ir::expression::Expression> {
    match expression {
        Expr::Path(path) => lower_path_expression(path, ctx, self_type),
        Expr::Field(field) => lower_field_expression(field, ctx, self_type),
        _ => Err(FrontendError::ExpectedIdentifierAssignmentTarget {
            location: location(expression),
        }),
    }
}

/// Infer a type from a literal expression when a `let` binding has no annotation.
fn infer_type_from_expression(
    expression: &factorio_ir::expression::Expression,
) -> Option<factorio_ir::r#type::Type> {
    match expression {
        factorio_ir::expression::Expression::Literal(literal) => match literal {
            factorio_ir::literal::Literal::Int(_) => Some(factorio_ir::r#type::Type::Int),
            factorio_ir::literal::Literal::Float(_) => Some(factorio_ir::r#type::Type::Float),
            factorio_ir::literal::Literal::String(_) => Some(factorio_ir::r#type::Type::Str),
        },
        _ => None,
    }
}

/// Lower a Rust type into IR.
fn lower_type(ty: &Type) -> FrontendResult<factorio_ir::r#type::Type> {
    match ty {
        Type::Path(path) => lower_path_type(path),
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(factorio_ir::r#type::Type::Void),
        Type::Reference(reference) if is_self_type(&reference.elem) => {
            Ok(factorio_ir::r#type::Type::Void)
        }
        _ => Err(FrontendError::UnsupportedType {
            ty: "unsupported type".to_string(),
            location: location(ty),
        }),
    }
}

fn lower_path_type(path: &syn::TypePath) -> FrontendResult<factorio_ir::r#type::Type> {
    let segment = path
        .path
        .segments
        .last()
        .ok_or_else(|| FrontendError::UnsupportedType {
            ty: "empty path".to_string(),
            location: location(path),
        })?;

    let ty = match segment.ident.to_string().as_str() {
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => factorio_ir::r#type::Type::Int,
        "f32" | "f64" => factorio_ir::r#type::Type::Float,
        "str" | "String" => factorio_ir::r#type::Type::Str,
        _ => factorio_ir::r#type::Type::Void,
    };

    Ok(ty)
}

fn is_self_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident("Self"))
}

fn lower_binding(
    pattern: &syn::Pat,
) -> FrontendResult<(String, Option<factorio_ir::r#type::Type>)> {
    match pattern {
        syn::Pat::Type(pat_type) => {
            let name = lower_binding_pattern(&pat_type.pat)?;
            let ty = lower_type(&pat_type.ty)?;
            Ok((name, Some(ty)))
        }
        pattern => {
            let name = lower_binding_pattern(pattern)?;
            Ok((name, None))
        }
    }
}

fn lower_binding_pattern(pattern: &syn::Pat) -> FrontendResult<String> {
    match pattern {
        syn::Pat::Ident(ident) => Ok(ident.ident.to_string()),
        syn::Pat::Type(pat_type) => lower_binding_pattern(&pat_type.pat),
        syn::Pat::Wild(_) => Ok("_".to_string()),
        _ => Err(FrontendError::ExpectedIdentifierPattern {
            location: location(pattern),
        }),
    }
}

/// Returns a source location string for error reporting.
fn location(span: impl Spanned) -> String {
    format!("{:?}", span.span())
}

/// Returns a short description of a top-level item for error reporting.
fn item_name(item: &syn::Item) -> String {
    match item {
        syn::Item::Fn(function) => format!("fn {}", function.sig.ident),
        syn::Item::Mod(module) => format!("mod {}", module.ident),
        syn::Item::Struct(item) => format!("struct {}", item.ident),
        syn::Item::Enum(item) => format!("enum {}", item.ident),
        syn::Item::Const(item) => format!("const {}", item.ident),
        syn::Item::Static(item) => format!("static {}", item.ident),
        syn::Item::Use(_) => "use".to_string(),
        syn::Item::Type(item) => format!("type {}", item.ident),
        syn::Item::Macro(_) => "macro".to_string(),
        _ => "item".to_string(),
    }
}

fn item_name_impl(item: &ImplItem) -> String {
    match item {
        ImplItem::Fn(function) => format!("fn {}", function.sig.ident),
        ImplItem::Const(item) => format!("const {}", item.ident),
        ImplItem::Type(item) => format!("type {}", item.ident),
        ImplItem::Macro(_) => "macro".to_string(),
        _ => "impl item".to_string(),
    }
}
