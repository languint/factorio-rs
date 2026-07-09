use std::collections::BTreeMap;

use syn::{ImplItem, Item, Visibility};

use crate::error::{FrontendError, FrontendResult};

pub mod attrs;
pub mod context;
pub mod expressions;
pub mod functions;
pub mod imports;
pub mod metadata;
pub mod print;
pub mod statements;
pub mod structs;
pub mod types;
pub mod util;

use context::LowerContext;
use expressions::lower_expression;
use functions::{lower_function, lower_impl_method};
use imports::{lower_use, merge_imports};
use metadata::{extract_doc_comments, struct_header_comment};
use structs::{PendingStruct, impl_type_name, lower_struct_fields};
use util::{item_name, item_name_impl, location};

/// Parse Rust source into a [`factorio_ir::module::Module`].
///
/// `module_name` is used as the module identifier in the resulting IR.
///
/// # Errors
/// Returns `Err` if the Rust source fails to parse
pub fn parse_module(
    source: &str,
    module_name: &str,
) -> FrontendResult<factorio_ir::module::Module> {
    let file = syn::parse_file(source)?;
    let stage = factorio_ir::stage::Stage::from_module_name(module_name).ok_or_else(|| {
        FrontendError::InvalidModuleStage {
            module: module_name.to_string(),
        }
    })?;
    lower_items(&file.items, module_name, stage)
}

/// Lower a discovered module into IR.
///
/// # Errors
/// Returns `Err` if lowering fails.
pub fn parse_discovered_module(
    discovered: &crate::discovery::DiscoveredModule,
) -> FrontendResult<factorio_ir::module::Module> {
    lower_items(&discovered.items, &discovered.module_name, discovered.stage)
}

fn lower_items(
    items: &[Item],
    module_name: &str,
    stage: factorio_ir::stage::Stage,
) -> FrontendResult<factorio_ir::module::Module> {
    let mut body = Vec::new();
    let mut symbols = Vec::new();
    let mut use_imports = Vec::new();
    let mut inline_imports = Vec::new();
    let mut submodules = Vec::new();
    let mut structs = BTreeMap::<String, PendingStruct>::new();
    let mut ctx = LowerContext {
        imports: &mut inline_imports,
    };
    let mut module_state = ModuleLowerState {
        module_name,
        stage,
        body: &mut body,
        symbols: &mut symbols,
        use_imports: &mut use_imports,
        submodules: &mut submodules,
        structs: &mut structs,
    };

    for item in items {
        lower_top_level_item(item, module_name, &mut module_state, &mut ctx)?;
    }

    finalize_pending_structs(structs, &mut body, &mut symbols);

    let mut all_imports = use_imports;
    all_imports.extend(inline_imports);

    Ok(factorio_ir::module::Module {
        name: module_name.to_string(),
        stage,
        body: factorio_ir::block::Block { statements: body },
        symbols,
        imports: merge_imports(all_imports),
        submodules,
    })
}

struct ModuleLowerState<'a> {
    module_name: &'a str,
    stage: factorio_ir::stage::Stage,
    body: &'a mut Vec<factorio_ir::statement::Statement>,
    symbols: &'a mut Vec<factorio_ir::module::Symbol>,
    use_imports: &'a mut Vec<imports::ImportFragment>,
    submodules: &'a mut Vec<String>,
    structs: &'a mut BTreeMap<String, PendingStruct>,
}

fn lower_top_level_item(
    item: &Item,
    module_name: &str,
    module_state: &mut ModuleLowerState<'_>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
    match item {
        Item::Fn(function) => {
            let lowered =
                factorio_ir::statement::Statement::FunctionDecl(lower_function(function, ctx)?);
            if let factorio_ir::statement::Statement::FunctionDecl(ref func) = lowered
                && func.event.is_some()
                && module_state.stage != factorio_ir::stage::Stage::Control
            {
                return Err(FrontendError::EventOutsideControlStage {
                    module: module_state.module_name.to_string(),
                });
            }
            push_scoped_statement(
                lowered,
                &function.vis,
                module_state.body,
                module_state.symbols,
            );
        }
        Item::Struct(item_struct) => lower_struct_item(item_struct, module_state.structs)?,
        Item::Impl(item_impl) => lower_impl_item(item_impl, module_state.structs, ctx)?,
        Item::Use(use_item) => module_state.use_imports.extend(lower_use(use_item)?),
        Item::Mod(item_mod) if item_mod.content.is_none() => {
            module_state
                .submodules
                .push(submodule_path(module_name, &item_mod.ident.to_string()));
        }
        Item::Mod(_) => {}
        item => {
            return Err(FrontendError::UnsupportedItem {
                item: item_name(item),
                location: location(item),
            });
        }
    }

    Ok(())
}

fn lower_struct_item(
    item_struct: &syn::ItemStruct,
    structs: &mut BTreeMap<String, PendingStruct>,
) -> FrontendResult<()> {
    let name = item_struct.ident.to_string();
    let entry = structs
        .entry(name)
        .or_insert_with(|| PendingStruct::new(item_struct.vis.clone()));
    entry.visibility = item_struct.vis.clone();
    entry.fields = lower_struct_fields(&item_struct.fields)?;
    entry.doc = extract_doc_comments(&item_struct.attrs);
    Ok(())
}

fn lower_impl_item(
    item_impl: &syn::ItemImpl,
    structs: &mut BTreeMap<String, PendingStruct>,
    ctx: &mut LowerContext<'_>,
) -> FrontendResult<()> {
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
                    .push(lower_impl_method(method, &struct_name, ctx)?);
            }
            ImplItem::Const(item) => {
                let value = lower_expression(&item.expr, ctx, Some(&struct_name))?;
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

    Ok(())
}

fn finalize_pending_structs(
    structs: BTreeMap<String, PendingStruct>,
    body: &mut Vec<factorio_ir::statement::Statement>,
    symbols: &mut Vec<factorio_ir::module::Symbol>,
) {
    for (name, pending_struct) in structs {
        let lowered =
            factorio_ir::statement::Statement::StructDecl(factorio_ir::structure::Struct {
                name: name.clone(),
                fields: pending_struct.fields.clone(),
                constants: pending_struct.constants,
                methods: pending_struct.methods,
                doc: pending_struct.doc,
                debug: Some(factorio_ir::debug::StructDebug {
                    header_comment: struct_header_comment(
                        &pending_struct.visibility,
                        &name,
                        &pending_struct.fields,
                    ),
                }),
            });

        push_scoped_statement(lowered, &pending_struct.visibility, body, symbols);
    }
}

fn push_scoped_statement(
    statement: factorio_ir::statement::Statement,
    visibility: &Visibility,
    body: &mut Vec<factorio_ir::statement::Statement>,
    symbols: &mut Vec<factorio_ir::module::Symbol>,
) {
    match visibility {
        Visibility::Public(_) => symbols.push(factorio_ir::module::Symbol {
            scope: factorio_ir::scope::Scope::Public,
            statement,
        }),
        _ => body.push(statement),
    }
}

fn submodule_path(module_name: &str, child: &str) -> String {
    format!("{module_name}.{child}")
}
