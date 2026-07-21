---
title: Lints
description: Transpile-time safety diagnostics, EXXX codes, and Factorio.toml configuration.
---

`factorio-rs check` / `build` run `cargo check` against API stubs that never run
in Factorio. Patterns that type-check in Rust can still miscompile or nil-crash
in Lua. Transpile-time diagnostics catch a set of those traps and report them as
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
`deny` because they can produce wrong or unsafe Lua. `format_spec` and
`integer_div` default to `warn` (formatting is dropped; Factorio math is often
float and operand types are not fully tracked).

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
| `identification_ctor` | `E0005` | allow | Obsolete; Identification constructors lower to payloads |
| `option_if` | `E0006` | deny | Plain `if` / `while` on an Option binding (Lua truthiness) |
| `ambiguous_try` | `E0007` | deny | `?` on an untyped local |
| `ambiguous_method` | `E0008` | deny | `.map` / overlapping helpers on an untyped local |
| `skipped_mod` | `E0009` | deny | Inline `mod` without `#[factorio_rs::export]` |
| `result_if` | `E0010` | deny | Plain `if` / `while` on a Result binding (always truthy) |
| `err_nil` | `E0011` | deny | `Err(nil)` / `Err(None)` collapses with Ok |
| `option_try` | `E0012` | deny | `?` on a call/method (assumes Result; Option APIs need a typed binding) |
| `integer_div` | `E0013` | warn | `/` or `/=` without a float operand (Lua `/` is always float) |
| `struct_rest` | `E0014` | deny | Struct update `..rest` other than `Default::default()` |

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

Obsolete. Identification constructors such as `ForceID::Name("enemy")` now
lower to the Factorio payload. The lint id remains so older
`[lints] identification_ctor = ...` keys still parse; the default level is
`allow` and nothing emits it.

Prefer exact constructors over payload `.into()`:

```rust
force: Some(ForceSet::One(ForceID::Name("enemy"))),
force: Some(ForceSet::One(ForceID::Force(source.force()))),
```

See [API types](api-types/) for Identification / `IndexOrName` details.

### `option_if` (`E0006`)

`if opt { ... }` or `while opt { ... }` when `opt` is an `Option` binding uses
Lua truthiness, so `Some(false)` / `Some(0)` skip the body. Prefer
`if let Some(...)` / `.is_some()`.

### `ambiguous_try` (`E0007`) / `ambiguous_method` (`E0008`)

`?` and helpers like `.map` / `.and_then` need to know Option vs Result. Typed
`Option` / `Result` bindings are fine (`Option` `?` early-returns `nil`).
Untyped locals get these denies - annotate the binding or use `.ok_or(...)?`.

### `skipped_mod` (`E0009`)

An inline `mod { ... }` without `#[factorio_rs::export]` is not lowered (contents
are dropped). Export the mod or move items into a file module.

### `result_if` (`E0010`)

`if result { ... }` / `while result { ... }` when `result` is a `Result` binding
is always truthy in Lua (Results are tables). Prefer `if let Ok(...)` or
`.is_ok()`.

### `err_nil` (`E0011`)

`Err(nil)` / `Err(None)` uses the same discriminant as Ok (`r.err == nil`).
Prefer a non-nil error payload (`String`, number, or table).

### `option_try` (`E0012`)

`?` on a **call or method** always lowers with Result semantics (`.err` / `.ok`).
Factorio APIs that return `Option` (nil) need a typed binding first, or
`.ok_or(...)?`:

```rust
// Bad - call `?` assumes Result tables
let player = game.get_player(index.into())?;

// Good - Option binding uses nil early-return
let player: Option<_> = game.get_player(index.into());
let player = player?;

// Good - bridge into Result
let player = game.get_player(index.into()).ok_or("missing")?;
```

For Result-returning helpers, bind with an explicit `Result` type before `?`.

### `integer_div` (`E0013`)

Rust integer `/` truncates; Lua `/` always does float division. This lint warns
on `/` and `/=` when neither operand looks like a float (`2.0`, `f64`, ...).

```rust
// Warns
let q = n / 2;

// Prefer an explicit float operand when you want Lua-style division
let q = n as f64 / 2.0;
```

### `struct_rest` (`E0014`)

Only **explicit** struct fields are emitted. `..Default::default()` is ignored
on purpose so Factorio parameter tables stay sparse. Any other `..rest` drops
fields silently and is denied:

```rust
// Bad - `base.y` is not copied
Point { x: 1, ..base }

// OK - intentional sparse update
LuaEntityMineParams {
    force: true,
    ..Default::default()
}
```

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
| Hard error | unsupported macro, bad locale key, unsupported pattern | Stops that module; other modules still run; build fails at the end |

Hard errors are not configurable via `[lints]`. Fix the source or use a
supported construct (see [Supported Rust](language/)).

## See also

- [`[lints]` in Factorio.toml](../reference/factorio-toml/#lints) - config keys
- [Supported Rust](language/) - what lowers, and the safety trap table
- [Option and Result](option-and-result/) - nil / `{ ok }` / `{ err }` / `?`
- [API types](api-types/) - Identification enums and exact constructors
