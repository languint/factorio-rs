---
title: Macros and attributes
description: factorio-rs proc macros and stage attributes.
---

Import via `factorio_rs` or `factorio_rs::prelude::*`.

## Stage attributes

Applied to a crate, module, or used as `*_mod!` wrappers:

| Attribute / macro | Stage |
| --- | --- |
| `#[factorio_rs::control]` / `control_mod!` | Control |
| `#[factorio_rs::settings]` / `settings_mod!` | Settings |
| `#[factorio_rs::settings_updates]` / `settings_updates_mod!` | Settings updates |
| `#[factorio_rs::settings_final_fixes]` / `settings_final_fixes_mod!` | Settings final fixes |
| `#[factorio_rs::data]` / `data_mod!` | Data |
| `#[factorio_rs::data_updates]` / `data_updates_mod!` | Data updates |
| `#[factorio_rs::data_final_fixes]` / `data_final_fixes_mod!` | Data final fixes |
| `#[factorio_rs::shared]` / `shared_mod!` | Shared |

`*_mod! { ... }` wraps items in a hidden module marked for that stage (useful from
`lib.rs`).

## `#[factorio_rs::event]`

See [Events and filters](../guides/events/).

```rust
#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {}

#[factorio_rs::event(filter = [OnBuiltEntityFilter::name("inserter")])]
pub fn on_built_entity(event: OnBuiltEntityEvent) {}
```

## `mod_settings!`

See [Mod settings](../guides/mod-settings/).

## `locale!`

See [Locale](../guides/locale/).

## Expression macros

In executable code, **`println!`**, **`format!`**, and (with the `tracing`
feature) **`tracing::{error,warn,info,debug,trace}!`** are lowered:

- `println!(...)` → `game.print(...)`
- `format!(...)` → Lua string concatenation with `..`
- `tracing::info!(...)` / `warn!` / ... → colored `game.print` (see [Tracing](../guides/tracing/))

Item macros such as `mod_settings!` and `locale!` are handled separately during
module lowering.

Full syntax inventory: [Language support](../guides/language/).
