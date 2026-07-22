---
title: Reactive GUI
description: Build a reactive Factorio GUI with factorio-rs-gui builders, state!, and mount.
---

Use builder-style widgets that rebuild when state changes.

## Shape

```rust
use factorio_rs::{
    factorio_api::{IndexOrName, lua_fn, lua_fn0},
    prelude::*,
};
use factorio_rs_gui::shared::button::Button;
use factorio_rs_gui::shared::frame::Frame;
use factorio_rs_gui::shared::text::Text;
use factorio_rs_gui::shared::widget::Widget;

const ROOT: &str = "my_mod_counter";

fn app() -> impl Into<Widget> {
    let count = factorio_rs_gui::state!(0);
    let label = format!("Count: {}", count.get());
    let increment = lua_fn(move |event: OnGuiClickEvent| {
        let _ = event;
        count.set(count.get() + 1);
    });

    Frame::new()
        .caption("Counter")
        .centered()
        .align_horizontal(LuaStyleHorizontalAlign::Center)
        .align_vertical(LuaStyleVerticalAlign::Center)
        .direction(GuiDirection::Vertical)
        .child(Text::new(&label))
        .child(Button::new("Increment").on_click(increment))
}

#[factorio_rs::event(OnPlayerCreated)]
pub fn on_player_created(event: OnPlayerCreatedEvent) {
    if let Some(player) = game.get_player(IndexOrName::Index(event.player_index)) {
        factorio_rs_gui::shared::runtime::mount(
            player.gui().screen(),
            ROOT,
            lua_fn0(app),
        );
    }
}

#[factorio_rs::event(OnTick)]
pub fn on_tick(_event: OnTickEvent) {
    // Re-bind after `game.reload_mods()` / hot-reload (module locals wipe).
    factorio_rs_gui::shared::runtime::restore(ROOT, lua_fn0(app));
}

// Required: handlers are in *this* mod's `storage`.
#[factorio_rs::event(OnGuiClick)]
pub fn on_gui_click(event: OnGuiClickEvent) {
    factorio_rs_gui::shared::runtime::dispatch_click(event);
}
```

## How it works

1. `state!(init)` allocates a hook slot that survives rebuilds (namespaced per root).
2. `mount(parent, root_name, app)` stores the app, applies `root_name` to the
   root [`Frame`](https://lua-api.factorio.com/latest/classes/LuaGuiElement.html),
   and builds the tree. Use a **mod-unique** `root_name` so other GUIs on
   `player.gui.screen` do not collide.
3. Button `on_click` registers handlers in **your** mod; your `OnGuiClick` must
   call `dispatch_click`.
4. `State::set` marks dirty and **rebuilds** that root (destroy + re-run `app`).
5. Call `restore(root_name, app)` after script reload (e.g. from `on_tick`).

Multiple windows in one mod: mount each with a different `root_name`. Hooks,
handlers, and locations are isolated per root. `unmount(root_name)` tears one
down.

Adaptations from a fully reactive DSL: use `format!` (not `"Count: {count}"`
literals), `lua_fn` / `lua_fn0` for callbacks, and a concrete `Widget` enum.
`Frame::child` takes `impl Into<Widget>`, so pass `Text` / `Button` / `Frame`
directly (backed by `impl From<...> for Widget`). Prefer
`factorio_rs_gui::shared::...` paths so Factorio `require`s resolve.

## Try it

```bash
cd ecosystem/factorio-rs-gui && factorio-rs build
cd examples/gui_counter
factorio-rs build && factorio-rs install --open
```

Working tree:
[`examples/gui_counter`](https://github.com/languint/factorio-rs/tree/main/examples/gui_counter).

## See also

- [Getting started](../getting-started/) / [State](../state/) / [Lifecycle](../lifecycle/)
- [Widgets](../../widgets/)
- [GUI basics](../../../recipes/gui-basics/) - imperative `LuaGuiElementAddParams`
- [Sharing code between mods](../../../guides/dependencies/) - library deps
- [Persist with storage](../../../recipes/persist-storage/) - hook values use `storage`
