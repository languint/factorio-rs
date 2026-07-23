//! Hand-written data-stage companions and helpers for generated prototypes.
//!
//! Generated stubs for every Factorio typename (~260).

include!(concat!(env!("OUT_DIR"), "/prototypes_gen.rs"));

/// RGBA color for fluid / graphics tables (`{r, g, b, a}`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: Option<f64>,
}

/// Axis-aligned box as two corners (`{{left_top}, {right_bottom}}` in Lua).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoundingBox {
    pub left_top_x: f64,
    pub left_top_y: f64,
    pub right_bottom_x: f64,
    pub right_bottom_y: f64,
}

/// Simplified entity energy source (`type` + optional priority / buffer fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnergySource {
    /// Factorio energy source type (e.g. `"electric"`, `"burner"`, `"void"`).
    pub r#type: &'static str,
    /// Electric usage priority (e.g. `"secondary-input"`).
    pub usage_priority: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IconData {
    /// Packaged icon path (`__mod__/graphics/...png`).
    pub icon: &'static str,
    /// Icon pixel size (commonly `64`).
    pub icon_size: Option<i64>,
}

/// Thin fluid box (`volume` + optional filter / production type). Connection
/// points and pipe covers remain sparse / hand-filled when needed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FluidBox {
    /// Fluid capacity.
    pub volume: f64,
    /// Optional fluid id filter.
    pub filter: Option<&'static str>,
    /// `"input"`, `"output"`, `"input-output"`, or `"none"`.
    pub production_type: Option<&'static str>,
}

/// Simplified minable properties for placeable entities.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MinableProperties {
    pub mining_time: f64,
    pub result: Option<&'static str>,
}

/// Item or fluid ingredient for a [`Recipe`].
///
/// Factorio 2.0 requires `{type, name, amount}`. Set [`Self::fluid`] to emit
/// `type = "fluid"`; otherwise `type = "item"` is injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecipeIngredient {
    /// Ingredient prototype name.
    pub name: &'static str,
    /// Count (items) or amount (fluids).
    pub amount: i64,
    /// When true, Lua `type = "fluid"`; otherwise `"item"`.
    pub fluid: bool,
}

/// Item product for a [`Recipe`] (`type = "item"` injected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecipeProduct {
    /// Result item prototype name.
    pub name: &'static str,
    /// Item count produced.
    pub amount: i64,
}

/// Science-pack entry for a [`TechnologyUnit`].
///
/// Emitted as a Factorio research ingredient tuple `{ "pack-name", amount }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TechnologyUnitIngredient {
    /// Tool / science-pack item name (e.g. `"automation-science-pack"`).
    pub name: &'static str,
    /// Count consumed per research unit.
    pub amount: i64,
}

/// Research cost block for a [`Technology`] (`unit = { count, time, ingredients }`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TechnologyUnit {
    /// How many lab cycles are required.
    pub count: i64,
    /// Seconds per cycle.
    pub time: f64,
    /// Science packs per cycle.
    pub ingredients: &'static [TechnologyUnitIngredient],
}

/// Unlock-recipe modifier (`type = "unlock-recipe"` injected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnlockRecipeEffect {
    /// Recipe prototype name unlocked when the technology is researched.
    pub recipe: &'static str,
}
