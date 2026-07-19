#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleEntry {
    /// Factorio category name (e.g. `mod-setting-name`), if any.
    pub category: Option<String>,
    /// Locale key (e.g. `msr-casual-mode`).
    pub key: String,
    /// Translated / English template string.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleFile {
    /// Language code (`en`, `de`, ...).
    pub lang: String,
    /// File stem written under `locale/<lang>/` (without `.cfg`).
    pub file: String,
    pub entries: Vec<LocaleEntry>,
}

/// Unresolved locale key from `locale!` (literal or `Type::CONST` / FQ path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingLocaleKey {
    Literal(String),
    /// Path segments as written (`Items`, `WIDGET`) or
    /// (`crate`, `data`, `items`, `Items`, `WIDGET`).
    Path(Vec<String>),
}

/// One unresolved entry before string-const resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLocaleEntry {
    pub category: Option<String>,
    pub key: PendingLocaleKey,
    pub value: String,
}

/// Parsed `locale!` language block awaiting `Type::CONST` resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLocaleFile {
    pub lang: String,
    pub file: String,
    pub entries: Vec<PendingLocaleEntry>,
}

impl LocaleFile {
    /// Serialize to Factorio's `.cfg` format.
    #[must_use]
    pub fn to_cfg(&self) -> String {
        let mut output = String::new();
        let mut current_category: Option<&str> = None;
        let mut first_section = true;

        for entry in &self.entries {
            let category = entry.category.as_deref();
            if category != current_category {
                if !first_section {
                    output.push('\n');
                }
                if let Some(name) = category {
                    output.push('[');
                    output.push_str(name);
                    output.push_str("]\n");
                }
                current_category = category;
                first_section = false;
            }

            output.push_str(&entry.key);
            output.push('=');
            output.push_str(&entry.value);
            output.push('\n');
        }

        output
    }
}
