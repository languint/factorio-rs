---
title: Language support
description: What Rust syntax and patterns factorio-rs can transpile to Lua.
---

factorio-rs does **not** implement a full Rust dialect. It lowers a Factorio-oriented subset of Rust into Lua. `cargo check` still type-checks against the SDK stubs;
`factorio-rs build` only accepts constructs the frontend knows how to lower.

This page is the inventory of that surface.

Lua has no enums, traits, or borrow checker. Option-like values are usually
**value or `nil`**. Tables stand in for structs, arrays, and maps.

## Top-level items

| Supported                   | Notes                                                            |
| --------------------------- | ---------------------------------------------------------------- |
| `fn`                        | `pub` functions are exported from the module                     |
| `struct` + inherent `impl`  | Fields, methods, associated `const`s                             |
| `const`                     | Becomes a local (or exported) binding                            |
| `use crate::...`            | Other crates are ignored; only `crate::` paths become `require`s |
| `mod name;`                 | Declares a submodule file                                        |
| `mod_settings!` / `locale!` | Expanded / collected at transpile time                           |
| Doc comments                | Emitted as Lua comments when debug comments are on               |

**Not supported (yet):** `enum`, `trait`, trait `impl`, `type` aliases, `static`, tuple structs, unknown macros at item position.
 
## Statements

| Supported                                            | Notes                                   |
| ---------------------------------------------------- | --------------------------------------- |
| `let x = ...` / `let x: T = ...`                     | Initializer required                    |
| `let (a, b) = (e1, e2)`                              | Same length; plain idents only          |
| `if` / `else` / `else if`                            |                                         |
| `if let Some(x) = e` / `if let x = e`                | Binds `e`, then tests truthiness        |
| Let chains (`a && let Some(x) = e && ...`)           | Nested locals + `if`s                   |
| `for x in iter`                                      | -> `for _, x in pairs(iter)`            |
| `continue`                                           | -> labeled `goto` inside `for`          |
| `return` / tail expression                           | Last expression without `;` is returned |
| `x = ...` / `x.field = ...`                          | Path or field targets only              |
| `+=` `-=` `*=` `/=`                                  |                                         |
| `println!(...);` and other call expressions with `;` |                                         |

**Not supported (yet):** `match`, `while`, `loop`, `break`, bare mid-block expressions without `;` (except `if` / `for`).

### `if let` and `Option`

```rust
if let Some(player) = game.get_player(index.into()) {
    // player is the Lua value; nil/false would skip the body
}
```

`get_player` takes [`IndexOrName`](api-types/) (`u32` or `&str` via `.into()`).
There is no real `Option` wrapper in Lua. `None` becomes `nil`, and `Some(x)` is
transparent so stub APIs typed as `Option<T>` still type-check in Rust.

## Expressions

| Supported                           | Notes                                                          |
| ----------------------------------- | -------------------------------------------------------------- |
| Literals                            | `i64`/`f64`/string/`bool`                                      |
| `None`                              | -> `nil`                                                       |
| `Some(x)` / `Option::Some(x)`       | -> `x` (for typed `Option` stub params)                        |
| Paths / fields / calls / methods    | Including `crate::` (auto-require)                             |
| Named struct literals               | -> Lua tables                                                  |
| `[a, b]`                            | -> `{ a, b }`                                                  |
| `a[i]`                              | Index `0` becomes Lua `1`                                      |
| `&x`, `*x`, `x as T`, `(x)`         | Transparent                                                    |
| `!` / `-`                           | `not` / unary minus                                            |
| `+ - * / % == != < <= > >= && \|\|` |                                                                |
| `if c { a } else { b }`             | Each arm must be a **single** expression; emits `c and a or b` |
| `println!(...)`                     | -> `game.print(...)` with `..` concatenation                   |
| `format!(...)`                      | -> string via `..` concatenation                               |
| `tracing::info!` / `warn!` / ...      | -> colored `game.print` (CLI `tracing` feature; default on)    |
| Literal string unions               | e.g. `GuiDirection::Horizontal` -> `"horizontal"`              |

**Transparent zero-arg methods** (receiver kept): `into`, `unwrap`, `clone`,
`as_str`, `as_ref`, `as_slice`, `as_deref`, `to_string`, `to_owned`.

**Special method lowering:**

