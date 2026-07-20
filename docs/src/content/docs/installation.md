---
title: Installation
description: Install factorio-rs-cli from crates.io and add the factorio-rs SDK to a Cargo project for Factorio modding.
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
factorio-rs = "0.2.0"
```

Optional features:

```toml
# Type-check `tracing::info!` etc.; CLI lowers them to colored game.print
factorio-rs = { version = "0.2.0", features = ["tracing"] }

# Type-check serde / serde_json; CLI lowers to helpers.table_to_json / string.pack
factorio-rs = { version = "0.2.0", features = ["serde"] }

# Both
factorio-rs = { version = "0.2.0", features = ["tracing", "serde"] }
```

See [Tracing](guides/tracing/) and [Serde / JSON](guides/serde/) for details.

`factorio-rs init` scaffolds a project with this pin, `edition = "2024"`, and
`rust-version = "1.88"` (edition 2024; let-chains in `if` / `while` require 1.88+).

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

## Community

Questions that aren’t bugs: [Discord](https://discord.gg/Tq8243rqmn). See also
[CONTRIBUTING.md](https://github.com/languint/factorio-rs/blob/main/CONTRIBUTING.md).
