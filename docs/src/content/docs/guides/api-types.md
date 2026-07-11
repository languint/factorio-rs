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
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        ..Default::default()
    }),
);
```

`..Default::default()` is ignored when lowering so only fields you set appear in
Lua (sparse Factorio parameter tables).

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
    area,
    force: source.force().into(),
    name: source.name().into(),
    ..Default::default()
});
```

`T | array<T>` in the schema collapses to `T` at the stub layer (the array form
is still valid in Lua; it just isn’t modeled in Rust yet).

Anonymous `uint32 | string` parameters (`game.get_player`, `game.get_surface`,
...) use `IndexOrName`:

```rust
if let Some(player) = game.get_player(player_index.into()) {
    // ...
}
```

## When `LuaAny` remains

Keep using `LuaAny` (or expect it) for truly open values:

- `Any` / `AnyBasic`
- Unstructured `table` / `function`
- `Tags` and similar open dictionaries
- A few leftover heterogeneous unions not covered by Identification enums

Prefer concrete concepts and Identification enums whenever the stubs expose
them. Reaching for `.into()` should mean “this API accepts several Factorio
shapes,” not “the type was erased.”

## See also

- [Language support](language/)
- [mandatory_spaghetti](../examples/mandatory-spaghetti/) - typed filters and
  `ScriptRenderTarget`
