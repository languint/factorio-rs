use proc_macro::TokenStream;

mod macros;
mod proto;

macro_rules! stage_attrs {
    ($($name:ident),+ $(,)?) => {
        $(
            #[proc_macro_attribute]
            pub fn $name(args: TokenStream, input: TokenStream) -> TokenStream {
                macros::stage::$name(args, input)
            }
        )+
    };
}

macro_rules! stage_fns {
    ($( $(#[$meta:meta])* $name:ident ),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[proc_macro]
            pub fn $name(input: TokenStream) -> TokenStream {
                macros::stage::$name(input)
            }
        )+
    };
}

macro_rules! proto_fns {
    ($( $(#[$meta:meta])* $name:ident ),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[proc_macro]
            pub fn $name(input: TokenStream) -> TokenStream {
                proto::$name(input)
            }
        )+
    };
}

/// Marks a function as a Factorio-RS benchmark.
///
/// The annotated function is kept with `#[allow(dead_code)]` and a hidden
/// const marker is emitted so the frontend can discover the bench and its
/// iteration count without re-invoking the macro.
///
/// # Syntax
///
/// ```ignore
/// // Default: 1 iteration.
/// #[factorio_rs::bench]
/// pub fn my_bench() {
///     // benchmark body
/// }
///
/// // Explicit iteration count (must be >= 1).
/// #[factorio_rs::bench(iterations = 100)]
/// pub fn heavy_bench() {
///     // body run 100 times per measurement
/// }
/// ```
///
/// Bench functions may be placed next to control code or inside
/// `#[cfg(test)]` modules; the frontend discovers them in both locations.
#[proc_macro_attribute]
pub fn bench(args: TokenStream, input: TokenStream) -> TokenStream {
    macros::bench::bench(args, input)
}

/// Marks a control-stage function as a Factorio event handler.
///
/// The event is inferred from the handler name and first parameter type
/// (`OnBuiltEntityEvent`). Filters are validated at compile time via a generated
/// const expression.
///
/// # Examples
///
/// Without filter:
/// ```ignore
/// #[factorio_rs::event]
/// pub fn on_singleplayer_init(event: OnSingleplayerInitEvent) {}
/// ```
///
/// With filter (filter expression is type-checked at compile time):
/// ```ignore
/// #[factorio_rs::event(filter = [OnBuiltEntityFilter::type_("inserter")])]
/// pub fn on_built_entity(event: OnBuiltEntityEvent) {}
/// ```
#[proc_macro_attribute]
pub fn event(args: TokenStream, input: TokenStream) -> TokenStream {
    macros::event::event(args, input)
}

stage_attrs!(
    settings,
    settings_updates,
    settings_final_fixes,
    data,
    data_updates,
    data_final_fixes,
    control,
);

/// Marks a file or inline `mod` as shared-stage code for transpilation.
///
/// Shared modules may be required by any other stage.
#[proc_macro_attribute]
pub fn shared(args: TokenStream, input: TokenStream) -> TokenStream {
    macros::stage::shared(args, input)
}

/// Embeds a verbatim Lua source block inside a Factorio mod function.
///
/// This macro must be used inside an `unsafe fn` or an `unsafe { }` block -
/// the transpiler enforces this requirement and emits a clear error if violated.
///
/// At the Rust level the macro expands to `()` so it is type-safe; the
/// actual code extraction happens in the frontend lowering phase.
///
/// # Example
///
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// pub unsafe fn patch_globals() {
///     lua! {
///         local old_print = print
///         print = function(...)
///             old_print("[patched]", ...)
///         end
///     }
/// }
/// ```
#[proc_macro]
pub fn lua(input: TokenStream) -> TokenStream {
    macros::lua::lua(input)
}

/// Publishes a function (or every `pub fn` in a module) as part of this mod's
/// cross-mod API.
///
/// Control-stage exports are registered with Factorio `remote.add_interface`.
/// Shared-stage exports remain requireable module functions and are included in
/// the generated `api/` stub crate.
///
/// Optional remote interface:
/// - `#[factorio_rs::export(interface)]` - remote using the mod name
/// - `#[factorio_rs::export(interface = "my_iface")]` - remote on a custom name
///
/// On a `mod` item, every public function inside inherits the export without
/// needing a per-fn attribute.
#[proc_macro_attribute]
pub fn export(args: TokenStream, input: TokenStream) -> TokenStream {
    macros::export::export(args, input)
}

/// Marks a **shared-stage** function as a hot-path library API.
///
/// Dependents call it via Lua `require` (same as a shared `#[factorio_rs::export]`),
/// never `remote.call`. Implies export for Cargo / Factorio packaging.
///
/// Invalid outside `shared` - move pure helpers there for near-native calls.
#[proc_macro_attribute]
pub fn inline(args: TokenStream, input: TokenStream) -> TokenStream {
    macros::export::inline(args, input)
}

stage_fns! {
    /// Declares a settings-stage module from a block of items.
    settings_mod,
    /// Declares a settings-updates-stage module from a block of items.
    settings_updates_mod,
    /// Declares a settings-final-fixes-stage module from a block of items.
    settings_final_fixes_mod,
    /// Declares a data/prototype-stage module from a block of items.
    data_mod,
    /// Declares a data-updates-stage module from a block of items.
    data_updates_mod,
    /// Declares a data-final-fixes-stage module from a block of items.
    data_final_fixes_mod,
    /// Declares a control/runtime-stage module from a block of items.
    control_mod,
    /// Declares a shared-stage module from a block of items.
    shared_mod,
}

/// Declare mod settings in a single, concise block.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// factorio_rs::mod_settings! {
///     prefix = "ms",
///
///     startup {
///         casual_mode: bool = false,
///         adjacency_enabled: bool = true,
///     }
///
///     runtime_global {
///         max_entities: i64 = 100,
///     }
/// }
/// ```
///
/// Access in control stage:
/// ```ignore
/// let enabled = settings.startup.get::<bool>(Settings::CASUAL_MODE);
/// ```
#[proc_macro]
pub fn mod_settings(input: TokenStream) -> TokenStream {
    macros::mod_settings::mod_settings(input)
}

/// Declare data-stage item prototypes.
///
/// Expands to an `Items` type with name constants (for `locale!`) and
/// `pub fn register()` that calls `data.extend` with [`Item`] literals.
/// Relative `icon` paths are prefixed with `__{CARGO_PKG_NAME}__/`.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// item! {
///     widget {
///         name = "my-mod-widget",
///         icon = "graphics/icon.png",
///         stack_size = 50,
///         icon_size = 64,
///     }
/// }
///
/// locale! {
///     en {
///         item_name {
///             Items::WIDGET = "Widget",
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn item(input: TokenStream) -> TokenStream {
    proto::item(input)
}

/// Declare data-stage recipe prototypes.
///
/// Expands to a `Recipes` type with name constants (for `locale!`) and
/// `pub fn register_recipes()` that calls `data.extend` with [`Recipe`]
/// literals. Prefer `register_recipes` over `register` so `item!` and
/// `recipe!` can coexist in one module.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// recipe! {
///     craft_widget {
///         name = "my-mod-widget",
///         energy_required = 1.0,
///         ingredients = [
///             { name = "iron-plate", amount = 2 },
///         ],
///         results = [
///             { name = "my-mod-widget", amount = 1 },
///         ],
///         category = "crafting",
///         enabled = true,
///     }
/// }
/// ```
#[proc_macro]
pub fn recipe(input: TokenStream) -> TokenStream {
    proto::recipe(input)
}

/// Declare data-stage technology prototypes.
///
/// Expands to a `Technologies` type with name constants (for `locale!`) and
/// `pub fn register_technologies()` that calls `data.extend` with
/// [`Technology`] literals.
///
/// # Example
/// ```ignore
/// use factorio_rs::prelude::*;
///
/// technology! {
///     widget_tech {
///         name = "my-mod-widget",
///         icon = "graphics/technology.png",
///         icon_size = 256,
///         prerequisites = ["automation"],
///         unlock_recipes = ["my-mod-widget"],
///         unit_count = 50,
///         unit_time = 30.0,
///         unit_ingredients = [
///             { name = "automation-science-pack", amount = 1 },
///         ],
///     }
/// }
/// ```
#[proc_macro]
pub fn technology(input: TokenStream) -> TokenStream {
    proto::technology(input)
}

/// Declare Factorio locale entries in Rust.
///
/// Keys that reference associated constants (e.g. `Settings::CASUAL_MODE`) are
/// type-checked by rustc. The frontend resolves them to the constant's string
/// value when assembling `locale/<lang>/*.cfg`.
///
/// # Example
/// ```ignore
/// factorio_rs::locale! {
///     file = "settings",
///
///     en {
///         mod_setting_name {
///             Settings::CASUAL_MODE = "Casual mode",
///         }
///         mod_setting_description {
///             Settings::CASUAL_MODE = "Relax some rules.",
///         }
///         "my-mod-messages" {
///             "hello" = "Hello engineer!",
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn locale(input: TokenStream) -> TokenStream {
    macros::locale::locale(input)
}

proto_fns! {
    /// Declare data-stage fluid prototypes.
    fluid,
    /// Declare data-stage assembling-machine entity prototypes.
    assembling_machine,
    container,
    inserter,
    transport_belt,
    furnace,
    mining_drill,
    lab,
    resource,
    tile,
    autoplace_control,
    recipe_category,
    item_group,
    item_subgroup,
}

#[proc_macro]
pub fn module(input: TokenStream) -> TokenStream {
    proto::module_proto(input)
}
