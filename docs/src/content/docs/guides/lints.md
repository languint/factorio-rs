---
title: Lints
description: Transpile-time safety diagnostics, EXXX codes, and Factorio.toml configuration.
---

`cargo check` only validates against API stubs that never run in Factorio.
Patterns that type-check in Rust can still miscompile or nil-crash in Lua.
factorio-rs catches a set of those traps at transpile time and reports them as
**lints**.

Lints are separate from hard frontend errors (unsupported syntax, bad locale
keys, and so on). Hard errors stop lowering for that module; lints do not -
every lint in the project is collected and printed together.

## Levels

Configure each lint in [`Factorio.toml`](../reference/factorio-toml/#lints):

```toml
[lints]
unwrap = "allow"   # silence
expect = "warn"    # print, build continues
variable_index = "deny"  # print, build fails
```

| Level | Effect |
| --- | --- |
| `allow` | Do not emit the lint |
| `warn` | Print a warning; Lua is still written |
| `deny` | Print an error; build fails after all diagnostics are shown |

Unset lints use their **defaults** (see the table below). Most default to
`deny` because they can produce wrong or unsafe Lua. `format_spec` defaults to
`warn` because unsupported specs are simply ignored and the rest of the
template still lowers.

## Diagnostics

Reports look like rustc / Cargo:

```text
error[E0001]: `.unwrap()` does not check for nil in Lua; use `if let Some(...)` instead
   ,-[ src/lib.rs:12:5 ]
   |
12 |     x.unwrap()
   |     |^^^^^^^^^
   |     `--------- `.unwrap()` does not check for nil in Lua; use `if let Some(...)` instead
---'
   = help: use `if let Some(x) = ...` (or set `[lints] unwrap = "allow"`)
```

- **Code** (`E0001` ...) - stable id shown in the header. Use this when talking
  about a finding.
- **Identifier** (`unwrap`, ...) - key in `[lints]`. Same lint, config spelling.
- **Span** - underlines the offending expression (ASCII carets).
- **Help** - short fix hint under the report.

Warns use `warning[EXXX]:` (yellow); denies use `error[EXXX]:` (red). The build
prints every diagnostic across all modules, then exits with
`transpile failed due to previous errors` if any deny (or hard error) occurred.
No Lua is written in that case.

## Catalog

| Identifier | Code | Default | Fires on |
| --- | --- | --- | --- |
| `unwrap` | `E0001` | deny | `.unwrap()` |
| `expect` | `E0002` | deny | `.expect(...)` |
| `format_spec` | `E0003` | warn | Format specs other than `:?` / `#?` |
| `variable_index` | `E0004` | deny | Non-literal `expr[index]` |
| `identification_ctor` | `E0005` | deny | `ForceID::Name(...)`-style constructors |

### `unwrap` (`E0001`) / `expect` (`E0002`)

`.unwrap()` and `.expect("...")` are stripped to the receiver so they type-check
like Rust, but Lua has no panic-on-nil. The call becomes a silent no-op and a
`nil` propagates.

```rust
// Bad - lowers to just `maybe_entity`; nil is unchecked
let entity = event.entity.unwrap();

// Good
if let Some(entity) = event.entity {
    // ...
}
```

`.expect`'s message is discarded entirely.

### `format_spec` (`E0003`)

`println!`, `format!`, and `tracing::*!` templates support `{}`, `{0}`,
`{name}`, `{:?}` / `{:#?}`, and `{{` / `}}`. Other specs after `:` (precision,
fill, alignment, ...) are **ignored** when lowering - the value is still
inserted, just without formatting.

```rust
// Warns: `:.2` has no effect in Lua
println!("y = {y:.2}");

// Prefer
println!("y = {y}");
println!("debug = {y:?}");
```

This defaults to **warn** because it does not change control flow or drop the
argument; only the formatting intent is lost.

### `variable_index` (`E0004`)

Rust arrays are 0-based; Factorio Lua tables are 1-based. factorio-rs shifts
**integer literal** indices (`arr[0]` -> `arr[1]`). A **variable** index is
passed through unchanged, so `arr[i]` with a 0-based `i` reads the wrong slot
(or `nil`).

```rust
// Bad - `i` is not shifted
let item = inventory[i];

// Good - use a 1-based index, or a literal
let item = inventory[0]; // -> inventory[1] in Lua
let item = inventory[i]; // only if `i` is already 1-based
```

### `identification_ctor` (`E0005`)

Schema “identification” unions (`ForceID`, `EntityID`, ...) are stub enums in
Rust. Writing `ForceID::Name("enemy")` does not lower to a real Lua value.
Pass the payload with `.into()` instead:

```rust
// Bad
force: Some(ForceID::Name("enemy")),

// Good
force: Some("enemy".into()),
force: Some(source.force().into()),
```

See [API types](api-types/) for Identification / `IndexOrName` details.

## Configuring defaults

You only need to list overrides. Example: silence format noise in a prototype
mod, keep the rest at their defaults:

```toml
[lints]
format_spec = "allow"
```

Or temporarily treat a deny lint as a warning while migrating:

```toml
[lints]
unwrap = "warn"
```

`factorio-rs init` comments the available keys in the generated
`Factorio.toml`.

## Lints vs hard errors

| Kind | Example | Behavior |
| --- | --- | --- |
| Lint | `x.unwrap()`, `{:.2}` | Collected for the whole project; deny fails after reporting |
| Hard error | `match`, unsupported macro, bad locale key | Stops that module; other modules still run; build fails at the end |

Hard errors are not configurable via `[lints]`. Fix the source or use a
supported construct (see [Language support](language/)).

## See also

- [`[lints]` in Factorio.toml](../reference/factorio-toml/#lints) - config keys
- [Language support](language/) - what lowers, and the safety trap table
- [API types](api-types/) - Identification enums and `.into()`
