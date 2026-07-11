use std::collections::BTreeMap;

use factorio_ir::{
    expression::Expression,
    literal::Literal,
    locale::{LocaleEntry, LocaleFile},
    statement::Statement,
};
use proc_macro2::TokenStream;
use syn::{
    Ident, LitStr, Path, Token,
    parse::{Parse, ParseStream},
};

use crate::error::{FrontendError, FrontendResult};

/// Parse a `locale!` invocation into locale files, resolving `Type::CONST` keys
/// against associated string constants already lowered in this module.
pub fn expand(
    tokens: TokenStream,
    const_strings: &BTreeMap<String, String>,
) -> FrontendResult<Vec<LocaleFile>> {
    let input: LocaleInput =
        syn::parse2(tokens).map_err(|e| FrontendError::Syn(e.to_string()))?;

    let file = input.file.unwrap_or_else(|| "locale".to_string());
    let mut files = Vec::new();

    for lang_block in input.languages {
        let mut entries = Vec::new();
        for category in lang_block.categories {
            for entry in category.entries {
                let key = resolve_key(&entry.key, const_strings)?;
                validate_cfg_key(&key)?;
                validate_cfg_value(&entry.value)?;
                entries.push(LocaleEntry {
                    category: Some(category.name.clone()),
                    key,
                    value: entry.value,
                });
            }
        }
        files.push(LocaleFile {
            lang: lang_block.lang,
            file: file.clone(),
            entries,
        });
    }

    Ok(files)
}

/// Collect `StructName::CONST` → string value from lowered module statements.
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

fn resolve_key(key: &LocaleKey, const_strings: &BTreeMap<String, String>) -> FrontendResult<String> {
    match key {
        LocaleKey::Literal(s) => Ok(s.clone()),
        LocaleKey::Path(path) => {
            let path_str = path_to_string(path);
            const_strings
                .get(&path_str)
                .cloned()
                .ok_or(FrontendError::LocaleKeyUnresolved { path: path_str })
        }
    }
}

fn path_to_string(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
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
            // `mod_setting_name` → `mod-setting-name`
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
