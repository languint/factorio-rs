---
title: Getting started
description: Create a factorio-rs project and produce a loadable Factorio mod.
---

## Create a project

```bash
mkdir my-mod && cd my-mod
factorio-rs init --name my-mod
```

`init` refuses to overwrite an existing `Cargo.toml` or `Factorio.toml`.

### What gets created

| Path            | Purpose                                  |
| --------------- | ---------------------------------------- |
| `Cargo.toml`    | Library crate depending on `factorio-rs` |
| `Factorio.toml` | Transpile + mod metadata                 |
| `src/lib.rs`    | Sample `control_mod!` with one event     |
| `.gitignore`    | Ignores `/target`, `/dist`, `/.factorio-rs/exports.json`, `/*.zip` |

Default `src/lib.rs`:

```rust
factorio_rs::control_mod! {
    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("Initialized");
    }
}
```

## Configure

Minimal `Factorio.toml`:

```toml
source = "src"
output_dir = "dist"

[mod]
title = "Factorio Mod"
factorio_version = "2.0"

[profiles.debug]
debug_level = 1
prune_dead_code = false

[profiles.release]
debug_level = 0
prune_dead_code = true
```

:::note
The Factorio mod **id** comes from **`[package].name` in
`Cargo.toml`**, not from `[mod].title`. Title is the display name in
`info.json`.
:::

## Build

```bash
factorio-rs check    # cargo check + transpile lints (no output)
factorio-rs build    # typecheck, then transpile into dist/
```

Typical output:

```text
dist/
  info.json
  control.lua
  lua/control.lua
```

Each build **wipes** `output_dir` before writing.

## Install and play

```bash
factorio-rs install           # copy dist/ -> mods/<name>_<version>/
factorio-rs install --open    # install, then launch Factorio
factorio-rs package           # release profile and zip at project root
```

Zip name: `{cargo_package_name}_{version}.zip` with a Factorio-ready root
folder inside.

Place an optional `thumbnail.png` in the project root (or set
`[mod].thumbnail`) so it is copied into the mod for the Factorio mod portal.

## Next steps

- [Language support](language/) - what Rust syntax you can use
- [Option and Result](option-and-result/) - nil / errors / `?`
- [Lints](lints/) - transpile safety diagnostics (`E0001` ...)
- [Stages](stages/) - control / settings / data / shared
- [Events and filters](events/)
- [hello_world](../examples/hello-world/) example
