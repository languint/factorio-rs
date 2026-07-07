use std::path::{Path, PathBuf};

/// Returns the dotted module name for a source file relative to a source root, e.g.
/// - `src/player/extra_info.rs` -> `"player.extra_info"`
/// - `src/player/mod.rs` -> `"player"`
pub fn module_name_from_source(source_dir: &Path, source_path: &Path) -> Option<String> {
    let relative = source_path.strip_prefix(source_dir).ok()?;

    if relative.file_name().is_some_and(|name| name == "lib.rs") {
        return None;
    }

    if relative.file_name().is_some_and(|name| name == "mod.rs") {
        let parent = relative.parent()?;
        if parent.as_os_str().is_empty() {
            return None;
        }
        return Some(path_to_module_name(parent));
    }

    Some(path_to_module_name(&relative.with_extension("")))
}

fn path_to_module_name(path: &Path) -> String {
    path.iter()
        .map(|component| component.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(".")
}

/// Returns the output Lua path for a dotted module name.
pub fn lua_output_path(output_dir: &Path, module_name: &str) -> PathBuf {
    let mut path = output_dir.to_path_buf();
    for segment in module_name.split('.') {
        path.push(segment);
    }
    path.set_extension("lua");
    path
}

/// Returns a valid Lua identifier for a required module path.
pub fn require_local_name(module_path: &str) -> String {
    module_path.replace('.', "_")
}

/// Splits a crate-relative path into a dotted module path and remaining item segments.
pub fn split_crate_path(segments: &[String]) -> (String, Vec<String>) {
    if segments.is_empty() {
        return (String::new(), Vec::new());
    }

    let mut module_parts = Vec::new();
    let mut item_start = segments.len();

    for (index, segment) in segments.iter().enumerate() {
        if starts_with_uppercase(segment) {
            item_start = index;
            break;
        }
        module_parts.push(segment.as_str());
    }

    if module_parts.is_empty() {
        module_parts.push(segments[0].as_str());
        item_start = 1;
    }

    (module_parts.join("."), segments[item_start..].to_vec())
}

fn starts_with_uppercase(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_nested_source_paths_to_module_names() {
        let source_dir = Path::new("/project/src");

        assert_eq!(
            module_name_from_source(source_dir, Path::new("/project/src/player/extra_info.rs")),
            Some("player.extra_info".to_string())
        );
        assert_eq!(
            module_name_from_source(source_dir, Path::new("/project/src/player/mod.rs")),
            Some("player".to_string())
        );
    }

    #[test]
    fn splits_crate_paths_into_module_and_item_segments() {
        assert_eq!(
            split_crate_path(&[
                "player".to_string(),
                "extra_info".to_string(),
                "MyType".to_string(),
            ]),
            ("player.extra_info".to_string(), vec!["MyType".to_string()])
        );
        assert_eq!(
            split_crate_path(&[
                "player".to_string(),
                "MyPlayer".to_string(),
                "new".to_string(),
            ]),
            (
                "player".to_string(),
                vec!["MyPlayer".to_string(), "new".to_string()]
            )
        );
        assert_eq!(
            split_crate_path(&["player".to_string(), "extra_info".to_string()]),
            ("player.extra_info".to_string(), Vec::new())
        );
    }
}
