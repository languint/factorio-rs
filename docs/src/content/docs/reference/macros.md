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

## `#[factorio_rs::export]`

Publishes a function as a cross-mod API. Control-stage exports become Factorio
`remote` interfaces; shared exports are requireable and included in the
export catalog for consumers. See [Sharing code between mods](../guides/dependencies/).

```rust
#[factorio_rs::export]
pub fn greet(name: &str) {}

#[factorio_rs::export(interface = "my_iface")]
pub fn ping() {}
```

## `mod_settings!`

See [Mod settings](../guides/mod-settings/).

## `item!`

Declare data-stage item prototypes. Expands to `Items` name constants (for
`locale!`) and `pub fn register()` that calls `data.extend`. Relative `icon`
paths become `__{package.name}__/...`.

Full field tables and stubs: [Prototypes](../guides/prototypes/).
Assets walkthrough: [Package graphics](../recipes/package-graphics/).

## `recipe!`

Declare data-stage recipe prototypes. Expands to `Recipes` name constants and
`pub fn register_recipes()` that calls `data.extend` with typed `Recipe`
literals (`type = "recipe"`; each ingredient/product injects `type = "item"`).

```rust
recipe! {
    craft_widget {
        name = "my-mod-widget",
        energy_required = 1.0,
        ingredients = [
            { name = "iron-plate", amount = 2 },
        ],
        results = [
            { name = "my-mod-widget", amount = 1 },
        ],
        category = "crafting",
        enabled = true,
    }
}
```

Required fields: `name`, `ingredients`, `results`. Optional: `energy_required`,
`category`, `enabled`, `subgroup`, `order`.

Full guide: [Prototypes](../guides/prototypes/).

## `locale!`

See [Locale](../guides/locale/).

## Expression macros

In executable code, **`println!`**, **`format!`**, **`matches!`**, (with the
`tracing` feature) **`tracing::{error,warn,info,debug,trace}!`**, and (with the
`serde` feature) **`serde_json::{to_string,from_str,...}`** calls are lowered:

- `println!(...)` -> `game.print(...)`
- `format!(...)` -> Lua string concatenation with `..`
- `matches!(expr, pat)` / `matches!(expr, pat if guard)` -> value `match` -> `true` / `false`
- `tracing::info!(...)` / `warn!` / ... -> colored `game.print` (see [Tracing](../guides/tracing/))
- `serde_json::to_string` / ... -> `helpers.table_to_json` / `string.pack` (see [Serde / JSON](../guides/serde/))

Item macros such as `mod_settings!`, `item!`, `recipe!`, and `locale!` are handled
separately during module lowering.

Full syntax inventory: [Supported Rust](../guides/language/).
