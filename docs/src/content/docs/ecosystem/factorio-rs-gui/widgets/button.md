---
title: Button
description: Button builder - caption, optional name, and on_click handlers.
---

A clickable button (`GuiElementType::Button`).

```rust
use factorio_rs::factorio_api::lua_fn;
use factorio_rs_gui::shared::button::Button;

let on_ok = lua_fn(move |event: OnGuiClickEvent| {
    let _ = event;
    // ...
});

Button::new("OK").on_click(on_ok)
```

## Builder API

| Method | Effect |
| --- | --- |
| `new(&str)` | Button with caption |
| `name(&str)` | Optional stable element name |
| `on_click(LuaFunction)` | Click handler (`lua_fn` / `lua_fn0` / function item) |

If `on_click` is set and you omit `.name(...)`, the runtime assigns a unique
name (`frg_btn...`) so the click can be routed.

## Handlers live in your mod

Click bindings are stored in the **consuming** mod's `storage`. Your control
stage must forward clicks:

```rust
#[factorio_rs::event(OnGuiClick)]
pub fn on_gui_click(event: OnGuiClickEvent) {
    factorio_rs_gui::shared::runtime::dispatch_click(event);
}
```

See [Lifecycle](../../guides/lifecycle/).
