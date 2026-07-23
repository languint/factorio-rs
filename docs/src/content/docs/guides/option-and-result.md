---
title: Option and Result
description: How Rust Option and Result lower to Lua nil and tagged tables, including ?, match, and method helpers.
---

Factorio’s Lua API is full of “maybe missing” values: `get_player` may return
nothing, `create_entity` may fail, surfaces can be absent. In Rust you model
those with **`Option`** and **`Result`**. factorio-rs keeps the same authoring
style under `cargo check`, then lowers them to idiomatic Lua.

This page is the full reference. For the rest of the language surface, see
[Supported Rust](/guides/language/).

## Mental model

| Rust | Lua representation | How you test it |
| --- | --- | --- |
| `Option<T>` | value or `nil` (no wrapper) | `x ~= nil` |
| `Result<T, E>` | `{ ok = T }` or `{ err = E }` | `r.err == nil` -> Ok |

**Option is transparent.** `Some(x)` is just `x`; `None` is `nil`. That matches
how Factorio already signals “no entity / no player”.

**Result is tagged.** Success and failure are both tables so `Ok` and `Err` stay
distinguishable even when the success value is falsey (`Ok(false)`, `Ok(0)`,
`Ok(())` as `{ ok = nil }`).

Do **not** use Lua truthiness (`if x then`) to mean “is Some” or “is Ok”:

- `Some(false)` / `Some(0)` must still count as present.
- `Ok` is checked via `r.err == nil`, not via truthiness of `r` or `r.ok`.

## Option

### Constructors and literals

| Rust | Lua |
| --- | --- |
| `None` | `nil` |
| `Some(x)` / `Option::Some(x)` | `x` (inner value only) |

Typed stub parameters like `color: Option<Color>` still type-check; at transpile
time optional fields that are `None` are simply omitted from tables.

### Branching

Prefer **`if let`** / **`match`** over `.unwrap()`:

```rust
if let Some(player) = game.get_player(index.into()) {
    // enters when the value is not nil - including Some(false) / Some(0)
    player.print("hi");
}

match game.get_surface(0) {
    Some(surface) => { /* ... */ }
    None => {}
}
```

Lowering for `if let Some(x) = e`:

```lua
local x = e
if x ~= nil then
	-- body
end
```

Let-chains work the same way:

```rust
if let Some(surface) = game.get_surface(0)
    && let Some(entity) = surface.find_entity("iron-ore", pos)
{
    // ...
}
```

### Methods

All of these are **nil-aware** (they test `~= nil`, not Lua truthiness). Side-effecting
receivers (method calls, etc.) are bound once to a temp so they are not evaluated
twice.

| Rust | Behaviour in Lua |
| --- | --- |
| `x.is_some()` | `x ~= nil` |
| `x.is_none()` | `x == nil` |
| `x.unwrap_or(d)` / `x.or(d)` | `x` if present, else `d` |
| `x.and(y)` | `y` if `x` present, else `nil` |
| `x.map(f)` / `x.and_then(f)` | call `f(x)` if present, else `nil` |
| `x.unwrap_or_else(f)` / `x.or_else(f)` | `x` if present, else `f()` |
| `x.filter(p)` | keep `x` when present and `p(x)`, else `nil` |
| `x.ok_or(e)` | `{ ok = x }` if present, else `{ err = e }` |
| `x.ok_or_else(f)` | `{ ok = x }` if present, else `{ err = f() }` |

**Not supported:** `.unwrap_or_default()` (Default is not typed in Lua — use
`.unwrap_or(...)` / `.unwrap_or_else(...)`), and other std helpers such as
`.unwrap_err()`, `.flatten()`, `.transpose()`, `.inspect()`, `.copied()` /
`.cloned()`, `.map_or` / `.map_or_else`, `.ok()` / `.err()`. Those hard-error
instead of emitting a Lua method call.

```rust
let entity = surface
    .create_entity(params)
    .ok_or("failed to place entity")?;
```

Becomes roughly (no Ok-path Result table):

```lua
local __try_1 = surface.create_entity(params)
if __try_1 == nil then
	return { err = "failed to place entity" }
end
local entity = __try_1
```

`ok_or_else(f)?` only calls `f()` on the nil path. Mid-expression `If` /
`unwrap_*` still lower through a safe IIFE (or a hoisted `__hN` temp under
`optimize_ir`) so falsey then-arms (e.g. `Some(false)`) are not lost to Lua
`and`/`or`.
### `.unwrap()` and `.expect()`

These **do not** check for nil. They strip to the receiver and emit lints
`unwrap` (`E0001`) / `expect` (`E0002`) (default **deny**). Prefer `if let`,
`unwrap_or`, or `ok_or` + `?`. See [Lints](/guides/lints/).

## Result

### Constructors

| Rust | Lua |
| --- | --- |
| `Ok(v)` / `Result::Ok(v)` | `{ ok = v }` |
| `Err(e)` / `Result::Err(e)` | `{ err = e }` |

