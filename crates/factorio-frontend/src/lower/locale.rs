use std::collections::BTreeMap;

use factorio_ir::{
    expression::Expression,
    literal::Literal,
    locale::{LocaleEntry, LocaleFile, PendingLocaleEntry, PendingLocaleFile, PendingLocaleKey},
    module::Module,
    prune::{ModuleGraph, find_struct_constant_in_module_tree},
    statement::Statement,
};
use proc_macro2::TokenStream;
use syn::{
    Ident, LitStr, Path, Token,
    parse::{Parse, ParseStream},
};

use crate::{
    error::{FrontendError, FrontendResult},
    paths::split_crate_path,
};

/// Parse a `locale!` invocation into pending locale files (keys unresolved).
///
/// # Errors
/// Returns [`FrontendError::Syn`] when the macro tokens are not valid `locale!`
/// syntax, or [`FrontendError::InvalidLocale`] when a value is not a single line.
pub fn parse_pending(tokens: TokenStream) -> FrontendResult<Vec<PendingLocaleFile>> {
    let input: LocaleInput = syn::parse2(tokens).map_err(|e| FrontendError::Syn(e.to_string()))?;

    let file = input.file.unwrap_or_else(|| "locale".to_string());
    let mut files = Vec::new();

    for lang_block in input.languages {
        let mut entries = Vec::new();
        for category in lang_block.categories {
            for entry in category.entries {
                validate_cfg_value(&entry.value)?;
                entries.push(PendingLocaleEntry {
                    category: Some(category.name.clone()),
                    key: match entry.key {
                        LocaleKey::Literal(s) => PendingLocaleKey::Literal(s),
                        LocaleKey::Path(path) => PendingLocaleKey::Path(path_segments(&path)),
                    },
                    value: entry.value,
                });
            }
        }
        files.push(PendingLocaleFile {
            lang: lang_block.lang,
            file: file.clone(),
            entries,
        });
    }

    Ok(files)
}

/// Resolve pending `locale!` keys against local and imported associated string
/// constants across `modules`, filling [`Module::locales`].
///
/// # Errors
/// Returns `LocaleKeyUnresolved` when a path key cannot be resolved.
pub fn resolve_project_locales(modules: &mut [Module]) -> FrontendResult<()> {
    let resolved: FrontendResult<Vec<(usize, Vec<LocaleFile>)>> = {
        let graph = ModuleGraph::new(modules);
        let mut out = Vec::new();
        for (idx, module) in modules.iter().enumerate() {
            if module.pending_locales.is_empty() {
                continue;
            }
            let local_consts = collect_const_strings(&module.body.statements, &module.symbols);
            let mut locales = Vec::new();
            for pending_file in &module.pending_locales {
                let mut entries = Vec::new();
                for entry in &pending_file.entries {
                    let key = resolve_pending_key(&entry.key, module, &local_consts, &graph)?;
                    validate_cfg_key(&key)?;
                    entries.push(LocaleEntry {
                        category: entry.category.clone(),
                        key,
                        value: entry.value.clone(),
                    });
                }
                locales.push(LocaleFile {
                    lang: pending_file.lang.clone(),
                    file: pending_file.file.clone(),
                    entries,
                });
            }
            out.push((idx, locales));
        }
        Ok(out)
    };

    for (idx, locales) in resolved? {
        if let Some(module) = modules.get_mut(idx) {
            module.locales = locales;
            module.pending_locales.clear();
        }
    }

    Ok(())
}

/// Collect `StructName::CONST` -> string value from lowered module statements.
pub fn collect_const_strings(
    body: &[Statement],
    symbols: &[factorio_ir::module::Symbol],
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for statement in body
        .iter()
        .chain(symbols.iter().map(|symbol| &symbol.statement))
    {
        let Statement::StructDecl(struct_decl) = statement else {
            continue;
        };
        for (name, value) in &struct_decl.constants {
            if let Expression::Literal(Literal::String(s)) = value {
                map.insert(format!("{}::{name}", struct_decl.name), s.clone());
            }
        }
    }
    map
}

fn resolve_pending_key(
    key: &PendingLocaleKey,
    module: &Module,
    local_consts: &BTreeMap<String, String>,
    graph: &ModuleGraph<'_>,
) -> FrontendResult<String> {
    match key {
        PendingLocaleKey::Literal(s) => Ok(s.clone()),
        PendingLocaleKey::Path(segments) => resolve_path_key(segments, module, local_consts, graph),
    }
}

fn resolve_path_key(
    segments: &[String],
    module: &Module,
    local_consts: &BTreeMap<String, String>,
    graph: &ModuleGraph<'_>,
) -> FrontendResult<String> {
    let path_str = segments.join("::");

    if segments.len() == 2 {
        let type_name = &segments[0];
        let const_name = &segments[1];
        let local_key = format!("{type_name}::{const_name}");
        if let Some(value) = local_consts.get(&local_key) {
            return Ok(value.clone());
        }
        if let Some(value) = resolve_type_const_via_imports(type_name, const_name, module, graph) {
            return Ok(value);
        }
        return Err(FrontendError::LocaleKeyUnresolved { path: path_str });
    }

    if let Some(value) = resolve_fq_path(segments, graph) {
        return Ok(value);
    }

    Err(FrontendError::LocaleKeyUnresolved { path: path_str })
}

