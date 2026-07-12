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

Written into `info.json` (and related packaging):

| Key | Description |
| --- | --- |
| `title` | Display title |
| `description` | Optional description |
| `factorio_version` | Factorio version string (default `"2.0"`); also used for a `base` dependency |
| `thumbnail` | Optional path to an image copied to `thumbnail.png` in the mod output |

Mod **name** / zip id still come from Cargo `[package].name` and version.

### Thumbnail

Factorio's mod portal expects `thumbnail.png` in the mod root (commonly
144×144). By default, if `thumbnail.png` exists in the project root it is
copied into `dist/` on build. Override the source path with:

```toml
[mod]
thumbnail = "assets/thumbnail.png"
```

If `thumbnail` is set and the file is missing, the build fails.

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

## `[lints]`

Transpile-time safety checks. Each key is a lint **identifier**; the value is
`allow`, `warn`, or `deny`. Unspecified lints default to **`deny`**.

| Identifier | Meaning |
| --- | --- |
| `unwrap` | `.unwrap()` does not check for nil in Lua |
| `expect` | `.expect(...)` does not check for nil; message is discarded |
| `format_spec` | Non-`?` format specs (e.g. `{:.2}`) are ignored when lowering |
| `variable_index` | Non-literal indices are not shifted for Lua's 1-based tables |
| `identification_ctor` | Identification enum constructors are not lowered; use `.into()` |

```toml
[lints]
unwrap = "allow"
expect = "warn"
format_spec = "deny"
variable_index = "deny"
identification_ctor = "deny"
```

`allow` disables the lint. `warn` prints a warning and continues. `deny` fails
the build.

## Example

```toml
source = "src"
output_dir = "dist"

[mod]
title = "My Mod"
description = "Does things"
factorio_version = "2.0"
thumbnail = "thumbnail.png"

[emit]
lua_module_prefix = "mm"

[lints]
unwrap = "allow"

[profiles.debug]
debug_level = 2
prune_dead_code = false

[profiles.release]
debug_level = 0
prune_dead_code = true
```
