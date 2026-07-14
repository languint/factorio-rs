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
| `factorio_version` | Factorio version string (default `"2.0"`); also used for a default `base` dependency |
| `thumbnail` | Optional path to an image copied to `thumbnail.png` in the mod output |
| `dependencies` | Extra Factorio dependency strings (`"? space-age"`, `"! conflict"`, ...). Merged with deps from binding crates; this list wins on duplicate mod names. See [Sharing code between mods](../guides/dependencies/). |
| `emit_api` | **Deprecated / ignored.** Exports are written to `.factorio-rs/exports.json`. |
| `api_dir` | **Deprecated / ignored.** Exports are published onto the library’s own Cargo package. |

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

Transpile-time safety checks. Full guide: [Lints](../guides/lints/).

Each key is a lint **identifier**; the value is `allow`, `warn`, or `deny`.
Unspecified lints use their defaults (`deny`, except `format_spec` -> `warn`).

| Identifier | Code | Default | Meaning |
| --- | --- | --- | --- |
| `unwrap` | `E0001` | deny | `.unwrap()` does not check for nil in Lua |
| `expect` | `E0002` | deny | `.expect(...)` does not check for nil; message is discarded |
| `format_spec` | `E0003` | warn | Non-`?` format specs (e.g. `{:.2}`) are ignored when lowering |
| `variable_index` | `E0004` | deny | Non-literal indices are not shifted for Lua's 1-based tables |
| `identification_ctor` | `E0005` | deny | Identification enum constructors are not lowered; use `.into()` |

```toml
[lints]
unwrap = "allow"
expect = "warn"
format_spec = "allow"
```

`allow` disables the lint. `warn` prints a warning and continues. `deny` prints
an error; the build fails after all diagnostics are shown (no Lua is written).
Reports use rustc-style codes (`error[E0001]:`, `warning[E0003]:`).

## Example

```toml
source = "src"
output_dir = "dist"

[mod]
title = "My Mod"
description = "Does things"
factorio_version = "2.0"
thumbnail = "thumbnail.png"
dependencies = ["? space-age"]

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
