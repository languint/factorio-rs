use std::path::{Path, PathBuf};

use crate::error::{CliError, CliResult};

/// Steam app id for Factorio.
pub const FACTORIO_STEAM_APP_ID: u32 = 427_520;

/// Resolved way to launch Factorio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactorioLaunchTarget {
    /// Path to the Factorio executable.
    ///
    /// When `steam_run` is true, the binary is launched via `steam-run` so
    /// Steam runtime libraries are available.
    Binary { path: PathBuf, steam_run: bool },
    /// Launch via the Steam client (`steam://rungameid/...`).
    Steam,
}

impl FactorioLaunchTarget {
    pub fn display(&self) -> String {
        match self {
            Self::Binary {
                path,
                steam_run: true,
            } => format!("steam-run {}", path.display()),
            Self::Binary {
                path,
                steam_run: false,
            } => path.display().to_string(),
            Self::Steam => format!("steam://rungameid/{FACTORIO_STEAM_APP_ID}"),
        }
    }
}

/// Resolve the Factorio mods directory.
///
/// Order: `FACTORIO_MODS_DIR`, then `~/.factorio/mods`.
pub fn factorio_mods_dir() -> CliResult<PathBuf> {
    if let Ok(path) = std::env::var("FACTORIO_MODS_DIR") {
        return Ok(PathBuf::from(path));
    }

    let home = home_dir().ok_or_else(|| CliError::FactorioNotFound {
        hint: "set FACTORIO_MODS_DIR or ensure HOME is set".to_string(),
    })?;

    Ok(home.join(".factorio/mods"))
}

/// Locate a Factorio install that can be launched.
///
/// Binary launches prefer `steam-run` when available so Steam runtime libraries
/// are present (required for the Steam build of Factorio on many Linux setups).
/// Set `FACTORIO_RS_NO_STEAM_RUN=1` to force a direct binary launch (useful for
/// tests and non-Steam installs).
pub fn find_factorio() -> CliResult<FactorioLaunchTarget> {
    let steam_run = find_on_path("steam-run").is_some() && !no_steam_run();

    if let Ok(path) = std::env::var("FACTORIO_PATH") {
        let path = resolve_factorio_path(PathBuf::from(path))?;
        return Ok(FactorioLaunchTarget::Binary { path, steam_run });
    }

    for candidate in candidate_binaries() {
        if is_executable(&candidate) {
            return Ok(FactorioLaunchTarget::Binary {
                path: candidate,
                steam_run,
            });
        }
    }

    if let Some(path) = find_on_path(factorio_binary_name()) {
        return Ok(FactorioLaunchTarget::Binary { path, steam_run });
    }

    if find_on_path("steam").is_some() {
        return Ok(FactorioLaunchTarget::Steam);
    }

    Err(CliError::FactorioNotFound {
        hint: format!(
            "set FACTORIO_PATH to the Factorio binary or install root, \
             or install Steam (app id {FACTORIO_STEAM_APP_ID})"
        ),
    })
}

fn resolve_factorio_path(path: PathBuf) -> CliResult<PathBuf> {
    if is_executable(&path) {
        return Ok(path);
    }

    let nested = path.join("bin").join("x64").join(factorio_binary_name());
    if is_executable(&nested) {
        return Ok(nested);
    }

    Err(CliError::FactorioNotFound {
        hint: format!(
            "`FACTORIO_PATH` (`{}`) is not an executable and does not contain `bin/x64/{}`",
            path.display(),
            factorio_binary_name()
        ),
    })
}

fn candidate_binaries() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let Some(home) = home_dir() else {
        return candidates;
    };

    let steam_roots = [
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join("Library/Application Support/Steam"),
    ];

    for root in steam_roots {
        candidates.push(
            root.join("steamapps/common/Factorio/bin/x64")
                .join(factorio_binary_name()),
        );
    }

    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from(
            "/Applications/factorio.app/Contents/MacOS/factorio",
        ));
        candidates.push(home.join("Applications/factorio.app/Contents/MacOS/factorio"));
    }

    #[cfg(target_os = "windows")]
    {
        for program_files in ["PROGRAMFILES(X86)", "PROGRAMFILES", "ProgramFiles"] {
            if let Ok(base) = std::env::var(program_files) {
                candidates.push(
                    PathBuf::from(base)
                        .join("Steam/steamapps/common/Factorio/bin/x64")
                        .join(factorio_binary_name()),
                );
                candidates.push(
                    PathBuf::from(base)
                        .join("Factorio/bin/x64")
                        .join(factorio_binary_name()),
                );
            }
        }
    }

    candidates
}

const fn factorio_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "factorio.exe"
    } else {
        "factorio"
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(name);
        is_executable(&candidate).then_some(candidate)
    })
}

fn no_steam_run() -> bool {
    matches!(
        std::env::var("FACTORIO_RS_NO_STEAM_RUN").as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{FactorioLaunchTarget, resolve_factorio_path};

    #[test]
    fn resolve_factorio_path_accepts_direct_binary() {
        let temp = tempfile::TempDir::new().unwrap();
        let binary = temp.path().join("factorio");
        std::fs::write(&binary, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&binary).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&binary, perms).unwrap();
        }

        let resolved = resolve_factorio_path(binary.clone()).unwrap();
        assert_eq!(resolved, binary);
    }

    #[test]
    fn resolve_factorio_path_accepts_install_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let binary = temp
            .path()
            .join("bin/x64")
            .join(super::factorio_binary_name());
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&binary).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&binary, perms).unwrap();
        }

        let resolved = resolve_factorio_path(temp.path().to_path_buf()).unwrap();
        assert_eq!(resolved, binary);
    }

    #[test]
    fn launch_target_display_for_steam() {
        assert_eq!(
            FactorioLaunchTarget::Steam.display(),
            "steam://rungameid/427520"
        );
    }
}
