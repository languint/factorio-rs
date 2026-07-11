use std::process::{Command, Stdio};

use crate::{
    error::{CliError, CliResult},
    paths::{FACTORIO_STEAM_APP_ID, FactorioLaunchTarget, find_factorio},
};

/// Locate Factorio on this system and launch it.
pub fn open() -> CliResult<FactorioLaunchTarget> {
    let target = find_factorio()?;
    launch(&target)?;
    Ok(target)
}

fn launch(target: &FactorioLaunchTarget) -> CliResult<()> {
    let mut command = match target {
        FactorioLaunchTarget::Binary {
            path,
            steam_run: true,
        } => {
            let mut command = Command::new("steam-run");
            command.arg(path);
            command
        }
        FactorioLaunchTarget::Binary {
            path,
            steam_run: false,
        } => Command::new(path),
        FactorioLaunchTarget::Steam => {
            let mut command = Command::new("steam");
            command.arg(format!("steam://rungameid/{FACTORIO_STEAM_APP_ID}"));
            command
        }
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| CliError::LaunchFactorio {
            target: target.display(),
            source,
        })?;

    Ok(())
}
