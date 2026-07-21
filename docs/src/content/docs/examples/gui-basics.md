---
title: gui_basics
description: Control-stage example that opens a framed GUI with LuaStyle on player create.
---

Path: `examples/gui_basics`.

Mirrors the [GUI basics](../recipes/gui-basics/) recipe: on `OnPlayerCreated`, add a
screen frame + label, then tune layout with `style().set_width` / `set_padding`.

```rust
#[factorio_rs::control]
mod control {
    use factorio_rs::{
        factorio_api::{classes::LuaGuiElementAddParams, IndexOrName},
        prelude::*,
    };

    #[factorio_rs::event(OnPlayerCreated)]
    pub fn on_player_created(event: OnPlayerCreatedEvent) {
        if let Some(player) = game.get_player(IndexOrName::Index(event.player_index)) {
            let frame = player.gui().screen().add(LuaGuiElementAddParams {
                r#type: GuiElementType::Frame,
                name: Some("gui_basics_root".into()),
                caption: Some("GUI basics".into()),
                ..Default::default()
            });

            let label = frame.add(LuaGuiElementAddParams {
                r#type: GuiElementType::Label,
                caption: Some("Hello from factorio-rs".into()),
                ..Default::default()
            });

            frame.style().set_width(280);
            label.style().set_padding(8);
        }
    }
}
```

## Try it

```bash
cd examples/gui_basics
factorio-rs build
factorio-rs install --open
```

Create a new player (or join a map). You should see a titled frame with a label.

Mod id: Cargo package name `gui_basics`.

See also: [GUI basics](../recipes/gui-basics/), [API types](../guides/api-types/),
[Events](../guides/events/), [First hour](../recipes/first-hour/).
