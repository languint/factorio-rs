---
title: Tracing
description: Optional tracing macros lowered to colored Factorio chat messages.
---

factorio-rs can lower the `tracing` crate’s level macros to colored
`game.print` calls. There is **no Rust tracing runtime** in Factorio - this is
compile-time lowering, the same idea as `println!`.

## Enable the feature

In your mod `Cargo.toml`:

```toml
[dependencies]
factorio-rs = { version = "0.3.1", features = ["tracing"] }
```

That pulls in the `tracing` crate so macros type-check under `cargo check`.

The **CLI** lowers these macros by default (`factorio-rs-cli` feature `tracing`).
If you built the CLI from source with `--no-default-features`, reinstall with
defaults enabled:

```bash
cargo install factorio-rs-cli
# or from a checkout:
cargo install --path crates/factorio-rs-cli --force
```

## Usage

```rust
factorio_rs::control_mod! {
    use factorio_rs::tracing;

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        tracing::info!("Hello factorio-rs!");
        let item = "iron-plate";
        tracing::warn!("missing {item}");
        tracing::error!("Oopsies!");
    }
}
```

With the feature enabled, `factorio_rs::prelude::*` also re-exports `info!`,
`warn!`, `error!`, `debug!`, and `trace!`, so bare `info!("...")` works after
`use factorio_rs::prelude::*`.

Supported paths:

| Form | Example |
| --- | --- |
| Qualified | `tracing::info!("...")`, `factorio_rs::tracing::warn!("...")` |
| Bare (feature on) | `info!("...")` after importing the macro |

## What it emits

Each call becomes `game.print` with a level prefix and a `PrintSettings` color:

```lua
game.print("[INFO] Hello factorio-rs!", { color = { r = 0.55, g = 0.85, b = 1, a = 1 } })
game.print("[ERROR] Oopsies!", { color = { r = 1, g = 0.25, b = 0.25, a = 1 } })
```

| Macro | Prefix | Color (approx.) |
| --- | --- | --- |
| `error!` | `[ERROR]` | Red |
| `warn!` | `[WARN]` | Amber |
| `info!` | `[INFO]` | Light blue |
| `debug!` | `[DEBUG]` | Gray |
| `trace!` | `[TRACE]` | Darker gray |

## Format strings

Same subset as `println!` / `format!`:

- `{}`, `{0}`, `{name}`
- `{:?}` / `{:#?}` / `{name:?}` -> `helpers.table_to_json(v)` for known table types, otherwise `tostring(v)` (chosen at compile time from the Rust type)
- `{{` / `}}` escapes
- Other format specs after `:` (e.g. `{:.2}`) are ignored

```rust
tracing::info!("built {:?}", event.entity);
// -> game.print("[INFO] built " .. tostring(event.entity), { color = ... })
```

**Not supported:** structured fields (`info!(foo = 1, "hi")`), `target:`, spans,
or runtime level filtering. Every lowered call always prints in-game.

## Features (summary)

| Crate | Feature | Purpose |
| --- | --- | --- |
| `factorio-rs` | `tracing` | Optional `tracing` dependency + prelude re-exports for type-checking |
| `factorio-rs-cli` | `tracing` (default) | Enable frontend lowering when building mods |
| `factorio-frontend` | `tracing` | Implementation of the macro lowering |

## See also

- Example: [tracing_test](../examples/tracing-test/)
- [Supported Rust](language/) - expression macros
- [Macros and attributes](../reference/macros/)
