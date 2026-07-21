//! Hot-reload generation marker and control.lua probe for `game.reload_mods()`.

use std::path::Path;

use crate::error::{CliError, CliResult};
use crate::status::{self, Status};
use crate::write_if_changed::write_if_changed;

const PROBE_MARKER: &str = "-- factorio-rs hot-reload probe";
const GEN_LUA: &str = "factorio_rs_reload_gen.lua";
const STAGE_FILES: &[&str] = &[
    "data.lua",
    "data-updates.lua",
    "data-final-fixes.lua",
    "settings.lua",
    "settings-updates.lua",
    "settings-final-fixes.lua",
];

/// Result of [`inject_hot_reload`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotReloadInject {
    pub generation: u64,
    /// `true` when the generation counter advanced (content changed).
    pub bumped: bool,
}

/// Ensure the control probe exists and write `lua/factorio_rs_reload_gen.lua`.
///
/// Generation only advances when **project sources** change (`src/**/*.rs`,
/// `Factorio.toml`, `Cargo.toml`). Rebuilding identical sources keeps the same
/// generation so Bacon/Factorio are not spuriously reloaded.
pub fn inject_hot_reload(
    project_root: &Path,
    output_dir: &Path,
    mod_name: &str,
) -> CliResult<HotReloadInject> {
    ensure_probe(output_dir, mod_name)?;

    let fingerprint = source_fingerprint(project_root)?;
    let state_dir = project_root.join(".factorio-rs");
    std::fs::create_dir_all(&state_dir).map_err(|source| CliError::CreateDir {
        path: state_dir.clone(),
        source,
    })?;

    let fp_path = state_dir.join("reload_content_fp");
    let gen_path = state_dir.join("reload_gen");
    let previous_fp = std::fs::read_to_string(&fp_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let previous_gen = std::fs::read_to_string(&gen_path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let (generation, bumped) =
        if previous_fp.as_deref() == Some(fingerprint.as_str()) && previous_gen > 0 {
            (previous_gen, false)
        } else {
            (previous_gen.saturating_add(1).max(1), true)
        };

    write_if_changed(&fp_path, &format!("{fingerprint}\n"))?;
    write_if_changed(&gen_path, &format!("{generation}\n"))?;

    let lua_dir = output_dir.join("lua");
    std::fs::create_dir_all(&lua_dir).map_err(|source| CliError::CreateDir {
        path: lua_dir.clone(),
        source,
    })?;
    let gen_lua_path = lua_dir.join(GEN_LUA);
    let gen_body =
        format!("-- factorio-rs hot-reload generation\nreturn {{ gen = {generation} }}\n");
    write_if_changed(&gen_lua_path, &gen_body)?;

    Ok(HotReloadInject { generation, bumped })
}

/// Compare data/settings stage fingerprints; note when a full Factorio restart is needed.
pub fn note_stage_restart_if_needed(project_root: &Path, output_dir: &Path) -> CliResult<()> {
    let fingerprint = stage_fingerprint(output_dir)?;
    let state_path = project_root.join(".factorio-rs").join("sync_stage_fp");
    let previous = std::fs::read_to_string(&state_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    write_if_changed(&state_path, &format!("{fingerprint}\n"))?;

    if fingerprint.is_empty() {
        return Ok(());
    }

    if previous.as_deref() != Some(fingerprint.as_str()) {
        status::status(
            Status::Note,
            "data/settings stage changed - restart Factorio to apply prototypes/settings \
             (control hot-reload cannot)",
        );
    }
    Ok(())
}

fn ensure_probe(output_dir: &Path, mod_name: &str) -> CliResult<()> {
    let control_path = output_dir.join("control.lua");
    let mut control =
        std::fs::read_to_string(&control_path).map_err(|source| CliError::ReadFile {
            path: control_path.clone(),
            source,
        })?;
    if let Some(idx) = control.find(PROBE_MARKER) {
        control.truncate(idx);
        while control.ends_with('\n') {
            control.pop();
        }
        control.push('\n');
    }
    control.push('\n');
    control.push_str(&generate_probe_lua(mod_name));
    write_if_changed(&control_path, &control)?;
    Ok(())
}

/// Hash project inputs that should trigger a Factorio control hot-reload.
fn source_fingerprint(project_root: &Path) -> CliResult<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    let mut paths = Vec::new();

    for name in ["Factorio.toml", "Cargo.toml"] {
        let path = project_root.join(name);
        if path.is_file() {
            paths.push(path);
        }
    }

    let source_dir = crate::config::Config::load(project_root).map_or_else(
        |_| project_root.join("src"),
        |config| project_root.join(&config.source),
    );
    if source_dir.is_dir() {
        collect_rust_sources(&source_dir, &mut paths)?;
    }

    paths.sort();
    for path in &paths {
        let relative = path.strip_prefix(project_root).unwrap_or(path.as_path());
        relative.hash(&mut hasher);
        let bytes = std::fs::read(path).map_err(|source| CliError::ReadFile {
            path: path.clone(),
            source,
        })?;
        hasher.write(&bytes);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> CliResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|source| CliError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CliError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
            && path
                .file_name()
                .is_some_and(|name| name != "factorio_exports.rs")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn stage_fingerprint(output_dir: &Path) -> CliResult<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    let mut any = false;
    for name in STAGE_FILES {
        let path = output_dir.join(name);
        if !path.is_file() {
            continue;
        }
        any = true;
        name.hash(&mut hasher);
        let bytes = std::fs::read(&path).map_err(|source| CliError::ReadFile {
            path: path.clone(),
            source,
        })?;
        hasher.write(&bytes);
    }
    if any {
        Ok(format!("{:016x}", hasher.finish()))
    } else {
        Ok(String::new())
    }
}

fn generate_probe_lua(mod_name: &str) -> String {
    format!(
        r#"{PROBE_MARKER}
do
  local gen_path = "__{mod_name}__/lua/factorio_rs_reload_gen"
  script.on_nth_tick(15, function()
    if not game or not game.reload_mods then
      return
    end
    package.loaded[gen_path] = nil
    local ok, mod = pcall(require, gen_path)
    if not ok or type(mod) ~= "table" or mod.gen == nil then
      return
    end
    local gen = mod.gen
    if storage.__frs_reload_gen == nil then
      storage.__frs_reload_gen = gen
      return
    end
    if storage.__frs_reload_gen ~= gen then
      storage.__frs_reload_gen = gen
      game.reload_mods()
    end
  end)
end
"#
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn inject_bumps_only_when_sources_change() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let out = root.join("dist");
        let src = root.join("src");
        std::fs::create_dir_all(out.join("lua")).unwrap();
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            root.join("Factorio.toml"),
            "source = \"src\"\noutput_dir = \"dist\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(src.join("lib.rs"), "fn main() {}\n").unwrap();
        std::fs::write(out.join("control.lua"), "-- base\n").unwrap();

        let first = inject_hot_reload(root, &out, "demo").unwrap();
        assert_eq!(first.generation, 1);
        assert!(first.bumped);

        let second = inject_hot_reload(root, &out, "demo").unwrap();
        assert_eq!(second.generation, 1);
        assert!(!second.bumped);

        // Rebuilding dist alone must not bump.
        std::fs::write(out.join("control.lua"), "-- rebuilt\n").unwrap();
        let third = inject_hot_reload(root, &out, "demo").unwrap();
        assert_eq!(third.generation, 1);
        assert!(!third.bumped);

        std::fs::write(src.join("lib.rs"), "fn main() { /* changed */ }\n").unwrap();
        let fourth = inject_hot_reload(root, &out, "demo").unwrap();
        assert_eq!(fourth.generation, 2);
        assert!(fourth.bumped);
    }
}
