---
title: Widgets
description: Builder widgets in factorio-rs-gui - Frame, Text, Button, and the Widget enum.
---

Builders produce a concrete [`Widget`](widget/) tree. Pass builders to
`Frame::child` via `impl Into<Widget>` (`From` impls on each builder).

| Widget | Factorio element | Page |
| --- | --- | --- |
| [`Frame`](frame/) | `frame` | Container, layout, caption |
| [`Text`](text/) | `label` | Caption label |
| [`Button`](button/) | `button` | Caption + optional `on_click` |

```rust
use factorio_rs_gui::shared::button::Button;
use factorio_rs_gui::shared::frame::Frame;
use factorio_rs_gui::shared::text::Text;
use factorio_rs_gui::shared::widget::Widget;

fn app() -> impl Into<Widget> {
    Frame::new()
        .caption("Hello")
        .direction(GuiDirection::Vertical)
        .child(Text::new("Label"))
        .child(Button::new("OK"))
}
```

Prefer `factorio_rs_gui::shared::...` paths (not a flattened prelude import alone)
so Factorio `require`s resolve correctly after transpile.
