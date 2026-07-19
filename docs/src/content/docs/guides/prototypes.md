---
title: Prototypes
description: Register Factorio data-stage prototypes with typed stubs, item!, and recipe!.
---

Prototype registration happens in the **data** stage (`data.rs`,
`#[factorio_rs::data]`, or `data_mod!`). factorio-rs gives you:

- typed stub structs (`Item`, `Recipe`, ...) for `data.extend`
- macros (`item!`, `recipe!`) that expand to name constants + a `pub fn`
  register helper
- codegen that injects Factorio’s `type = "..."` discriminant from the Rust
  struct name

Every **`pub fn`** in a data-stage module runs from `data.lua` at load time -
see [Stages](stages/).

## Hand-written stubs

Import from the prelude and call `data.extend` with struct literals. Prefer
`..Default::default()` so unset optional fields omit as Lua `nil` (sparse
tables).

```rust
use factorio_rs::prelude::*;

pub fn register_custom() {
    data.extend([
        Item {
            name: "my-mod-widget",
            icon: "__my_mod__/graphics/icon.png",
            stack_size: 50,
            icon_size: Some(64),
            ..Default::default()
        },
        Recipe {
            name: "my-mod-widget",
            energy_required: Some(1.0),
            ingredients: &[
                RecipeIngredient {
                    name: "iron-plate",
                    amount: 2,
                    ..Default::default()
                },
            ],
            results: &[
                RecipeProduct {
                    name: "my-mod-widget",
                    amount: 1,
                    ..Default::default()
                },
            ],
            category: Some("crafting"),
            enabled: Some(true),
            ..Default::default()
        },
    ]);
}
```

Emitted Lua (simplified):

```lua
data.extend({
  {
    type = "item",
    name = "my-mod-widget",
    icon = "__my_mod__/graphics/icon.png",
    icon_size = 64,
    stack_size = 50,
  },
  {
    type = "recipe",
    name = "my-mod-widget",
    energy_required = 1.0,
    ingredients = {
      { type = "item", name = "iron-plate", amount = 2 },
    },
    results = {
      { type = "item", name = "my-mod-widget", amount = 1 },
    },
    category = "crafting",
    enabled = true,
  },
})
```

### Type injection

| Rust struct | Injected Lua `type` |
| --- | --- |
| `Item` | `"item"` |
| `Recipe` | `"recipe"` |
| `RecipeIngredient` | `"item"` |
| `RecipeProduct` | `"item"` |
| `BoolSetting` / `IntSetting` / ... | `"bool-setting"` / ... (settings stage) |

Factorio 2.0 recipes need the full ingredient/product tables
(`{type, name, amount}`); the stubs always emit that shape.

Use hand-written stubs when you need fields the macros do not expose yet.

## `item!`

Declares item prototypes. Expands to:

- `Items` with `pub const` internal names (for `locale!`)
- `pub fn register()` that `data.extend`s typed `Item` literals

```rust
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
```

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | Factorio internal prototype name |
| `icon` | yes | Relative paths rewrite to `__{package.name}__/...`; `__...__` paths keep as-is |
| `stack_size` | yes | |
| `icon_size` | no | |
| `subgroup` | no | |
| `order` | no | |

Block idents become screaming consts: `widget` → `Items::WIDGET`.

Co-locate `locale!` in the same module, or put it in a sibling module and
`use crate::data::items::Items` (see [Locale](locale/)):

```rust
locale! {
    file = "items",

    en {
        item_name {
            Items::WIDGET = "Widget",
        }
        item_description {
            Items::WIDGET = "A sample item.",
        }
    }
}
```

Packaging icons and `Factorio.toml` assets: [Package graphics](../recipes/package-graphics/).

## `recipe!`

Declares recipe prototypes. Expands to:

- `Recipes` with name constants
- `pub fn register_recipes()` (named so it can sit next to `item!`’s `register()`)

```rust
use factorio_rs::prelude::*;

recipe! {
    craft_widget {
        name = "my-mod-widget",
        energy_required = 1.0,
        ingredients = [
            { name = "iron-plate", amount = 2 },
            { name = "copper-plate", amount = 1 },
        ],
        results = [
            { name = "my-mod-widget", amount = 1 },
        ],
        category = "crafting",
        enabled = true,
        subgroup = "intermediate-product",
        order = "a[my-mod]-b[widget]",
    }
}
```

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | Often matches the crafted item’s `name` |
| `ingredients` | yes | `[{ name = "...", amount = N }, ...]` |
| `results` | yes | Same table shape as ingredients |
| `energy_required` | no | Seconds; Factorio default is `0.5` when omitted |
| `category` | no | e.g. `"crafting"` |
| `enabled` | no | `true` = unlocked at game start |
| `subgroup` | no | |
| `order` | no | |

`craft_widget` → `Recipes::CRAFT_WIDGET`. Use that const in `locale!` under
`recipe_name` / `recipe_description` when you want localized recipe titles.

## Items + recipes together

Typical data module:

```rust
// src/data.rs
use factorio_rs::prelude::*;

item! {
    widget {
        name = "my-mod-widget",
        icon = "graphics/icon.png",
        stack_size = 50,
        icon_size = 64,
    }
}

recipe! {
    craft_widget {
        name = "my-mod-widget",
        energy_required = 1.0,
        ingredients = [
            { name = "iron-plate", amount = 2 },
        ],
        results = [
            { name = "my-mod-widget", amount = 1 },
        ],
        category = "crafting",
        enabled = true,
    }
}

locale! {
    file = "items",

    en {
        item_name {
            Items::WIDGET = "Widget",
        }
        recipe_name {
            Recipes::CRAFT_WIDGET = "Widget",
        }
    }
}
```

Both `register` and `register_recipes` are `pub fn`s, so both run from
`data.lua`. Keep the same string in `item!` / `recipe!` `name` fields (and in
ingredient/result `name`s) so Factorio links the recipe to the item.

When you want a single Rust source of truth for the item id, hand-write the
`Recipe` stub and pass `Items::WIDGET` for `results[0].name` (macros currently
parse component names as string literals only).

## Build check

```bash
factorio-rs build
rg 'type = "item"' dist/lua
rg 'type = "recipe"' dist/lua
rg 'my-mod-widget' dist/locale
```

## See also

- [Package graphics](../recipes/package-graphics/) - assets + `item!` end-to-end
- [Stages](stages/) - data-stage discovery and `pub fn` entry points
- [Locale](locale/) - `locale!` + `Items::*` / `Recipes::*` keys
- [Mod settings](mod-settings/) - same const + register pattern on the settings stage
- [Macros and attributes](../reference/macros/) - concise macro inventory
- [API types](api-types/) - sparse struct tables / `Default`
