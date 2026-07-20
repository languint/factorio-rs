---
title: Prototypes
description: Register Factorio data-stage prototypes with ~260 typed stubs and macros (item, recipe, technology, fluid, entities, ...).
---

Prototype registration happens in the **data** stage (`data.rs`,
`#[factorio_rs::data]`, or `data_mod!`). factorio-rs gives you:

- sparse typed stubs for **all** Factorio prototype typenames (~260) from
  bundled `prototype-api.json`, with auto field classification (common/entity
  packs, `LuaAny` escapes) and rich curated overrides for `Item` / `Recipe` /
  `Technology` / `Fluid` / `AssemblingMachine`
- curated companions (`RecipeIngredient`, `Color`, `EnergySource`, ...) for
  special Lua shapes
- macros (`item!`, `recipe!`, `technology!`, `fluid!`, `assembling_machine!`,
  plus entity/category helpers like `container!`, `inserter!`, ...) that expand
  to name constants + a `pub fn` register helper
- codegen that injects Factorio’s `type = "..."` discriminant from the Rust
  struct name (`prototype_lua_typename`)

Every **`pub fn`** in a data-stage module runs from `data.lua` at load time -
see [Stages](stages/).

## Typed stubs

Import from the prelude and call `data.extend` with struct literals. Prefer
`..Default::default()` so unset optional fields omit as Lua `nil` (sparse
tables).

The five rich types (`Item`, `Recipe`, `Technology`, `Fluid`,
`AssemblingMachine`) and their companions are re-exported by name from
`factorio_rs::prelude::*`. The full stub surface lives under
`factorio_api::prototypes` (also `factorio_rs::prelude::prototypes`):

```rust
use factorio_rs::prelude::*;
use factorio_rs::prelude::prototypes::Container;
// or: use factorio_api::prototypes::Container;
```

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
| `RecipeIngredient` | `"item"` or `"fluid"` (from `fluid: bool`) |
| `RecipeProduct` | `"item"` |
| `Technology` | `"technology"` |
| `UnlockRecipeEffect` | `"unlock-recipe"` |
| `TechnologyUnitIngredient` | (tuple `{ "name", amount }` - no `type` field) |
| `Fluid` | `"fluid"` |
| `AssemblingMachine` | `"assembling-machine"` |
| `BoolSetting` / `IntSetting` / ... | `"bool-setting"` / ... (settings stage) |

Factorio 2.0 recipes need the full ingredient/product tables
(`{type, name, amount}`); the stubs always emit that shape. Technology research
ingredients use Factorio’s compact tuple form instead. Fluid ingredients set
`fluid: true` (or `type = "fluid"` in `recipe!`).

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

Block idents become screaming consts: `widget` -> `Items::WIDGET`.

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

`craft_widget` -> `Recipes::CRAFT_WIDGET`. Use that const in `locale!` under
`recipe_name` / `recipe_description` when you want localized recipe titles.

## `technology!`

Declares technology prototypes that unlock recipes. Expands to:

- `Technologies` with name constants
- `pub fn register_technologies()` (separate from `item!`’s `register()` and
  `recipe!`’s `register_recipes()`)

```rust
use factorio_rs::prelude::*;

technology! {
    widget_tech {
        name = "my-mod-widget",
        icon = "graphics/technology.png",
        icon_size = 256,
        prerequisites = ["automation"],
        unlock_recipes = ["my-mod-widget"],
        unit_count = 50,
        unit_time = 30.0,
        unit_ingredients = [
            { name = "automation-science-pack", amount = 1 },
        ],
        order = "a[my-mod]-c[widget]",
    }
}
```

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | Internal tech id; often matches the unlocked recipe |
| `icon` | yes | Relative paths rewrite to `__{package.name}__/...` |
| `icon_size` | no | Technology icons are often `256` |
| `prerequisites` | no | List of prerequisite technology name strings |
| `unlock_recipes` | yes | Recipe names unlocked on research |
| `unit_count` | yes | Lab cycles required |
| `unit_time` | yes | Seconds per cycle |
| `unit_ingredients` | yes | `[{ name = "pack", amount = N }, ...]` science packs |
| `order` | no | |

`widget_tech` -> `Technologies::WIDGET_TECH`. Use that const in `locale!` under
`technology_name` / `technology_description`.

Science packs emit Factorio tuples `{ "pack-name", amount }`. Each unlock
injects `type = "unlock-recipe"`.

Cross-refs in `unlock_recipes`, `prerequisites`, and recipe ingredient `name`
fields may be string literals **or** paths like `Recipes::CRAFT_WIDGET` /
`Items::WIDGET`. Declaring macros still require a string literal `name` for
their own const tables.

Set the matching recipe’s `enabled = false` (or omit `enabled` and keep it
disabled by default in hand-written stubs) so research gates the craft.

## `fluid!`

Declares fluid prototypes. Expands to `Fluids::*` + `pub fn register_fluids()`.

