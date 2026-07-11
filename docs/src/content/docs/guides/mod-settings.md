---
title: Mod settings
description: Declare Factorio mod settings with mod_settings!.
---

Use `mod_settings!` in a **settings-stage** module to define startup or runtime
settings. The macro expands to:

- a `Settings` type with `pub const` name strings
- `pub fn register()` that extends Factorio `data` with setting prototypes

```rust
use factorio_rs::prelude::*;

factorio_rs::mod_settings! {
    prefix = "msr",

    startup {
        casual_mode: bool = false,
        adjacency_enabled: bool = true,
    }

    // runtime_global { ... }
    // runtime_per_user { ... }
}
```

## Naming

| Rust field | Const | Factorio setting name |
| --- | --- | --- |
| `casual_mode` | `Settings::CASUAL_MODE` | `msr-casual-mode` (with `prefix = "msr"`) |

Stage keywords use underscores (`runtime_global`). Generated Lua uses Factorio’s hyphenated strings (`"runtime-global"`).

## Types

| Rust type | Prototype |
| --- | --- |
| `bool` | bool-setting |
| integer types (`i32`, `u64`, ...) | int-setting |
| `f32` / `f64` | double-setting |
| `&str` / `String` | string-setting |

## Reading in control

```rust
use factorio_rs::prelude::*;
use crate::settings::Settings;

const CASUAL_MODE: bool = settings.startup.get::<bool>(Settings::CASUAL_MODE);
```

Put locale strings for setting names/descriptions in a `locale!` block - see [Locale](locale/).

## Reading values in Lua terms

```rust
settings.startup.get::<bool>(Settings::CASUAL_MODE)
```

lowers roughly to `settings.startup["msr-casual-mode"].value`.

Setting / API structs often use `..Default::default()` so only the fields you set appear in the Lua table - see [Language support](language/).
