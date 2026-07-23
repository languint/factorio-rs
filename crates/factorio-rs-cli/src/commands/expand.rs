//! Expand macros with rustc before frontend lowering.
//!
//! Uses `-Zunpretty=expanded` (nightly-only flag). On stable toolchains we set
//! `RUSTC_BOOTSTRAP=1` so the same rustc that typechecked the crate can dump
//! the fully expanded AST - including `macro_rules!` and dependency proc macros.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    error::{CliError, CliResult},
    paths,
    write_if_changed::write_if_changed,
};

/// Expand the package library crate to a single Rust source string.
///
/// # Errors
/// Returns [`CliError::MacroExpandFailed`] when rustc/cargo fail or produce empty output.
pub fn expand_crate(project_root: &Path) -> CliResult<String> {
    // Safe when typecheck was skipped; no-op when exports are already fresh.
    super::typecheck::prepare_cargo_project(project_root)?;

    let manifest = project_root.join("Cargo.toml");

    let mut command = Command::new("cargo");
    command
        .arg("rustc")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--profile=check")
        .arg("--lib")
        .arg("--")
        .arg("-Zunpretty=expanded")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // Capture stderr for better failure hints; also print it for the user.
        .stderr(Stdio::piped());
    configure_stable_rustc_env(&mut command)?;

    let output = command.output().map_err(|source| CliError::CargoMetadata {
        message: format!("failed to run macro expansion (`cargo rustc`): {source}"),
    })?;

    if !output.stderr.is_empty() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), &output.stderr);
    }

    if !output.status.success() {
        return Err(CliError::MacroExpandFailed {
            message: macro_expand_failure_message(&output.stderr),
        });
    }

    let expanded = String::from_utf8(output.stdout).map_err(|source| CliError::CargoMetadata {
        message: format!("macro expansion output was not UTF-8: {source}"),
    })?;

    if expanded.trim().is_empty() {
        return Err(CliError::MacroExpandFailed {
            message: "rustc produced empty expansion output; factorio-rs needs `-Zunpretty=expanded` via its RUSTC_WRAPPER bootstrap helper".to_string(),
        });
    }

    Ok(expanded)
}

fn macro_expand_failure_message(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("unknown unstable option")
        || lower.contains("unstable options are only allowed")
        || lower.contains("only accepted on the nightly")
        || lower.contains("the `-z` flag is only accepted")
        || lower.contains("unpretty")
    {
        return "cargo rustc -Zunpretty=expanded failed: the toolchain rejected the expand dump. \
factorio-rs enables `RUSTC_BOOTSTRAP=1` only for that flag through its RUSTC_WRAPPER \
(see `~/.cache/factorio-rs/rustc-expand-wrapper.sh`); check that wrappers like sccache still \
exec the wrapped rustc, and see rustc diagnostics above"
            .to_string();
    }
    if lower.contains("could not exec")
        || (lower.contains("no such file") && lower.contains("wrapper"))
    {
        return "cargo rustc -Zunpretty=expanded failed: the factorio-rs RUSTC_WRAPPER could not be executed \
(see rustc diagnostics above)"
            .to_string();
    }
    "cargo rustc -Zunpretty=expanded failed (see rustc diagnostics above); \
factorio-rs requires that flag to lower macros after typecheck"
        .to_string()
}

pub fn configure_stable_rustc_env(command: &mut Command) -> CliResult<()> {
    let wrapper = ensure_rustc_expand_wrapper()?;
    command.env("RUSTC_WRAPPER", &wrapper);

    match std::env::var_os("RUSTC_WRAPPER") {
        Some(inner) if !inner.is_empty() && Path::new(&inner) != wrapper.as_path() => {
            command.env("FACTORIO_RS_INNER_RUSTC_WRAPPER", inner);
        }
        _ => {
            command.env_remove("FACTORIO_RS_INNER_RUSTC_WRAPPER");
        }
    }
    // Never propagate bootstrap to cargo's unit fingerprint
    command.env_remove("RUSTC_BOOTSTRAP");
    Ok(())
}

