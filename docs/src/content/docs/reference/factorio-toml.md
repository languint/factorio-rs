---
title: Factorio.toml
description: Project configuration keys for factorio-rs.
---

`Factorio.toml` sits next to `Cargo.toml` in the project root.

## Top level

| Key | Default | Description |
| --- | --- | --- |
| `source` | `"src"` | Root of Rust sources to transpile |
| `output_dir` | `"dist"` | Mod output directory (wiped each build) |

## `[mod]`

Written into `info.json`:

| Key | Description |
| --- | --- |
| `title` | Display title |
| `description` | Optional description |
| `factorio_version` | Factorio version string (default `"2.0"`); also used for a `base` dependency |

Mod **name** / zip id still come from Cargo `[package].name` and version.

## `[emit]`

| Key | Description |
| --- | --- |
| `lua_module_prefix` | Prefix applied to the last segment of Lua module paths (e.g. `"msr"` -> `msr_control.lua`) |

## `[profiles.<name>]`

| Key | Description |
| --- | --- |
| `debug_level` | Lua debug comment level |
| `prune_dead_code` | Whether to prune unreachable IR |

See [Profiles](../guides/profiles/).

## Example

```toml
source = "src"
output_dir = "dist"

[mod]
title = "My Mod"
description = "Does things"
factorio_version = "2.0"

[emit]
lua_module_prefix = "mm"

[profiles.debug]
debug_level = 2
prune_dead_code = false

[profiles.release]
debug_level = 0
prune_dead_code = true
```
