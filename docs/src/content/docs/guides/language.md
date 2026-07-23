---
title: Supported Rust
description: Inventory of Rust syntax factorio-rs can transpile to Lua.
---

factorio-rs does **not** implement a full Rust dialect. It lowers a Factorio-oriented subset of Rust into Lua. `factorio-rs check` / `build` run `cargo check` against the SDK stubs, **expand macros with rustc**, then only accept constructs the frontend knows how to lower.

This page is the **inventory**. Prefer recipes and focused language pages when
learning a feature:

| Topic | Page |
| --- | --- |
| `Option` / `Result` / `?` | [Option and Result](/guides/option-and-result/) |
| User `enum` + `match` | [Enums](/language/enums/) - [State machines](/recipes/state-machines/) |
| `Vec`, ranges, `.map`/`.filter`/`.collect` | [Collections](/language/collections/) - [Filter entities](/recipes/filter-entities/) |
| `type` aliases | [Type aliases](/language/type-aliases/) |
| Writing `macro_rules!` / proc-macro DSLs | [Authoring macros](/guides/authoring-macros/) |

Lua has no native traits or borrow checker. Option-like values are usually
**value or `nil`**; Results are tagged `{ ok }` / `{ err }` tables. Tables also
stand in for structs, arrays, and maps. Same-crate Rust traits lower to method
tables (and dyn fat pointers); see [Traits](#traits).

## Top-level items

| Supported                   | Notes                                                            |
| --------------------------- | ---------------------------------------------------------------- |
| `fn`                        | Private -> `local function`; `pub` -> `module.name` (see below) |
| `struct` + inherent `impl`  | Fields, methods, associated `const`s                             |
| `trait` + `impl Trait for T` | Same-crate traits (`use` across modules); methods merge onto the concrete type; see [Traits](#traits) |
| `impl From<T> for U` / `impl Into<U>` | `From` becomes `T:into()`; `impl Into` in signatures is accepted; `.into()` calls that method when applicable (otherwise transparent) |
| `enum` + inherent `impl`    | Unit, tuple, and named variants as tagged tables                 |
| `const`                     | Becomes a local (or exported) binding; same-module uses of `pub const` qualify as `module.NAME` |
| `type Name = ...`           | Transparent; resolved then forgotten (no Lua emission)           |
| `use crate::...`            | Binding crates with `[package.metadata.factorio]` also lower; see [Sharing code between mods](/guides/dependencies/). `crate::` paths become `require`s |
| `#[factorio_rs::export]`      | Publishes a fn via remote (control) or require (shared); see [Sharing code between mods](/guides/dependencies/) |
| `mod name;`                 | Declares a submodule file                                        |
| Prototype / locale macros   | `mod_settings!`, `item!`, `recipe!`, ... - see [Macros](/reference/macros/) |
| User / dep macros           | `macro_rules!` and dependency proc macros - [Authoring macros](/guides/authoring-macros/) |
| Doc comments                | Emitted as Lua comments when debug comments are on               |

**Not supported (yet):** `static`, tuple structs, trait generics / supertraits /
associated-type bounds or defaults.

`type` aliases, enums, and collection iterators have dedicated pages (links in
the table above).

### Traits

Same-crate `trait` + `impl Trait for Struct` is supported:

- Trait methods merge onto the concrete type’s method table (call as
  `value.method()`).
- Default method bodies in the trait are filled into impls that omit them.
- Associated types (`type Output;`) are supported without bounds or defaults;
  use `Self::Output` in method signatures. Dyn coerce rejects traits that
  declare associated types (not object-safe).
- Import a trait from another module in the same crate with
  `use crate::shared::alert::Alert` (project build builds a trait catalog).
- `&dyn Trait` / `Box<dyn Trait>` lower to Lua fat pointers `{ _data, _vt }`
  with per-impl `__vt_Trait_Concrete` vtables and dyn method dispatch.
  Call sites to dyn parameters auto-coerce concrete args (`f(&value)`);
  `as &dyn Trait` is still supported for locals and explicit casts.
- Dyn coerce requires object-safe methods (no `Self` by value in signatures
  beyond the receiver pattern the frontend accepts; no associated types).

Non-goals for now: generics on traits, supertraits, associated-type bounds /
defaults, and traits imported from other Factorio mods (binding crates).

See the [`traits_demo`](/examples/traits-demo/) example for a Factorio-flavored
walkthrough (cross-module `Alert`, defaults, overrides, static + dyn dispatch).

### `pub fn` vs `fn`

| Rust | Lua definition | Name used as a value (`add_command(..., greet)`) |
| --- | --- | --- |
| `fn greet` | Forward-declared `local greet`, then `function greet` | `greet` |
| `pub fn greet` | `function control.greet` | `control.greet` |

Private functions are forward-declared at the top of the module so earlier locals
can call later ones (plain `local function` would resolve those calls as globals).

Either form is valid for callbacks. Prefer `fn` for handlers that only exist to
pass to Factorio APIs; use `pub fn` when other modules need to call the function.
 
## Statements

| Supported                                            | Notes                                   |
| ---------------------------------------------------- | --------------------------------------- |
| `let x = ...` / `let x: T = ...`                     | Initializer required                    |
| `let (a, b) = (e1, e2)`                              | Same length; plain idents only          |
| `if` / `else` / `else if`                            |                                         |
| `if let Some(x) = e` / `if let x = e`                | Binds `e`, then tests `x ~= nil`        |
| Let chains (`a && let Some(x) = e && ...`)           | Nested locals + `if x ~= nil`           |
| `for x in iter`                                      | `ipairs` for `Vec` / `.iter()`; else `pairs`; ranges -> numeric `for` |
| `while cond { ... }`                                 | -> `while cond do ... end`              |
| `loop { ... }`                                       | -> `while true do ... end`              |
| `continue`                                           | -> labeled `goto` inside `for` / `while` / `loop` |
| `break`                                              | -> Lua `break` (no value / label)       |
| `match`                                              | Desugared to nested `if`/`else` (see below) |
| `return` / tail expression                           | Last expression without `;` is returned |
| `x = ...` / `x.field = ...`                          | Path or field targets only              |
| `+=` `-=` `*=` `/=`                                  |                                         |
| `println!(...);` and other call expressions with `;` |                                         |

**Not supported (yet):** `break value`, labeled break/continue, bare mid-block expressions without `;` (except `if` / `for` / `while` / `loop` / `match`).

### `match`

```rust
match value {
    Some(player) if player.connected() => { /* ... */ }
    Some(_) | None => {}
}

match point {
    Point { x, y: 0 } => x,
    Point { x, y, .. } => x + y,
}

let n = match flag {
    true | false => 1, // or-pattern
};
```

Supported patterns: `_`, literals, `None` / `Option::None`, `Some(...)` (nested
patterns ok), `Ok(...)` / `Err(...)`, struct patterns (`Foo { a, b: 0, .. }`),
or-patterns (`A | B`), plain bindings, and `if` guards. Guards that fail fall
through to later arms. Struct patterns only destructure fields (no runtime type
tag). Top-level `A | B => body` expands to nested arms so each alternative can
bind differently; nested ors require identical bindings.

Statement-position `match` becomes a temp plus an `if` / `elseif` / `else` chain.
Guarded arms (`pat if cond =>`) test the discriminant once: guard failure falls
through to later arms without re-checking a tag already proven true, and pattern
miss skips later arms that require that same discriminant. Enum matches also bind
`.tag` once before the `if`/`elseif` chain (so method `rawget(self, "tag")` is not
repeated per arm), and same-discriminant fallthrough reuses payload bindings.
Value-position `match` (including tail expressions) becomes an IIFE in debug
builds; with `optimize_ir` (release default), `let x = match ...` /
`return match ...` are expanded into statement `if` / `elseif` instead.

### `if let`, `Option`, and `Result`

Factorio APIs are full of missing values and fallible helpers. Prefer Rust
`Option` / `Result` at the source; the transpile maps them to nil and tagged
tables.

```rust
if let Some(player) = game.get_player(IndexOrName::Index(index)) {
    // ...
}

fn load(name: &str) -> Result<i32, String> {
    let n = parse(name)?;
    Ok(n + 1)
}
```

Full reference (Lua representation, methods, `?`, traps):
[Option and Result](/guides/option-and-result/).

### Closures

```rust
let double = |n| n * 2;
let y = x.map(|n| n + 1);
table.sort(list, lua_fn2(|a, b| a < b));
```

Closures lower to Lua `function(...) ... end`. Outer locals are captured as Lua
upvalues (`move` is ignored). Params must be plain identifiers (optional types
ok: `|x: i32|`). Async closures are rejected.

For Factorio callback APIs typed as `LuaFunction`, wrap closures with
`lua_fn` / `lua_fn0` / `lua_fn2` so `cargo check` accepts them (fn items can
still pass directly). The transpile strips `lua_fn(...)` to the inner function
value.

## Expressions

| Supported                           | Notes                                                          |
| ----------------------------------- | -------------------------------------------------------------- |
| Literals                            | `i64`/`f64`/string/`bool`                                      |
| `None`                              | -> `nil`                                                       |
| `Some(x)` / `Option::Some(x)`       | -> `x` (for typed `Option` stub params)                        |
| `Ok(v)` / `Err(e)`                  | -> `{ ok = v }` / `{ err = e }` - [Option and Result](/guides/option-and-result/) |
| `expr?`                             | Result early-return - [Option and Result](/guides/option-and-result/) |
| Paths / fields / calls / methods    | Including `crate::` (auto-require)                             |
| Named struct literals               | -> Lua tables                                                  |
| `[a, b]`                            | -> `{ a, b }`                                                  |
| `a[i]`                              | Integer literal indices are +1 for Lua (`0`->`1`, `1`->`2`, ...); non-literal indices lint `variable_index` |
| `&x`, `*x`, `x as T`, `(x)`         | Transparent                                                    |
| `!` / `-`                           | `not` / unary minus                                            |
| `+ - * / % == != < <= > >= && \|\|` |                                                                |
| `if c { a } else { b }`             | Each arm must be a **single** expression; safe Lua `if`/`else` (IIFE mid-expression; statement form under `optimize_ir`) |
| `\|x\| ...` / `\|x\| { ... }`           | -> Lua `function(x) ... end` (see [Closures](#closures))         |
| `println!(...)`                     | -> `game.print(...)` with `..` concatenation                   |
| `format!(...)`                      | -> string via `..` concatenation                               |
| `tracing::info!` / `warn!` / ...      | -> colored `game.print` (CLI `tracing` feature; default on)    |
| `serde_json::to_string` / `from_str` / ... | -> `helpers` JSON / `string.pack` (`serde` feature)           |
| Literal string unions               | e.g. `GuiDirection::Horizontal` -> `"horizontal"`              |

**Transparent zero-arg methods** (receiver kept): `clone`,
`as_str`, `as_ref`, `as_slice`, `as_deref`, `to_string`, `to_owned`.
`.into()` is transparent unless a `From` conversion or `impl Into<_>` parameter
applies (then it becomes `:into()`).

`.unwrap()` and `.expect("...")` are also stripped to the receiver, but emit
lints `unwrap` / `expect` (default **deny**; see [Lints](/guides/lints/)).

**Option / Result helpers** (`is_some`, `ok_or`, `?`, `map`, ...): see
[Option and Result](/guides/option-and-result/).

**Special method lowering:**

| Rust                | Lua                               |
| ------------------- | --------------------------------- |
| `.get(key)`         | settings: `recv[key].value`; `storage.get`: `storage[key]` |
| `.len()`            | `#recv`                           |
| `.is_empty()`       | `#recv == 0`                      |
| `.push(x)`          | `table.insert(recv, x)`           |
| `.random_int(n)` / `.random_range(m, n)` | `recv.random(...)` (math) |
| `.format_1`...`.format_4` | `recv.format(...)` (string) |
| `.insert_at(list, pos, value)` | `table.insert(...)` |
| zero-arg API method | `recv.method` (property read)     |
| `set_<attr>(v)` / `write_<attr>(v)` | `recv.attr = v` (attribute write) |
| method with args    | `recv.method(args)` (`.` not `:`) |

### `serde_json`

Enable `factorio-rs` feature `serde` (CLI default includes lowering). Serde does
**not** run in Factorio - see [Serde / JSON](/guides/serde/) for the full mapping.

**Constructors:** `Vec::new()`, `Type::default()`, `LuaAny::new()` -> `{}` or
`nil` as appropriate. Prefer typed concepts over `LuaAny` when the stubs expose
them - see [API types](/guides/api-types/).

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

`Vec`, ranges, `ipairs` / `pairs`, and the `.map` / `.filter` / `.collect`
subset: [Collections and iterators](/language/collections/).

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

See [Stages](/guides/stages/) for how files map to Factorio load phases.

## Factorio-oriented features

| Feature              | Docs                                  |
| -------------------- | ------------------------------------- |
| Stages / discovery   | [Stages](/guides/stages/)             |
| `#[event]` + filters | [Events](/guides/events/)             |
| `mod_settings!`      | [Mod settings](/guides/mod-settings/) |
| `item!` / `recipe!` / `technology!` / `fluid!` / `assembling_machine!` / ... | [Prototypes](/guides/prototypes/) / [Macros](/reference/macros/) |
| `locale!`            | [Locale](/guides/locale/)             |
| Profiles / prune     | [Profiles](/guides/profiles/)         |

Filter arguments must be **string literals**. Events are **control-stage
only**.

## Expression macros

`factorio-rs build` / `check` expand macros with rustc before lowering, so
**`macro_rules!` and dependency proc macros** work when their expansion is
supported Rust. Built-in helpers are still recognized both unexpanded and in
their rustc-expanded forms:

| Macro | Lua |
| --- | --- |
| `println!(...)` | `game.print(...)` with `..` concatenation |
| `format!(...)` | string built with `..` (no `game.print`) |
| `assert!(cond)` / `assert!(cond, "...")` | `if not (cond) then error(...) end` |
| `assert_eq!` / `assert_ne!` | compare temps, `error` with left/right |
| `panic!("...")` | `error(...)` |
| `tracing::info!` / `warn!` / `error!` / `debug!` / `trace!` | `game.print` with `[LEVEL]` prefix + color |

`{:?}` / `{:#?}` (and `{name:?}`) dump values for Factorio using the static Rust
type: plain tables (event data, concepts) go through `helpers.table_to_json`,
and userdata / scalars / unknown types use `tostring` (LuaObjects like
`LuaEntity` are userdata, so JSON alone would error). Applies to `println!`,
`format!`, and `tracing::*!`. Plain `{}` still concatenates the value as-is.

Enable `factorio-rs` feature `tracing` in the mod `Cargo.toml` so those macros
type-check. Details: [Tracing](/guides/tracing/).

Supported template forms: `{}`, `{0}`, `{name}`, `{:?}` / `{:#?}` / `{name:?}`,
and `{{` / `}}` escapes. Other format specs after `:` (e.g. `{:.2}`) trigger the
`format_spec` lint (default **warn**; see [Lints](/guides/lints/)).

Macros whose expansion uses unsupported Rust still fail at lower time. For
writing your own macros, see [Authoring macros](/guides/authoring-macros/).

## Safety

`factorio-rs check` / `build` run **`cargo check`** against Factorio API stubs
(real method names, arity, and Rust types) before lowering. Stubs never execute
in Factorio - patterns that type-check can still miscompile or nil-crash at
runtime. Transpile [lints](/guides/lints/) catch several of those traps; missing coverage
still fails the build as unsupported syntax when known unsafe.

| Trap | What happens | Fix |
| --- | --- | --- |
| `.unwrap()` / `.expect(...)` | Stripped; lint E0001/E0002 | `if let` / `?` / `ok_or` - [Option and Result](/guides/option-and-result/) |
| `if opt { ... }` on an Option | `Some(false)` skipped; lint E0006 | `if let Some(...)` or `is_some()` |
| Untyped local `?` / `.map` | Assumes Result / Option; lint E0007/E0008 | Annotate `Option`/`Result` or `.ok_or` |
| Call/method `?` | Assumes Result; lint E0012 | Typed `Option`/`Result` binding or `.ok_or` |
| `/` / `/=` without float operand | Lua float div; lint E0013 (warn) | Use `n as f64 / 2.0` or allow the lint |
| `Struct { ..other }` (not Default) | Rest fields dropped; lint E0014 | Explicit fields or `..Default::default()` |
| `n as u8` (numeric cast) | No-op in Lua; lint E0015 | Drop the cast or use explicit math |
| `todo!` / `unimplemented!` | Lowers to `error(...)`; lint E0016 | Prefer `panic!("...")` or finish the path |
| `storage["key"]` | Opaque `LuaAny`; lint E0017 | Prefer `storage.get` / `storage.set` |
| Inline `mod` without `#[export]` | Contents skipped; lint E0009 | Export the mod or use a file module |
| `arr[i]` with variable `i` | Not +1 for Lua | Use a 1-based index, or literal indices |
| `{:.2}` / other format specs | Ignored output | Use `{}` / `{:?}` only |
| `ForceID::Name(...)` etc. | Lowers to the payload | Prefer constructors over `.into()` |
| Trailing `None` args | Omitted from Lua calls | Prefer omit / `None` only for unused tails |
| `if c { a } else { b }` when `a` is falsey | Was wrong with `and`/`or`; now safe IIFE | Prefer statement `if` for complex arms |
| Optional table fields | Typed `Option<T>`; `None` omitted | Set `Some(...)` only for fields you need |
| Stringly callback names under prune | Prefer fn items / `lua_fn` | Pass the function value, not only a string |

## Common errors

| Error | Typical cause |
| --- | --- |
| `unsupported expression (Async)` / ... | Use a supported construct (see [Language](/guides/language/)) |
| `unsupported item` | Unknown macro / unsupported item form (`static`, tuple struct, ...) |
| `let binding requires an initializer` | `let x;` without value |
| `event handlers are only allowed in control-stage modules` | Move handler to control |
| `could not resolve locale key` | `Settings::FOO` not in this module |
| `unsupported macro` | Expansion produced unsupported Rust, or a non-allowlisted helper in unit tests |

## See also

- [Enums](/language/enums/) - [Collections](/language/collections/) - [Type aliases](/language/type-aliases/)
- [Option and Result](/guides/option-and-result/) - nil, `{ ok }` / `{ err }`, `?`, methods
- [Recipes](/recipes/first-hour/) - first hour, storage, settings, iterators, ...
- [mandatory_spaghetti](/examples/mandatory-spaghetti/) - larger control-stage tour
- [hello_world](/examples/hello-world/) - Result, events, tests
- [API types](/guides/api-types/) - [Lints](/guides/lints/)
