---
title: Getting started
description: Add factorio-rs-gui and mount your first reactive window.
---

## 1. Depend on the crate

```toml
[dependencies]
factorio-rs-gui = "0.1"
```

Also install the
[`factorio-rs-gui`](https://mods.factorio.com/mod/factorio-rs-gui) **library mod**
from the [Factorio mod portal](https://mods.factorio.com/mod/factorio-rs-gui)
and enable it with your mod. Your build emits `require("__factorio-rs-gui__/…")`
and lists `factorio-rs-gui` in `info.json` dependencies.

For local monorepo work (examples in this repo):

```toml
factorio-rs-gui = { path = "../../ecosystem/factorio-rs-gui" }
```

## 2. Build an `app`

```rust
use factorio_rs::factorio_api::{lua_fn, lua_fn0};
use factorio_rs::prelude::*;
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
        .direction(GuiDirection::Vertical)
        .child(Text::new(&label))
        .child(Button::new("Increment").on_click(increment))
}
```

## 3. Wire Factorio events

```rust
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
    factorio_rs_gui::shared::runtime::install(ROOT, lua_fn0(app));
}
```

No `OnGuiClick` stub: `mount` / `install` register clicks via `script.on_event`.
Do not also define `#[factorio_rs::event(OnGuiClick)]` (it would replace that
handler). Extra click logic: [`runtime::on_click`](../lifecycle/).

## 4. Try the example

From this repository (builds the library mod + the counter):

```bash
cd ecosystem/factorio-rs-gui && factorio-rs build && factorio-rs install
cd examples/gui_counter
factorio-rs build && factorio-rs install --open
```

Or depend on crates.io, enable the portal mod, and `factorio-rs build` /
`install` your own project.

Next: [State](../state/), [Lifecycle](../lifecycle/), [Widgets](../../widgets/).
