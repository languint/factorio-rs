use std::path::{Path, PathBuf};

use crate::{
    cargo_manifest::CargoPackage,
    commands::{
        build::{BuildOptions, build},
        deploy::{DeployMode, deploy_mod, mod_dest},
    },
    config::Config,
    error::CliResult,
    paths::factorio_mods_dir,
};

/// Build the mod and copy it into the Factorio mods directory.
pub fn install(project_root: &Path, options: &BuildOptions) -> CliResult<PathBuf> {
    build(project_root, options)?;

    let package = CargoPackage::load(project_root)?;
    let config = Config::load(project_root)?;
    let output_dir = project_root.join(&config.output_dir);
    let mods_dir = factorio_mods_dir()?;
    let dest = mod_dest(&mods_dir, &package.name, &package.version);

    deploy_mod(&output_dir, &dest, DeployMode::Copy)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use crate::paths::factorio_mods_dir;

    #[test]
    fn factorio_mods_dir_defaults_to_home_factorio_mods() {
        let mods_dir = factorio_mods_dir().unwrap();
        assert!(mods_dir.ends_with(".factorio/mods"));
    }
}
