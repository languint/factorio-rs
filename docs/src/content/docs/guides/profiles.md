---
title: Profiles
description: Configure transpile profiles for debug comments and dead-code pruning.
---

Profiles in `Factorio.toml` control **transpile** behaviour. They are not Cargo
`[profile.dev]` / `--release` profiles.

## Keys

Under `[profiles.<name>]`:

| Key | Meaning |
| --- | --- |
| `debug_level` | Lua comment verbosity (`0` = headers only; `1+` adds more inline comments) |
| `prune_dead_code` | Remove unreachable IR before codegen |

## Defaults

| Profile name | Default `debug_level` | Default `prune_dead_code` |
| --- | --- | --- |
| `debug` | `1` | `false` |
| anything else (including `release`) | unset (no debug-comment mode) unless TOML sets it | `true` |

Init templates and examples usually set release to `debug_level = 0` explicitly.

## CLI defaults

| Command | Default `--profile` |
| --- | --- |
| `build`, `install` | `debug` |
| `package` | `release` |
| `check` | _(no profile; lints come from `[lints]`)_ |

Override emit profiles with `--profile <name>` and/or `--debug-level N` on
`build` / `package` / `install`.

## What pruning keeps

Reachability starts from:

- `#[factorio_rs::event]` handlers
- `#[factorio_rs::export]` functions (cross-mod API must survive even if unused in-mod)
- public structs and enums in **shared** stage modules (library `require()` surface)
- public functions and structs (and enums) in settings/data stage modules
  (load-time entry points)

Everything else is kept only if referenced from those roots. Prefer passing
**function values** (or `lua_fn(...)`) into Factorio APIs instead of stringly
callback names when prune is on - otherwise the handler can be deleted.

Config reference: [Factorio.toml](../reference/factorio-toml/#profilesname).