fn resolve_type_const_via_imports(
    type_name: &str,
    const_name: &str,
    module: &Module,
    graph: &ModuleGraph<'_>,
) -> Option<String> {
    for import in &module.imports {
        for item in &import.items {
            if item.local == type_name || item.name == type_name {
                return string_const_in_tree(graph, &import.module, &item.name, const_name);
            }
        }
        // `use crate::data::items;` / `use crate::data::items::*;` - module require only.
        if import.items.is_empty()
            && let Some(value) = string_const_in_tree(graph, &import.module, type_name, const_name)
        {
            return Some(value);
        }
    }
    None
}

fn resolve_fq_path(segments: &[String], graph: &ModuleGraph<'_>) -> Option<String> {
    let segs = if segments.first().map(String::as_str) == Some("crate") {
        &segments[1..]
    } else {
        segments
    };
    if segs.len() < 3 {
        return None;
    }
    let const_name = segs.last()?;
    let type_and_module = &segs[..segs.len() - 1];
    let (module_path, item_segments) = split_crate_path(type_and_module);
    if item_segments.len() != 1 || module_path.is_empty() {
        return None;
    }
    string_const_in_tree(graph, &module_path, &item_segments[0], const_name)
}

fn string_const_in_tree(
    graph: &ModuleGraph<'_>,
    module_name: &str,
    struct_name: &str,
    constant_name: &str,
) -> Option<String> {
    let (_owner, expr) =
        find_struct_constant_in_module_tree(graph, module_name, struct_name, constant_name)?;
    match expr {
        Expression::Literal(Literal::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn path_segments(path: &Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn validate_cfg_key(key: &str) -> FrontendResult<()> {
    if key.is_empty() {
        return Err(FrontendError::InvalidLocale {
            message: "locale key must not be empty".to_string(),
        });
    }
    if key.contains('=') || key.contains('\n') || key.contains('\r') {
        return Err(FrontendError::InvalidLocale {
            message: format!("locale key `{key}` contains invalid characters"),
        });
    }
    if key.starts_with('[') {
        return Err(FrontendError::InvalidLocale {
            message: format!("locale key `{key}` must not start with `[`"),
        });
    }
    Ok(())
}

fn validate_cfg_value(value: &str) -> FrontendResult<()> {
    if value.contains('\n') || value.contains('\r') {
        return Err(FrontendError::InvalidLocale {
            message: "locale values must be a single line".to_string(),
        });
    }
    Ok(())
}

struct LocaleInput {
    file: Option<String>,
    languages: Vec<LanguageBlock>,
}

struct LanguageBlock {
    lang: String,
    categories: Vec<CategoryBlock>,
}

struct CategoryBlock {
    name: String,
    entries: Vec<ParsedEntry>,
}

struct ParsedEntry {
    key: LocaleKey,
    value: String,
}

enum LocaleKey {
    Path(Path),
    Literal(String),
}

impl Parse for LocaleInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut file = None;
        if input.peek(Ident) {
            let fork = input.fork();
            let kw: Ident = fork.parse()?;
            if kw == "file" && fork.peek(Token![=]) {
                let _: Ident = input.parse()?;
                let _: Token![=] = input.parse()?;
                let lit: LitStr = input.parse()?;
                file = Some(lit.value());
                let _: Option<Token![,]> = input.parse()?;
            }
        }

        let mut languages = Vec::new();
        while !input.is_empty() {
            languages.push(input.parse()?);
        }

        if languages.is_empty() {
            return Err(input.error("expected at least one language block such as `en { ... }`"));
        }

        Ok(Self { file, languages })
    }
}

impl Parse for LanguageBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let lang = if input.peek(LitStr) {
            input.parse::<LitStr>()?.value()
        } else {
            input.parse::<Ident>()?.to_string()
        };

        let content;
        syn::braced!(content in input);

        let mut categories = Vec::new();
        while !content.is_empty() {
            categories.push(content.parse()?);
        }

        Ok(Self { lang, categories })
    }
}

impl Parse for CategoryBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = if input.peek(LitStr) {
            input.parse::<LitStr>()?.value()
        } else {
            // `mod_setting_name` -> `mod-setting-name`
            let ident: Ident = input.parse()?;
            ident.to_string().replace('_', "-")
        };

        let content;
        syn::braced!(content in input);

        let mut entries = Vec::new();
        while !content.is_empty() {
            entries.push(content.parse()?);
        }

        Ok(Self { name, entries })
    }
}

impl Parse for ParsedEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key = if input.peek(LitStr) {
            LocaleKey::Literal(input.parse::<LitStr>()?.value())
        } else {
            LocaleKey::Path(input.parse()?)
        };
        let _: Token![=] = input.parse()?;
        let value: LitStr = input.parse()?;
        let _: Option<Token![,]> = input.parse()?;
        Ok(Self {
            key,
            value: value.value(),
        })
    }
}