/// Write (or refresh) the rustc wrapper that enables bootstrap only for expand.
fn ensure_rustc_expand_wrapper() -> CliResult<PathBuf> {
    let dir = paths::factorio_rs_cache_dir()?;
    #[cfg(windows)]
    let path = dir.join("rustc-expand-wrapper.cmd");
    #[cfg(not(windows))]
    let path = dir.join("rustc-expand-wrapper.sh");

    write_if_changed(&path, rustc_expand_wrapper_script())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)
            .map_err(|source| CliError::ReadFile {
                path: path.clone(),
                source,
            })?
            .permissions();
        let mode = perms.mode();
        if mode & 0o111 == 0 {
            perms.set_mode(mode | 0o755);
            std::fs::set_permissions(&path, perms).map_err(|source| CliError::WriteFile {
                path: path.clone(),
                source,
            })?;
        }
    }

    Ok(path)
}

#[allow(clippy::literal_string_with_formatting_args)] // shell `${...}`, not format args
const fn rustc_expand_wrapper_script() -> &'static str {
    #[cfg(windows)]
    {
        "@echo off\r\n\
         setlocal EnableExtensions\r\n\
         set \"FACTORIO_RS_NEED_BOOTSTRAP=\"\r\n\
         for %%A in (%*) do (\r\n\
           if /I \"%%~A\"==\"-Zunpretty=expanded\" set \"FACTORIO_RS_NEED_BOOTSTRAP=1\"\r\n\
         )\r\n\
         if defined FACTORIO_RS_NEED_BOOTSTRAP set \"RUSTC_BOOTSTRAP=1\"\r\n\
         if defined FACTORIO_RS_INNER_RUSTC_WRAPPER (\r\n\
           \"%FACTORIO_RS_INNER_RUSTC_WRAPPER%\" %*\r\n\
         ) else (\r\n\
           %*\r\n\
         )\r\n"
    }
    #[cfg(not(windows))]
    {
        "#!/bin/sh\n\
         # @generated by factorio-rs. Enables RUSTC_BOOTSTRAP only for -Zunpretty=expanded.\n\
         for arg in \"$@\"; do\n\
           case \"$arg\" in\n\
             -Zunpretty=expanded) export RUSTC_BOOTSTRAP=1; break ;;\n\
           esac\n\
         done\n\
         if [ -n \"${FACTORIO_RS_INNER_RUSTC_WRAPPER:-}\" ]; then\n\
           exec \"$FACTORIO_RS_INNER_RUSTC_WRAPPER\" \"$@\"\n\
         fi\n\
         exec \"$@\"\n"
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{
        ensure_rustc_expand_wrapper, macro_expand_failure_message, rustc_expand_wrapper_script,
    };

    #[test]
    fn wrapper_script_mentions_unpretty_and_bootstrap() {
        let script = rustc_expand_wrapper_script();
        assert!(script.contains("unpretty=expanded"));
        assert!(script.contains("RUSTC_BOOTSTRAP"));
    }

    #[test]
    fn expand_failure_mentions_bootstrap_for_unstable_flag() {
        let message = macro_expand_failure_message(
            b"error: the option `Z` is only accepted on the nightly compiler",
        );
        assert!(message.contains("RUSTC_BOOTSTRAP"));
        assert!(message.contains("-Zunpretty=expanded"));
    }

    #[test]
    fn expand_failure_generic_still_mentions_flag() {
        let message = macro_expand_failure_message(b"error: could not compile `demo`");
        assert!(message.contains("-Zunpretty=expanded"));
    }

    #[test]
    fn ensure_wrapper_writes_executable_script() {
        let path = ensure_rustc_expand_wrapper().unwrap();
        assert!(path.is_file());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("unpretty=expanded"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0, "wrapper must be executable");
        }
    }
}
