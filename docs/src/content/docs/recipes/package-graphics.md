---
title: Package graphics
description: Copy sprites into the mod output and register a data-stage item with item!.
---

factorio-rs builds a normal Factorio mod directory. Put graphics under your
project, list them in `Factorio.toml`, and register items with `item!` so relative
icon paths become `__mod__/...` and `Items::*` constants wire into `locale!`.

For recipes, hand-written stubs, and the full `item!` / `recipe!` field tables,
see [Prototypes](../guides/prototypes/).

## 1. Add files

```text
my-mod/
  assets/graphics/icon.png
  Factorio.toml
  Cargo.toml
  src/
    lib.rs
    data.rs
```

## 2. Declare assets

```toml
[mod]
title = "My Mod"
factorio_version = "2.0"
assets = [
  { from = "assets/graphics", to = "graphics" },
]
```

Or keep the same relative path:

```toml
[mod]
assets = ["graphics"]
```

Rules (collisions, remaps, thumbnail): [Factorio.toml -> Assets](../reference/factorio-toml/#assets).

## 3. Register items + locale

Cargo `[package].name` is the mod id. Relative `icon` paths are rewritten to
`__my_mod__/graphics/...` (replace `my_mod` with your package name). Paths that
already start with `__` are left unchanged.

Co-locate `item!` and `locale!` in the same data-stage module, or import
`Items` from a sibling module (see [Locale](../guides/locale/) and
[Prototypes](../guides/prototypes/)) so `Items::CONST` keys resolve.

```rust
// src/data.rs
use factorio_rs::prelude::*;

item! {
    widget {
        name = "my-mod-widget",
        icon = "graphics/icon.png",
        stack_size = 50,
        icon_size = 64,
        subgroup = "intermediate-product",
        order = "a[my-mod]-a[widget]",
    }
}

locale! {
    file = "items",

    en {
        item_name {
            Items::WIDGET = "Widget",
        }
        item_description {
            Items::WIDGET = "A sample packaged item.",
        }
    }
}
```

`item!` expands to an `Items` type with name constants and `pub fn register()`
that calls `data.extend` with typed `Item` literals (`type = "item"` is injected).
Every `pub fn` in a data-stage module runs from `data.lua` at load time -
see [Stages](../guides/stages/).

Factorio reads `[item-name]` / `[item-description]` from locale `.cfg` files
automatically (category idents use underscores -> hyphens).

Escape hatch: hand-write `data.extend([Item { ... }])` when you need fields the
macro does not expose yet.

## 4. Build and check `dist/`

```bash
factorio-rs build
ls dist/graphics
rg 'type = "item"' dist/lua
rg 'my-mod-widget' dist/locale
```

You should see `icon.png`, an item table using `__my_mod__/graphics/...`, and
locale keys for the item name.

Emitted shape (simplified):

```lua
data.extend({
  {
    type = "item",
    name = "my-mod-widget",
    icon = "__my_mod__/graphics/icon.png",
    icon_size = 64,
    stack_size = 50,
    subgroup = "intermediate-product",
    order = "a[my-mod]-a[widget]",
  },
})
```

```ini
[item-name]
my-mod-widget=Widget
```

## Thumbnail

Portal thumbnail is separate from `assets`:

```toml
[mod]
thumbnail = "assets/thumbnail.png"  # or rely on ./thumbnail.png
```

## See also

- [Prototypes](../guides/prototypes/) - `item!` / `recipe!` / typed stubs
- [Getting started](../guides/getting-started/) - install / package
- [First hour](first-hour/) - end-to-end loop
- [Stages](../guides/stages/) - data vs control modules
- [Locale](../guides/locale/) - `locale!` + `Items::*` keys
- [Mod settings](../guides/mod-settings/) - same `register` / const pattern
