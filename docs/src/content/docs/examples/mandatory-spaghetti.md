---
title: mandatory_spaghetti
description: Multi-file example with settings, locale, shared helpers, and a Lua module prefix.
---

Path: `examples/mandatory_spaghetti`.

A larger control-stage mod that demonstrates:

- path-based `control.rs` / `settings.rs`
- shared helpers as top-level files (`adjacent_blacklist.rs`, ...) - Shared by
  default
- `mod_settings!` + `locale!` (English and German)
- `[emit] lua_module_prefix = "msr"`

## Layout

```text
src/
  lib.rs                 # mod declarations
  control.rs             # Control
  settings.rs            # Settings + locale
  adjacent_blacklist.rs  # Shared
  pattern_blacklist.rs   # Shared
```

## Notable config

```toml
[emit]
lua_module_prefix = "msr"

[profiles.debug]
debug_level = 2
prune_dead_code = false
```

Build produces prefixed Lua modules (`msr_control.lua`, `msr_settings.lua`),
`settings.lua` calling `msr_settings.register()`, and
`locale/en/settings.cfg` / `locale/de/settings.cfg`.

Useful as a tour of [language support](../guides/language/): `Vec` / `for` /
`continue`, let-chains, `..Default::default()` on API param structs, and
settings reads via `.get`.
