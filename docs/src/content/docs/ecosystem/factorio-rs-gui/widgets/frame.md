---
title: Frame
description: Frame builder - caption, layout direction, alignment, centering, and children.
---

A titled container that lays out children.

```rust
use factorio_rs_gui::shared::frame::Frame;
use factorio_rs_gui::shared::text::Text;

Frame::new()
    .caption("Counter")
    .centered()
    .direction(GuiDirection::Vertical)
    .align_horizontal(LuaStyleHorizontalAlign::Center)
    .align_vertical(LuaStyleVerticalAlign::Center)
    .child(Text::new("Hello"))
```

## Builder API

| Method | Effect |
| --- | --- |
| `new()` | Empty frame builder |
| `caption(&str)` | Frame title |
| `name(&str)` | Stable element name (optional) |
| `direction(GuiDirection)` | `Horizontal` / `Vertical` layout |
| `align_horizontal(...)` | `LuaStyle` horizontal align |
| `align_vertical(...)` | `LuaStyle` vertical align |
| `centered()` | Center in `player.gui.screen` after mount |
| `child(impl Into<Widget>)` | Append a child |

Alignment helpers are type aliases from the Factorio API
(`GuiDirection`, `LuaStyleHorizontalAlign`, `LuaStyleVerticalAlign`) via
`factorio_rs_gui::shared::align`.

## Root name

When you [`mount`](../../guides/lifecycle/) a GUI, the runtime calls
`ensure_name(root_name)` on the root frame if you did not set `.name(...)`.
You usually omit `.name` on the root and pass a mod-unique string to `mount`.

## Centering and drag

`centered()` sets `auto_center` and calls `force_auto_center` after children
mount. Rebuild restores a saved drag location when the player has moved the
window, so centering does not fight position persistence across state updates.

Only applies when the frame is a **direct** child of `player.gui.screen`.