| Rust                | Lua                               |
| ------------------- | --------------------------------- |
| `.get(key)`         | `recv[key].value` (mod settings)  |
| `.len()`            | `#recv`                           |
| `.is_empty()`       | `#recv == 0`                      |
| `.push(x)`          | `table.insert(recv, x)`           |
| zero-arg API method | `recv.method` (property)          |
| method with args    | `recv.method(args)` (`.` not `:`) |

**Constructors:** `Vec::new()`, `Type::default()`, `LuaAny::new()` -> `{}` or
`nil` as appropriate. Prefer typed concepts over `LuaAny` when the stubs expose
them - see [API types](api-types/).

### Struct update / `Default`

```rust
LuaEntityMineParams {
    force: true,
    inventory,
    ..Default::default()
}
```

Only **explicit** fields are emitted. `..Default::default()` is ignored on
purpose so optional Factorio parameter tables stay sparse. Do not expect Rust default field values to appear in Lua.

## Collections

```rust
let mut list: Vec<MapPosition> = Vec::new();
list.push(pos);
for item in list {
    // ...
}
```

| Rust | Lua behaviour |
| --- | --- |
| `Vec::new()` | `{}` |
| `push` / `len` / `is_empty` | `table.insert` / `#` / `# == 0` |
| `for x in v` | `pairs(v)` - unordered; key discarded |

Not supported: iterator adapters, ranges (`0..n`), `collect`, etc.

## Types

Lowered for comments / light IR typing:
- integers, floats, `str` / `String` / `&str`, `()`
- `&self` / `&mut self` on methods
- other path types (API classes, `bool`, `Option`, ...) are treated as opaque for Lua

Reference types other than `&str` / `&Self` are rejected.

## Modules and imports

```rust
use crate::settings::Settings;
use crate::adjacent_blacklist;
```

- Only `crate::` imports produce `require`s.
- `use factorio_rs::...` is for `cargo check`; the frontend ignores non-`crate`
  imports.
- Nested item paths like `use crate::a::b::C` (two+ item segments) are not
  supported - import the module, then path to the item.

See [Stages](stages/) for how files map to Factorio load phases.

## Factorio-oriented features

| Feature              | Docs                                  |
| -------------------- | ------------------------------------- |
| Stages / discovery   | [Stages](stages/)             |
| `#[event]` + filters | [Events](events/)             |
| `mod_settings!`      | [Mod settings](mod-settings/) |
| `locale!`            | [Locale](locale/)             |
| Profiles / prune     | [Profiles](profiles/)         |

Filter arguments must be **string literals**. Events are **control-stage
only**.

## Expression macros

Only **`println!`**, **`format!`**, and (CLI default) **`tracing::*!`** level
macros are lowered:

| Macro | Lua |
| --- | --- |
| `println!(...)` | `game.print(...)` with `..` concatenation |
| `format!(...)` | string built with `..` (no `game.print`) |
| `tracing::info!` / `warn!` / `error!` / `debug!` / `trace!` | `game.print` with `[LEVEL]` prefix + color |

Enable `factorio-rs` feature `tracing` in the mod `Cargo.toml` so those macros
type-check. Details: [Tracing](tracing/).

Supported template forms: `{}`, `{0}`, `{name}`, and `{{` / `}}` escapes.
Format specs after `:` (e.g. `{:.2}`) are ignored.

Other macros in expression position fail with `UnsupportedMacro`.

## Common errors

| Error | Typical cause |
| --- | --- |
| `unsupported expression (Match)` / `(While)` / ... | Use `if` / `for` instead |
| `unsupported item` | `enum` / `trait` / unknown macro |
| `let binding requires an initializer` | `let x;` without value |
| `event handlers are only allowed in control-stage modules` | Move handler to control |
| `could not resolve locale key` | `Settings::FOO` not in this module |
| `unsupported macro` | Only `println!` / `format!` / `tracing::*!` in expressions |

## See also

- [mandatory_spaghetti](../examples/mandatory-spaghetti/) - settings, locale,
  `Vec`, `for`, `continue`, `..Default::default()`, let-chains
- [hello_world](../examples/hello-world/) - events, filters, `println!`
- [tracing_test](../examples/tracing-test/) - optional `tracing` feature
- [API types](api-types/) - concepts, Identification enums, `LuaAny`
