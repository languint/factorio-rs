---
title: GUI basics
description: Open a framed GUI on an event, set captions, and style with LuaStyle.
---

Build a small on-screen frame from the control stage: create the element, set
text, then tune layout through `LuaStyle`.

Requires a control-stage module ([Stages](../guides/stages/)). Attribute
reads/writes: [API types](../guides/api-types/).

## 1. Create a frame on player join

```rust
use factorio_rs::{
    factorio_api::{classes::LuaGuiElementAddParams, IndexOrName},
    prelude::*,
};

#[factorio_rs::event(OnPlayerCreated)]
pub fn on_player_created(event: OnPlayerCreatedEvent) {
    if let Some(player) = game.get_player(IndexOrName::Index(event.player_index)) {
        let frame = player.gui().screen().add(LuaGuiElementAddParams {
            r#type: GuiElementType::Frame,
            name: Some("my_mod_root".into()),
            caption: Some("My mod".into()),
            ..Default::default()
        });

        let label = frame.add(LuaGuiElementAddParams {
            r#type: GuiElementType::Label,
            caption: Some("Hello from factorio-rs".into()),
            ..Default::default()
        });

        // Use the style object for size / spacing (typed LuaStyle).
        frame.style().set_width(280);
        label.style().set_padding(8);

        // Optional: swap the whole style prototype by name.
        // frame.set_style("inside_shallow_frame");
    }
}
```

`r#type` is Rust’s way of naming Factorio’s `type` field (`frame`, `label`, ...).
`GuiElementType` lives in the prelude via generated unions.
`IndexOrName::Index(player_index)` is the typed form Factorio accepts for
`get_player` (prefer constructors over `.into()`).

## 2. What lowers to Lua

| Rust | Lua |
| --- | --- |
| `player.gui().screen().add(...)` | `player.gui.screen.add{ type = "frame", ... }` |
| `frame.set_caption("Hi")` | `frame.caption = "Hi"` |
| `frame.style().set_width(280)` | `frame.style.width = 280` |
| `frame.set_style("...")` | `frame.style = "..."` |

Caption can also be set at create time (`caption: Some(...)`) as above, or later
with `set_caption`.

## 3. Rebuild and try it

Working tree: [`examples/gui_basics`](../examples/gui-basics/).

```bash
cd examples/gui_basics
factorio-rs build && factorio-rs install --open
```

Create a new player (or join a map). You should see a titled frame with a label.

## Tips

- Give elements unique `name`s under the same parent if you need to find them
  later (`frame["child_name"]` / `children`).
- Prefer `style().set_*` for width/height/padding; use `set_style("name")` when
  you want a style **prototype**, not one property.
- `Tags` uses `Tags { pairs: &[TagPair { .. }] }` for string values; choose-elem
  filters are still sparse - stick to frame/label/button for starters.

## See also

- [gui_basics example](../examples/gui-basics/) - full crate in the repo
- [API types](../guides/api-types/) - attribute writers and `style()` typing
- [Events](../guides/events/) - `OnPlayerCreated` and filters
- [First hour](first-hour/) - init / build / install loop
- [Prototypes](../guides/prototypes/) - data-stage items / recipes / tech
