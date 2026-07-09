#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Control,
    Shared,
    Data,
}

impl Stage {
    #[must_use]
    pub fn from_module_name(module_name: &str) -> Option<Self> {
        if module_name == "control" || module_name.starts_with("control.") {
            return Some(Self::Control);
        }
        if module_name == "shared" || module_name.starts_with("shared.") {
            return Some(Self::Shared);
        }
        if module_name == "data" || module_name.starts_with("data.") {
            return Some(Self::Data);
        }
        None
    }

    #[must_use]
    pub fn default_module_name(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Shared => "shared",
            Self::Data => "data",
        }
    }
}
