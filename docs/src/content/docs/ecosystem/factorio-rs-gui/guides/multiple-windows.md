---
title: Multiple windows
description: Mount several factorio-rs-gui roots without colliding on player.gui.screen.
---

Each mounted GUI needs a unique `root_name`. Hooks, click handlers, and saved
locations are namespaced per root.

```rust
const INVENTORY: &str = "my_mod_inventory";
const SETTINGS: &str = "my_mod_settings";

factorio_rs_gui::shared::runtime::mount(screen, INVENTORY, lua_fn0(inventory_app));
factorio_rs_gui::shared::runtime::mount(screen, SETTINGS, lua_fn0(settings_app));
```

## Rules

1. Use a **mod-unique** prefix (`my_mod_...`) so other mods' screen children do not collide.
2. Call `restore` for each root you care about after reload.
3. `unmount(root_name)` removes one window; others stay mounted.
4. Hook order is per root, each `app` has its own `state!` sequence.

## Why not share one root?

`state!` and click registration key off the current root. Mixing unrelated UIs
in one tree forces a full rebuild of everything when any hook changes. Separate
roots keep rebuilds and storage isolated.
