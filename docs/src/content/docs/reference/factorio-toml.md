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
| `assets` | Files/directories copied into the mod output (graphics, sounds, ...). See [Assets](#assets). |
| `dependencies` | Extra Factorio dependency strings (`"? space-age"`, `"! conflict"`, ...). Merged with deps from Cargo crates that publish `[package.metadata.factorio]`; this list wins on duplicate mod names. See [Sharing code between mods](/guides/dependencies/). |
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

### Assets

Extra mod files (sprites, sounds, icons, ...) are copied from the project into
`output_dir` on each build. List paths under `[mod].assets`:

```toml
[mod]
# Keep the same relative path in the mod (dist/graphics/...)
assets = ["graphics", "sounds"]

# Or remap when sources live under assets/
assets = [
  { from = "assets/graphics", to = "graphics" },
  { from = "assets/sounds", to = "sounds" },
  { from = "assets/extra.png", to = "graphics/extra.png" },
]
```

Rules:

- Paths are relative to the project root; every listed source must exist.
- Directories are copied recursively; files copy to the destination path.
- Destinations must stay inside the mod output (no `..` or absolute paths).
- Destinations must not collide with generated layout: `info.json`, stage entry
  Lua (`control.lua`, `data*.lua`, `settings*.lua`), `lua/`, `locale/`, or
  `thumbnail.png` (use `[mod].thumbnail` for the portal thumbnail).

Reference packaged files in data-stage code with Factorio paths such as
`"__my_mod__/graphics/icon.png"` (replace `my_mod` with your Cargo package name),
or use `item!` with a relative `icon` path. End-to-end: [Package graphics](/recipes/package-graphics/).

## `[emit]`

| Key | Description |
| --- | --- |
| `lua_module_prefix` | Prefix applied to the last segment of Lua module paths (e.g. `"msr"` -> `msr_control.lua`) |

## `[profiles.<name>]`

| Key | Description |
| --- | --- |
| `debug_level` | Lua debug comment level |
| `prune_dead_code` | Whether to prune unreachable IR |
| `optimize_ir` | Whether to run IR expression optimizations before codegen |

See [Profiles](/guides/profiles/).

## `[lints]`

Transpile-time safety checks. See [Lints](/guides/lints/).

Each key is a lint **identifier**; the value is `allow`, `warn`, or `deny`.
Unspecified lints use their defaults (`deny`, except `format_spec` /
`integer_div` / `numeric_cast` / `storage_index` -> `warn`).

| Identifier | Code | Default | Meaning |
| --- | --- | --- | --- |
| `unwrap` | `E0001` | deny | `.unwrap()` does not check for nil in Lua |
| `expect` | `E0002` | deny | `.expect(...)` does not check for nil; message is discarded |
| `format_spec` | `E0003` | warn | Non-`?` format specs (e.g. `{:.2}`) are ignored when lowering |
| `variable_index` | `E0004` | deny | Non-literal indices are not shifted for Lua's 1-based tables |
| `identification_ctor` | `E0005` | allow | Obsolete; Identification constructors now lower to payloads |
| `option_if` | `E0006` | deny | Plain `if` / `while` on an Option uses Lua truthiness |
| `ambiguous_try` | `E0007` | deny | `?` on an untyped local (assumes Result) |
| `ambiguous_method` | `E0008` | deny | Overlapping Option/Result method on an untyped local |
| `skipped_mod` | `E0009` | deny | Inline `mod` without `#[factorio_rs::export]` is skipped |
| `result_if` | `E0010` | deny | Plain `if` / `while` on a Result is always truthy |
| `err_nil` | `E0011` | deny | `Err(nil)` / `Err(None)` collapses with Ok |
| `option_try` | `E0012` | deny | `?` on a call/method assumes Result; Option APIs need a typed binding |
| `integer_div` | `E0013` | warn | `/` or `/=` without a float operand (Lua `/` is always float) |
| `struct_rest` | `E0014` | deny | Struct update `..rest` other than `Default::default()` |
| `numeric_cast` | `E0015` | warn | Numeric `as` cast is a no-op in Lua (no truncation/clamping) |
| `todo_macro` | `E0016` | deny | `todo!` / `unimplemented!` (prefer `panic!` or finish the path) |
| `storage_index` | `E0017` | warn | `storage["key"]` read/write returns opaque `LuaAny`; prefer `.get` / `.set` |

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
assets = [
  { from = "assets/graphics", to = "graphics" },
]
dependencies = ["? space-age"]

[emit]
lua_module_prefix = "mm"

[lints]
unwrap = "allow"

[profiles.debug]
debug_level = 2
prune_dead_code = false
optimize_ir = false

[profiles.release]
debug_level = 0
prune_dead_code = true
optimize_ir = true
```
