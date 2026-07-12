---
title: Locale
description: Author Factorio locale .cfg files from Rust with locale!.
---

Declare translations in Rust with `locale!`. On build, factorio-rs writes
`locale/<lang>/<file>.cfg` into the mod output.

Keys that reference associated constants (such as `Settings::CASUAL_MODE`) are checked by rustc and resolved to the setting’s Factorio name when assembling `.cfg` files.

```rust
factorio_rs::locale! {
    file = "settings",

    en {
        mod_setting_name {
            Settings::CASUAL_MODE = "Casual mode",
            Settings::ADJACENCY_ENABLED = "Adjacency checks",
        }
        mod_setting_description {
            Settings::CASUAL_MODE = "Relax some placement rules.",
        }
        "my-mod-messages" {
            "hello" = "Hello engineer!",
        }
    }

    de {
        mod_setting_name {
            Settings::CASUAL_MODE = "Lässig Modus",
        }
    }
}
```

## Rules

- Optional `file = "..."` - default file stem is `locale`.
- Category idents: underscores become hyphens (`mod_setting_name` ->
  `[mod-setting-name]`). Quoted category strings are used as-is.
- Keys: `Type::CONST` paths or string literals.
- Values: single-line string literals only.
- Multiple language blocks in one `locale!` are supported.

## Output

```text
dist/locale/en/settings.cfg
dist/locale/de/settings.cfg
```

```ini
[mod-setting-name]
msr-casual-mode=Casual mode
```

## Runtime messages (`LocalisedString`)

`locale!` only writes `.cfg` files. To print a translated string in-game, pass a
Factorio localised string: a plain string, or a table
`{ "category.key", arg1, ... }` where `__1__`, `__2__`, ... in the locale value
are filled from those args.

```rust
factorio_rs::locale! {
    en {
        greetings {
            "hello" = "Hello, __1__!",
        }
    }
}

player.print(["greetings.hello", player.name()], None);
// -> player.print({ "greetings.hello", player.name })
```

Arrays and plain strings implement `Into<LocalisedString>`, so they can be passed
directly to `print` / other localised parameters.

Example: [locale_test](../examples/locale-test/).
