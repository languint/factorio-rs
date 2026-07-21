//! Build + deploy the mod for Bacon / hot-reload workflows.

use std::path::{Path, PathBuf};

use crate::{
    cargo_manifest::CargoPackage,
    commands::{
        build::{BuildOptions, build},
        deploy::{DeployMode, deploy_mod, mod_dest},
        hot_reload::{inject_hot_reload, note_stage_restart_if_needed},
    },
    config::Config,
    error::CliResult,
    paths::factorio_mods_dir,
    status::{self, Status},
};

/// Where to install the built mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncTarget {
    /// User Factorio mods directory (`FACTORIO_MODS_DIR` / `~/.factorio/mods`).
    Mods,
    /// Isolated `.factorio-rs/test-run/mods/` used by `factorio-rs test`.
    TestRun,
}

/// Options for [`sync`].
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub build: BuildOptions,
    pub symlink: bool,
    pub hot_reload: bool,
    pub target: SyncTarget,
}

/// Build the project, optionally inject hot-reload, and deploy to mods or test-run.
pub fn sync(project_root: &Path, options: &SyncOptions) -> CliResult<PathBuf> {
    build(project_root, &options.build)?;

    let package = CargoPackage::load(project_root)?;
    let config = Config::load(project_root)?;
    let output_dir = project_root.join(&config.output_dir);

    if options.hot_reload {
        let injected = inject_hot_reload(project_root, &output_dir, &package.name)?;
        if injected.bumped {
            status::status(
                Status::Note,
                format!("hot-reload generation {}", injected.generation),
            );
        } else {
            status::status(
                Status::Note,
                format!("hot-reload generation {} (unchanged)", injected.generation),
            );
        }
        note_stage_restart_if_needed(project_root, &output_dir)?;
    }

    let mods_dir = match options.target {
        SyncTarget::Mods => factorio_mods_dir()?,
        SyncTarget::TestRun => {
            let mods = project_root
                .join(".factorio-rs")
                .join("test-run")
                .join("mods");
            std::fs::create_dir_all(&mods).map_err(|source| crate::error::CliError::CreateDir {
                path: mods.clone(),
                source,
            })?;
            mods
        }
    };

    let dest = mod_dest(&mods_dir, &package.name, &package.version);
    let mode = if options.symlink {
        DeployMode::Symlink
    } else {
        DeployMode::Copy
    };
    let used = deploy_mod(&output_dir, &dest, mode)?;
    if options.symlink && used == DeployMode::Copy {
        status::status(
            Status::Note,
            "symlink unavailable; copied mod directory instead",
        );
    }

    Ok(dest)
}
