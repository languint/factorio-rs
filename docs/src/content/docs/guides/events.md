---
title: Events and filters
description: Register Factorio events with #[factorio_rs::event] and typed filters.
---

Mark handlers with `#[factorio_rs::event]`. The CLI wires them into `control.lua` via `script.on_event`.

## Handler rules

- The **function name must match** the Factorio event name (`on_built_entity`,
  `on_singleplayer_init`, ...).
- The event type comes from the attribute **or** from a parameter whose type
  ends in `Event` (e.g. `OnBuiltEntityEvent` -> `OnBuiltEntity`).

```rust
use factorio_rs::prelude::*;
use factorio_rs::factorio_api::events::{OnBuiltEntityEvent, OnBuiltEntityFilter};

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    println!("Hello");
}

#[factorio_rs::event]
pub fn on_built_entity(event: OnBuiltEntityEvent) {
    let _ = event.entity;
}

#[factorio_rs::event(filter = [OnBuiltEntityFilter::name("inserter")])]
pub fn on_built_entity(event: OnBuiltEntityEvent) {
    println!("inserter built");
}
```

A single filter expression (not only arrays) is also accepted:

```rust
#[factorio_rs::event(filter = OnBuiltEntityFilter::type_("inserter"))]
pub fn on_built_entity(event: OnBuiltEntityEvent) {}
```

Filters are type-checked at compile time. Events that do not support filters reject a `filter =` argument.

## Generated Lua

```lua
script.on_event(defines.events.on_built_entity, function(event)
	control.on_built_entity(event)
end, { { filter = "name", name = "inserter" } })
```

(`control` may be renamed when `[emit].lua_module_prefix` is set.)

## Filter limitations

- Filter method arguments must be **string literals** (e.g. `"inserter"`), not variables or expressions.
- Only events that declare a filter type accept `filter = ...`.
- Handlers must live in a **control-stage** module.

See also [Option and Result](option-and-result/) for `if let Some(...)` /
`ok_or` / `?` on event payloads, and [Language support](language/) for other
syntax.
commonly used inside handlers.
