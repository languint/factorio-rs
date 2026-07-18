---
title: CLI
description: Reference for the factorio-rs command-line interface.
---

Binary name: **`factorio-rs`** (from crate **`factorio-rs-cli`**).

## Commands

### `factorio-rs init`

Create a new project in the current directory (or `--manifest-path`).

| Flag | Description |
| --- | --- |
| `--name <NAME>` | Cargo package name (default: directory name) |
| `--manifest-path <PATH>` | Project directory or `Factorio.toml` |

### `factorio-rs check`

Run **`cargo check`** (Factorio API stubs + deps), then lower every module and
apply transpile lints - **without** writing `output_dir`.

| Flag | Description |
| --- | --- |
| `--manifest-path <PATH>` | Project directory or `Factorio.toml` |
| `--skip-typecheck` | Skip `cargo check`; only validate lowering / lints |

Lint levels come from `[lints]` in `Factorio.toml` (not from build profiles).

### `factorio-rs build`

Typecheck, then transpile `source` into `output_dir`.

| Flag | Description |
| --- | --- |
| `--manifest-path <PATH>` | Project directory or `Factorio.toml` |
| `--profile <NAME>` | Default: `debug` |
| `--debug-level <N>` | Override profile debug comments |
| `--package` | Also write `{name}_{version}.zip` after building |
| `--skip-typecheck` | Skip `cargo check` before transpile |

### `factorio-rs package`

Build then create a Factorio-ready zip at the project root.

| Flag | Description |
| --- | --- |
| `--manifest-path <PATH>` | Project directory or `Factorio.toml` |
| `--profile <NAME>` | Default: `release` |
| `--debug-level <N>` | Override profile debug comments |
| `--skip-typecheck` | Skip `cargo check` before transpile |

### `factorio-rs install`

Build and copy `output_dir` to `{mods_dir}/{name}_{version}/`.

| Flag | Description |
| --- | --- |
| `--manifest-path <PATH>` | Project directory or `Factorio.toml` |
| `--profile <NAME>` | Default: `debug` |
| `--debug-level <N>` | Override profile debug comments |
| `--open` | Launch Factorio after installing |
| `--skip-typecheck` | Skip `cargo check` before transpile |

Mods directory: `FACTORIO_MODS_DIR` or `~/.factorio/mods`.

### `factorio-rs add`

Add another factorio-rs library as a Cargo path dependency and merge Factorio.toml
deps. See [Sharing code between mods](../guides/dependencies/).

### `factorio-rs open`

Launch Factorio if detected (`FACTORIO_PATH`, Steam installs, PATH, or Steam
protocol). Prefers `steam-run` when available.

### `factorio-rs test`

Build the mod, discover `#[test]` functions under `#[cfg(test)]`, launch Factorio
(headless by default), run the suite in-game, and print a colored report.

See [Testing](../guides/testing/).

| Flag | Description |
| --- | --- |
| `--manifest-path <PATH>` | Project directory or `Factorio.toml` |
| `--profile <NAME>` | Default: `debug` |
| `--debug-level <N>` | Override profile debug comments |
| `[FILTER]` | Only run tests whose name contains this substring |
| `--skip-typecheck` | Skip `cargo check --tests` |
| `--gui` | Open a Factorio window; stays open after the suite |
| `--timeout <SECS>` | Kill Factorio if the suite does not finish (default: 120) |

Requires a Factorio binary (`FACTORIO_PATH` recommended). Steam protocol-only
installs are not supported for testing.
