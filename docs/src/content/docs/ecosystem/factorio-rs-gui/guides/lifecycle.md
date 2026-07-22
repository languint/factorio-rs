---
title: Lifecycle
description: mount, restore, rebuild, dispatch_click, and unmount for factorio-rs-gui.
---

The runtime owns Factorio lifecycle glue. Import from
`factorio_rs_gui::shared::runtime`.

## Mount

```rust
factorio_rs_gui::shared::runtime::mount(
    player.gui().screen(),
    "my_mod_window",
    lua_fn0(app),
);
```

- Parent is usually `player.gui().screen()`.
- `root_name` must be **mod-unique** among siblings on that parent.
- Applies `root_name` to the root [`Frame`](../../widgets/frame/) when unset.
- Stores the `app` closure and builds the tree.

## Restore (hot reload)

After `game.reload_mods()` / hot reload, module locals wipe. Re-bind on tick
(or another safe event):

```rust
factorio_rs_gui::shared::runtime::restore("my_mod_window", lua_fn0(app));
```

## Clicks

Handlers are registered in **your** mod's `storage`. Forward every GUI click:

```rust
#[factorio_rs::event(OnGuiClick)]
pub fn on_gui_click(event: OnGuiClickEvent) {
    factorio_rs_gui::shared::runtime::dispatch_click(event);
}
```

A library-mod event handler cannot see another mod's `storage`.

## Rebuild and unmount

| Call | When |
| --- | --- |
| `State::set` | Marks dirty and rebuilds that root (destroy + re-run `app`) |
| `rebuild` / `rebuild_root` | Manual rebuild APIs |
| `unmount(root_name)` | Tear down one window |

v1 rebuilds the **whole** tree for a root when state changes.

## Constants

`ROOT_NAME` (`"frg_root"`) is a default for single-GUI experiments. Prefer an
explicit mod-prefixed string in real mods. See [Multiple windows](../multiple-windows/).
