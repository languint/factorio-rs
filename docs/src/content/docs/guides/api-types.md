---
title: API types
description: How Factorio API stubs are typed, Identification enums, and when LuaAny remains.
---

factorio-rs generates Rust stubs from Factorio’s `runtime-api.json` so `cargo check`
and the IDE can validate mod code. The stubs never run in Factorio, the CLI
transpiles your sources to Lua.

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
and `ForceSet` are generated as enums. Prefer **exact constructors**; the
frontend lowers them to the Factorio payload (no `.into()` needed):

```rust
pub enum ForceID {
    Name(&'static str),
    Index(u8),
    Force(LuaForce),
}

surface.find_entities_filtered(EntitySearchFilters {
    area: Some(area),
    force: Some(ForceSet::One(ForceID::Force(source.force()))),
    name: Some(EntityID::Name(source.name())),
    ..Default::default()
});
```

`From` impls still exist for compatibility, but constructors keep the intended
arm visible at the call site.

`T | array<T>` in the schema collapses to `T` at the stub layer (the array form
is still valid in Lua; it just isn’t modeled in Rust yet).

Anonymous `uint32 | string` parameters (`game.get_player`, `game.get_surface`,
...) use `IndexOrName`:

```rust
if let Some(player) = game.get_player(IndexOrName::Index(player_index)) {
    // ...
}
```

## Flag sets, Tags, and LuaStructs

- **Flag sets** (`MouseButtonFlags`, `SelectionModeFlags`,
  `EntityPrototypeFlags`, ...): `Copy` structs with `flags: &'static [&str]`.
  Lowers to `{ ["left"] = true, ... }`.
- **`Tags`**: `Tags { pairs: &[TagPair { key, value }] }` for string values.
- **`GameViewSettings` / `MapSettings` / `DifficultySettings`**: generated from
  schema `LuaStruct` attributes (no longer `LuaAny`).
- **`MapGenSize` / `RenderLayer`**: payload enums (`Number` / `Named`);
  constructors lower like Identification enums.
- **`script.on_event` filters**: `Option<Vec<EventFilterEntry>>` (same entries as
  `#[factorio_rs::event(filter = ...)]`).

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

## Attribute reads and writes

Factorio class attributes lower as:

| Rust | Lua |
| --- | --- |
| `elem.caption()` | `elem.caption` (property read) |
| `elem.set_caption("Hi")` | `elem.caption = "Hi"` |
| `elem.style().set_width(32)` | `elem.style.width = 32` |
| `elem.set_style("frame_style")` | `elem.style = "frame_style"` |
| `entity.set_filter(...)` | `entity.set_filter(...)` (real method - unchanged) |

Writable attributes get a `set_<name>` stub. When that name collides with a real
Factorio method (rare: `driving`, `zoom_limits`, ...), the writer is named
`write_<name>` instead.

`LuaGuiElement.style()` returns `LuaStyle` (class). The writer `set_style` takes a
style name `&'static str` - Factorio accepts either a `LuaStyle` or a string for
that attribute; Rust keeps the asymmetric shapes that match typical usage.

End-to-end walkthrough: [GUI basics](../recipes/gui-basics/).

Write-only attributes (for example most `LuaStyle` size/margin helpers) have
**setters only** - there is no fake `LuaAny` getter.

Field assignment (`target.field = value`) still works for struct fields and
lowers to the same Lua property write form.

## When `LuaAny` remains

Keep using `LuaAny` (or expect it) for truly open values:

- `Any` / `AnyBasic` payloads (including non-string Tag values)
- Unstructured anonymous `table` parameters
- Prototype graphics / animation packs still skipped by the classifier
- `storage[...]` indexing and polymorphic lua stdlib helpers
- Self-referential concept fields that must stay `Copy` (`MapLocation.position`)

Prefer concrete concepts, flag sets, and Identification enums whenever the stubs
expose them.

## Globals

Schema `global_objects` become prelude statics: `game`, `script`, `commands`,
`remote`, `rendering`, `rcon`, `settings`, `prototypes`, `helpers`.

Factorio also documents auxiliaries that are not in `global_objects`. The stubs
include:

- `storage` (`LuaStorage`) - persistent mod-local table (survives events + save/load)
- `serpent` - table pretty-printer (`block` / `line` / `dump`)
- `math` / `string` / `table` - standard Lua libraries

```rust
let x = math.floor(position.x);
let label = string.format_1("tick %d", game.tick().into());
table.insert(list, value);

// Persist mod state (not Rust `static` / `LazyLock` - those are unsupported)
storage.set("counter", 0_u32);
let counter = storage.get::<u32>("counter").unwrap_or(0);
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

- [Supported Rust](language/)
- [Option and Result](option-and-result/) - optional fields and fallible helpers
- [mandatory_spaghetti](../examples/mandatory-spaghetti/) - typed filters and
  `ScriptRenderTarget`
