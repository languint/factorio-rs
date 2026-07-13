---
title: API types
description: How Factorio API stubs are typed, Identification enums, and when LuaAny remains.
---

factorio-rs generates Rust stubs from Factorio’s `runtime-api.json` so `cargo check`
and the IDE can validate mod code. The stubs never run in Factorio - the CLI
transpiles your sources to Lua - but their types drive the developer experience.

## Concepts and nested tables

Table concepts (`Color`, `MapPosition`, `BoundingBox`, `PrintSettings`, ...) are
`Copy` structs. Nested concept fields keep their real types:

```rust
game.print(
    "hi",
    Some(PrintSettings {
        color: Some(Color {
            r: Some(1.0),
            g: Some(0.0),
            b: Some(0.0),
            a: Some(1.0),
        }),
        ..Default::default()
    }),
);
```

Optional concept / takes-table fields are `Option<T>` so you can omit them with
`None` (or `..Default::default()`). Only fields you set appear in the generated
Lua table (sparse Factorio parameter tables). `None` field values are omitted
from the Lua table as well.

String aliases (`SoundPath`, ...) and numeric aliases (`RealOrientation`,
`Weight`, ...) are Rust type aliases.

Self-referential fields (today: `MapLocation.position`) stay as `LuaAny` so the
parent struct can remain `Copy`.

## Identification enums

Mixed unions such as `ForceID`, `PlayerIdentification`, `ScriptRenderTarget`,
and `ForceSet` are generated as enums with `From` arms for each payload:

```rust
pub enum ForceID {
    Name(&'static str),
    Index(u8),
    Force(LuaForce),
}

// ForceSet also accepts LuaForce / &str / u8 via From
surface.find_entities_filtered(EntitySearchFilters {
    area: Some(area),
    force: Some(source.force().into()),
    name: Some(source.name().into()),
    ..Default::default()
});
```

Prefer `.into()` on payloads (`force.into()`, `"enemy".into()`) over enum
constructors like `ForceID::Name(...)`, which are rejected by the
[`identification_ctor`](lints/#identification_ctor-e0005) lint.

`T | array<T>` in the schema collapses to `T` at the stub layer (the array form
is still valid in Lua; it just isn’t modeled in Rust yet).

Anonymous `uint32 | string` parameters (`game.get_player`, `game.get_surface`,
...) use `IndexOrName`:

```rust
if let Some(player) = game.get_player(player_index.into()) {
    // ...
}
```

## Callbacks (`LuaFunction`)

Schema `function` parameters map to `LuaFunction`, not `LuaAny`. Required
callbacks take `impl Into<LuaFunction>`; `function | nil` (e.g. `script.on_event`)
takes `impl IntoOptionalLuaFunction`, so you can pass a Rust `fn` item or
`None`:

```rust
commands.add_command("hello", "Say hello", lua_fn(hello_cmd));
script.on_event(defines::events::on_tick, on_tick);
script.on_event(defines::events::on_tick, None); // unregister
```

`lua_fn` (and `lua_fn0` / `lua_fn2`) coerce Rust `fn` items to `LuaFunction` for
`cargo check`; the frontend strips them so Lua still gets the bare function name.
`fn` pointers also convert via `From` / the optional trait.

## When `LuaAny` remains

Keep using `LuaAny` (or expect it) for truly open values:

- `Any` / `AnyBasic`
- Unstructured `table`
- `Tags` and similar open dictionaries
- A few leftover heterogeneous unions not covered by Identification enums

Prefer concrete concepts and Identification enums whenever the stubs expose
them. Reaching for `.into()` should mean “this API accepts several Factorio
shapes,” not “the type was erased.”

## Globals

Schema `global_objects` become prelude statics: `game`, `script`, `commands`,
`remote`, `rendering`, `rcon`, `settings`, `prototypes`, `helpers`.

Factorio also documents auxiliaries that are not in `global_objects`. The stubs
include:

- `storage` (`LuaStorage`) - persistent mod-local table
- `serpent` - table pretty-printer (`block` / `line` / `dump`)
- `math` / `string` / `table` - standard Lua libraries

```rust
let x = math.floor(position.x);
let label = string.format_1("tick %d", game.tick().into());
table.insert(list, value);
```

Overloads that need distinct Rust names (`random_int`, `format_2`, `insert_at`,
...) lower to the real Lua method (`random`, `format`, `insert`).

Plus global functions `log`, `localised_print`, and `table_size`.

Data-stage `data` / settings registration helpers live in `factorio_api::settings`
(also re-exported from the prelude).

## `serde_json`

Enable `factorio-rs` feature `serde`. Calls lower to `helpers.table_to_json` /
`json_to_table`, with binary via `string.pack("s", ...)`. Details:
[Serde / JSON](serde/).

## See also

- [Language support](language/)
- [mandatory_spaghetti](../examples/mandatory-spaghetti/) - typed filters and
  `ScriptRenderTarget`