Discriminant: **`r.err == nil` means Ok**; **`r.err ~= nil` means Err**.

Prefer **non-nil** error payloads (`String`, tables, numbers). `Err(nil)` /
`Err(None)` is ambiguous with Ok under the `.err == nil` test and fires lint
`err_nil` (`E0011`).

### `?` (try operator)

On a typed **`Option`** binding, `opt?` early-returns `nil` and yields the value.
On **`Result`** (typed bindings), `expr?` early-returns the Err table and yields
`.ok`. **Call/method** `?` still assumes Result and triggers lint `option_try`
(`E0012`) - bind with an explicit `Option`/`Result` type first, or use
`.ok_or(...)?` for Option APIs. Untyped locals assume Result and fire
`ambiguous_try` (`E0007`).

```rust
fn take(opt: Option<i32>) -> Option<i32> {
    let n = opt?; // nil early-return
    Some(n + 1)
}
```

Result example:

```rust
fn try_place_entity(params: LuaSurfaceCreateEntityParams) -> Result<(), String> {
    let surface = game
        .get_surface(0)
        .ok_or("surface does not exist")?;
    surface
        .create_entity(params)
        .ok_or("engine returned None")?;
    Ok(())
}
```

Sketch of the generated control flow:

```lua
local __try_1 = -- Option.ok_or(...) Result table
if __try_1.err ~= nil then
	return __try_1
end
local surface = __try_1.ok
-- ...
```

`?` does **not** rewrite error types with `From`; the Err table is returned
as-is (same “transpile ignores Rust conversion traits” idea as `Option`).

### Branching

```rust
if let Ok(n) = parse(name) {
    // tmp Result; if tmp.err == nil then local n = tmp.ok
}

match load(path) {
    Ok(data) => use(data),
    Err(e) => println!("load failed: {e}"),
}
```

`match` / `if let` on `Ok` / `Err` use the same nested `if` desugaring as other
patterns (see [Supported Rust](/guides/language/#match)).

### Methods

| Always available | Notes |
| --- | --- |
| `is_ok` / `is_err` | `r.err == nil` / `r.err ~= nil` |
| `map_err` | rewrite Err payload; Ok unchanged |

These need a **Result-typed binding** so they are not mistaken for Option
helpers with the same names (`map`, `and_then`, ...):

| Needs `Result` binding | Notes |
| --- | --- |
| `unwrap_or` / `unwrap_or_else` | Ok value or default / `f(err)` |
| `map` / `and_then` | map Ok; Err unchanged / chain Results |
| `or_else` | Ok unchanged; else `f(err)` |

How the binding becomes `Result`:

```rust
let r: Result<i32, String> = load(name); // annotation
let r = Ok(1);                           // inferred from Ok / Err
fn f(r: Result<i32, String>) { ... }     // parameter type
```

Without that, overlapping names like `.map(...)` still use **Option**
semantics - annotate when in doubt.

On a Result binding, `.unwrap()` / `.expect(...)` become `.ok` and still lint.

## Choosing Option vs Result

| Situation | Prefer |
| --- | --- |
| API returns “missing” (`get_player`, lookups) | `Option` (matches Factorio nil) |
| Your helper can fail with a reason | `Result` (`Ok` / `Err` tables) |
| Bridge “maybe nil” into fallible code | `opt.ok_or("...")?` |

```rust
fn require_player(index: u32) -> Result<LuaPlayer, String> {
    game.get_player(index.into())
        .ok_or_else(|| format!("no player {index}"))
}
```

## Closures with map / and_then

Closures lower to Lua `function(...) ... end`. Combine them with Option/Result
helpers as in Rust:

```rust
let n = maybe.map(|x| x + 1);
let r = result.and_then(|x| scale(x));
```

See [Supported Rust](/guides/language/#closures) for closure limits (plain params,
no async).

## Safety traps

| Trap | What goes wrong | Do this instead |
| --- | --- | --- |
| `if player { ... }` on an `Option` | `Some(false)` skipped | `if let Some(player) = ...` or `player.is_some()` |
| `.unwrap()` / `.expect()` | No nil/Err check; lint deny | `if let`, `?`, `unwrap_or`, `ok_or` |
| `Err(nil)` | Looks like Ok (`err == nil`) | Non-nil error values |
| `.map` on Result without type | Treated as Option `.map` | `let r: Result<...> = ...` |
| Assuming `?` converts errors | No `From` at transpile time | Same Err type, or `map_err` |

## See also

- [Supported Rust](/guides/language/) - statements, `match`, closures, collections
- [Lints](/guides/lints/) - `unwrap` / `expect` and other transpile diagnostics
- [API types](/guides/api-types/) - optional concept fields as `Option<T>`
- [Events](/guides/events/) - typical `if let Some(...)` on event payloads
