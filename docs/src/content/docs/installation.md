---
title: Installation
description: Install the factorio-rs CLI and add the SDK to a Cargo project.
---

## CLI

The command-line tool is published as **`factorio-rs-cli`**. Installing it
provides the **`factorio-rs`** binary:

```bash
cargo install factorio-rs-cli
factorio-rs --help
```

## SDK dependency

In your mod’s `Cargo.toml`:

```toml
[dependencies]
factorio-rs = "0.1.1"
```

`factorio-rs init` scaffolds a project with this pin and `edition = "2024"`.

## Factorio (optional)

You only need a Factorio install for:

- `factorio-rs install` - copies `dist/` into the mods directory
- `factorio-rs open` / `install --open` - launches the game

| Purpose        | Resolution                                                                   |
| -------------- | ---------------------------------------------------------------------------- |
| Mods directory | `FACTORIO_MODS_DIR`, else `~/.factorio/mods`                                 |
| Game binary    | `FACTORIO_PATH`, common Steam paths, `factorio` on `PATH`, or Steam protocol |

On Linux, binary launches prefer `steam-run` when it is available so Steam
runtime libraries are present.