```rust
fluid! {
    coolant {
        name = "my-mod-coolant",
        icon = "graphics/fluid.png",
        default_temperature = 15.0,
        base_color = { r = 0.2, g = 0.4, b = 0.8 },
        flow_color = { r = 0.3, g = 0.5, b = 0.9 },
    }
}
```

Required: `name`, `icon`, `default_temperature`, `base_color`, `flow_color`.
Optional: `icon_size`, `subgroup`, `order`, `hidden`.

Use a fluid in `recipe!` with `fluid = true` or `type = "fluid"` on an
ingredient:

```rust
ingredients = [
    { name = Fluids::COOLANT, amount = 10, fluid = true },
],
```

## `assembling_machine!`

First entity-kind macro. Expands to `AssemblingMachines::*` +
`pub fn register_assembling_machines()`.

```rust
assembling_machine! {
    widget_assembler {
        name = "my-mod-assembler",
        icon = "graphics/entity.png",
        crafting_speed = 0.5,
        crafting_categories = ["crafting"],
        energy_usage = "150kW",
        energy_type = "electric",
        usage_priority = "secondary-input",
        flags = ["placeable-neutral", "player-creation"],
        max_health = 300.0,
    }
}
```

Required: `name`, `icon`, `crafting_speed`, `crafting_categories`,
`energy_usage`. Optional: `energy_type` (default `"electric"`),
`usage_priority`, `icon_size`, `flags`, `max_health`, `module_slots`,
`subgroup`, `order`. Collision / selection boxes and `minable` can be set on
the typed stub when you need them.

## Other prototype macros

High-value dual-path macros follow the same pattern (name-const module +
`register_*` helper). See [Macros and attributes](../reference/macros/) for the
full inventory:

| Macro | Const module | Register fn | Stub |
| --- | --- | --- | --- |
| `container!` | `Containers` | `register_containers` | `Container` |
| `inserter!` | `Inserters` | `register_inserters` | `Inserter` |
| `transport_belt!` | `TransportBelts` | `register_transport_belts` | `TransportBelt` |
| `furnace!` | `Furnaces` | `register_furnaces` | `Furnace` |
| `mining_drill!` | `MiningDrills` | `register_mining_drills` | `MiningDrill` |
| `lab!` | `Labs` | `register_labs` | `Lab` |
| `resource!` | `Resources` | `register_resources` | `ResourceEntity` |
| `tile!` | `Tiles` | `register_tiles` | `Tile` |
| `autoplace_control!` | `AutoplaceControls` | `register_autoplace_controls` | `AutoplaceControl` |
| `recipe_category!` | `RecipeCategories` | `register_recipe_categories` | `RecipeCategory` |
| `item_group!` | `ItemGroups` | `register_item_groups` | `ItemGroup` |
| `item_subgroup!` | `ItemSubgroups` | `register_item_subgroups` | `ItemSubgroup` |
| `module!` | `Modules` | `register_modules` | `Module` |

Macros emit sparse tables and may omit complex required Factorio fields
(collision boxes, animations, ...). Fill those via hand-written `data.extend` on
the typed stub when needed. Complex properties may be skipped or typed as
`LuaAny` on auto-curated stubs.

## Items + recipes + technologies

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
        enabled = false,
    }
}

technology! {
    widget_tech {
        name = "my-mod-widget",
        icon = "graphics/technology.png",
        icon_size = 256,
        prerequisites = ["automation"],
        unlock_recipes = [Recipes::CRAFT_WIDGET],
        unit_count = 50,
        unit_time = 30.0,
        unit_ingredients = [
            { name = "automation-science-pack", amount = 1 },
        ],
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
        technology_name {
            Technologies::WIDGET_TECH = "Widget",
        }
    }
}
```

`register`, `register_recipes`, and `register_technologies` are all `pub fn`s,
so each runs from `data.lua`. Keep the same string in `item!` / `recipe!` /
`technology!` `name` fields (and in ingredient/result / `unlock_recipes` names)
so Factorio links the chain together. Cross-ref fields may also use paths such
as `Recipes::CRAFT_WIDGET` instead of duplicating the string.

When you want a single Rust source of truth for the item id, hand-write the
`Recipe` / `Technology` stubs and pass `Items::WIDGET` for recipe result names
or unlock targets (macros currently parse those names as string literals only).

## Build check

```bash
factorio-rs build
rg 'type = "item"' dist/lua
rg 'type = "recipe"' dist/lua
rg 'type = "technology"' dist/lua
rg 'my-mod-widget' dist/locale
```

## See also

- [Package graphics](../recipes/package-graphics/) - assets + `item!` end-to-end
- [Stages](stages/) - data-stage discovery and `pub fn` entry points
- [Locale](locale/) - `locale!` + `Items::*` / `Recipes::*` / `Technologies::*` keys
- [Mod settings](mod-settings/) - same const + register pattern on the settings stage
- [Macros and attributes](../reference/macros/) - concise macro inventory
- [API types](api-types/) - sparse struct tables / `Default`
