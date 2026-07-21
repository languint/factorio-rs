---
title: Settings that change gameplay
description: Declare a startup setting, read it in control, and cover it with a test.
---

Use a settings-stage `mod_settings!` plus a control-stage read so players can
toggle behavior without editing code.

## 1. Declare the setting

`src/settings.rs` (or `settings_mod!` in `lib.rs`):

```rust
use factorio_rs::prelude::*;

factorio_rs::mod_settings! {
    prefix = "my_mod",

    startup {
        casual_mode: bool = false,
    }
}
```

This generates `Settings::CASUAL_MODE` and a `register()` entry point Factorio
calls at settings load. Details: [Mod settings](../guides/mod-settings/).

## 2. Read it in control

```rust
use factorio_rs::prelude::*;
use crate::settings::Settings;

fn casual_mode() -> bool {
    settings.startup.get_bool(Settings::CASUAL_MODE)
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    if casual_mode() {
        println!("casual mode on");
    } else {
        println!("casual mode off");
    }
}
```

## 3. Localize the label (optional)

```rust
factorio_rs::locale! {
    en {
        ["mod-setting-name.my_mod-casual-mode"] = "Casual mode",
        ["mod-setting-description.my_mod-casual-mode"] = "Softer defaults for new players.",
    }
}
```

See [Locale](../guides/locale/).

## 4. Cover the branch with a test

Startup settings are fixed for a test run’s mod config. Assert the branch you
care about after arranging the setting in Factorio’s test harness, or keep the
helper pure and unit-test the decision:

```rust
fn welcome(casual: bool) -> &'static str {
    if casual { "casual mode on" } else { "casual mode off" }
}

#[cfg(test)]
mod tests {
    use super::welcome;

    #[test]
    fn welcome_respects_flag() {
        assert_eq!(welcome(true), "casual mode on");
        assert_eq!(welcome(false), "casual mode off");
    }
}
```

For world-touching checks, use `factorio-rs test` - [Testing](../guides/testing/).

## See also

- [First hour](first-hour/) - scaffold -> build -> test loop
- [mandatory_spaghetti](../examples/mandatory-spaghetti/) - settings + locale in a larger mod
