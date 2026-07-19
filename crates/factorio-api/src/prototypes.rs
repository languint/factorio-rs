//! Hand-written prototype stubs for the data stage (`data.extend`).
//!
//! These are not generated from `prototype-api.json` yet. They exist so mods can
//! register common prototypes with typed struct literals; the codegen injects
//! Factorio's `type = "..."` discriminant from the Rust struct name.

/// Minimal [`ItemPrototype`](https://lua-api.factorio.com/latest/prototypes/ItemPrototype.html)
/// for `data.extend`.
///
/// Required Factorio fields: `name`, `icon` (or `icons`), `stack_size`.
/// `type = "item"` is injected by the Lua generator.
///
/// Optional fields omit as Lua `nil` when `None`. Prefer
/// `..Default::default()` for fields you do not set (same sparse-table pattern
/// as other API structs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Item {
    /// Internal prototype name (e.g. `"my-mod-widget"`).
    pub name: &'static str,
    /// Packaged icon path (e.g. `"__my_mod__/graphics/icon.png"`).
    pub icon: &'static str,
    /// Max items per inventory slot.
    pub stack_size: i64,
    /// Icon pixel size. Factorio defaults to `64` when omitted.
    pub icon_size: Option<i64>,
    /// Item subgroup id (e.g. `"intermediate-product"`).
    pub subgroup: Option<&'static str>,
    /// Sort order within the subgroup.
    pub order: Option<&'static str>,
}

/// Item ingredient for a [`Recipe`] (`type = "item"` injected).
///
/// Factorio 2.0 requires the full `{type, name, amount}` table form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecipeIngredient {
    /// Ingredient item prototype name.
    pub name: &'static str,
    /// Item count consumed.
    pub amount: i64,
}

/// Item product for a [`Recipe`] (`type = "item"` injected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecipeProduct {
    /// Result item prototype name.
    pub name: &'static str,
    /// Item count produced.
    pub amount: i64,
}

/// Minimal [`RecipePrototype`](https://lua-api.factorio.com/latest/prototypes/RecipePrototype.html)
/// for `data.extend`.
///
/// `type = "recipe"` is injected by the Lua generator. Ingredients and results
/// use [`RecipeIngredient`] / [`RecipeProduct`] (each injects `type = "item"`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Recipe {
    /// Internal prototype name (e.g. `"my-mod-widget"`).
    pub name: &'static str,
    /// Crafting ingredients.
    pub ingredients: &'static [RecipeIngredient],
    /// Crafting products.
    pub results: &'static [RecipeProduct],
    /// Crafting energy in seconds. Factorio defaults to `0.5` when omitted.
    pub energy_required: Option<f64>,
    /// Recipe category (e.g. `"crafting"`).
    pub category: Option<&'static str>,
    /// Whether the recipe is unlocked at start. Defaults to `true` in Factorio when omitted.
    pub enabled: Option<bool>,
    /// Item subgroup id.
    pub subgroup: Option<&'static str>,
    /// Sort order within the subgroup.
    pub order: Option<&'static str>,
}
