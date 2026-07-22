# factorio-rs-gui

Reactive, builder-style GUI helpers for [factorio-rs](https://crates.io/crates/factorio-rs) mods.

Docs: <https://languint.github.io/factorio-rs/ecosystem/factorio-rs-gui/>

## Example

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

// factorio_rs_gui::shared::runtime::mount(screen, ROOT, lua_fn0(app));
// factorio_rs_gui::shared::runtime::restore(ROOT, lua_fn0(app)); // after reload
// factorio_rs_gui::shared::runtime::dispatch_click(event); // from OnGuiClick
```

Each mount takes a **unique** `root_name` (applied to the root frame for you).
`Frame::child` takes `impl Into<Widget>` via `From` impls on `Text` / `Button` /
`Frame`. Hooks and handlers are namespaced per root so multiple windows can
coexist.

v1 rebuilds the whole tree when state changes (destroy root + re-run `app`).
Handlers live in the consuming mod's `storage`, so you must call
`dispatch_click` from your own `OnGuiClick` handler.

```bash
cd ecosystem/factorio-rs-gui && factorio-rs build
cd examples/gui_counter && factorio-rs build
```
