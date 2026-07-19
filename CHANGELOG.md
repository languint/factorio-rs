# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Cross-module `locale!` keys: `Items::*` / `Settings::*` / `Recipes::*`
  resolve via `use` imports or `crate::...` paths (no longer requires
  co-locating `locale!` with `item!` / `mod_settings!` / `recipe!`).
- Docs: Prototypes guide for data-stage stubs, `item!`, and `recipe!`.
- `LuaGuiElement.style()` returns `LuaStyle`; `set_style` takes a style name
  `&'static str` (asymmetric Class|string attribute mapping).
- `recipe!` data-stage macro: declares recipes with `Recipes::*` name constants
  and `pub fn register_recipes()` via `data.extend`. Ingredients/products emit
  Factorio 2.0 `{type = "item", name, amount}` tables.
- Data-stage `Recipe` / `RecipeIngredient` / `RecipeProduct` stubs
  (`type = "recipe"` / `type = "item"`) for `data.extend`.
- Attribute writers: writable Factorio properties emit `set_<name>` (or
  `write_<name>` on rare method collisions) and lower to Lua property
  assignment. Write-only attrs (e.g. `LuaStyle` width/height) no longer invent
  `LuaAny` getters.
- `item!` data-stage macro: declares item prototypes with relative icon paths
  rewritten to `__{package.name}__/...`, emits `Items::*` name constants for
  `locale!` (`item_name` / `item_description`), and `pub fn register()` via
  `data.extend`. See Package graphics recipe.
- Data-stage `Item` prototype stub (`type = "item"`) for `data.extend`, with
  packaged-icon fields. Package graphics recipe walks assets -> `__mod__/...` ->
  `Item` registration end-to-end.
- `matches!(expr, pat)` / `matches!(expr, pat if guard)`: desugars to a value
  `match` (`true` / `false`), reusing the same patterns as `match` arms.
- Lints: `option_try` (`E0012`) for `?` on call/method results (assumes Result);
  `integer_div` (`E0013`, warn) for `/` / `/=` without a float operand;
  `struct_rest` (`E0014`) for struct updates other than `..Default::default()`.

## [0.1.7] - 2026-07-18

### Fixed

- CLI resolves `version.workspace = true` when reading package metadata (workspace
  example crates no longer fail `factorio-rs build` with a Cargo.toml parse error).

## [0.1.6] - 2026-07-18

### Added

- `storage.get::<T>(key) -> Option<T>`: typed optional reads lowering to
  `storage[key]` (missing -> nil). Distinct from settings `.get` (still
  `recv[key].value`).
- Transparent `type` aliases (`type Name = ...`, `type Name<T> = ...`), including
  nested and block-local aliases. Resolved before Option/`Vec`/binding detection;
  no Lua is emitted for the alias itself.
- Numeric range `for` loops, ordered `Vec`/`.iter()` loops, and collecting
  range/Vec `.map(...).filter(...).collect()` iterator chains.
- User-defined `enum` support: unit, tuple, and named variants lower to tagged
  Lua tables (`{ tag = "..." , ... }`), with `match` patterns and inherent
  `impl` methods.
- Lints: `result_if` (`E0010`) for plain `if`/`while` on Result bindings;
  `err_nil` (`E0011`) for `Err(nil)` / `Err(None)`. `option_if` (`E0006`) now
  also fires on `while option`.
- Declared MSRV `1.85` (edition 2024); `factorio-rs init` scaffolds
  `rust-version = "1.85"`.
- CI jobs for `cargo fmt --check` and `cargo clippy -D warnings`.

### Changed

- Docs: sidebar split into Recipes / Language / Concepts / Modding (no catch-all
  “Guides”); recipes for first hour, storage, settings, iterators, enums,
  graphics, and cross-mod APIs; language pages for enums, collections, and type
  aliases; fixed truncated Profiles and Events pages; splash highlights tests
  and exports.
- README: value prop, Lua vs factorio-rs comparison, pipeline, docs/examples.
- CLI look and feel: Cargo-style status lines (aligned verbs, color via yansi),
  quieter build summary (`Finished transpile [profile] -> dist/ (N files) in ...`
  instead of dumping every generated path), cargo-test-shaped reports, and no
  duplicate `error:` after diagnostics already printed.

## [0.1.5] - 2026-07-18

### Added

- `factorio-rs test`: discover ordinary `#[test]` functions under `#[cfg(test)]`,
  transpile them into a Factorio harness, launch Factorio (headless by default),
  and print a colored `[OK]` / `[FAIL]` report. Use `--gui` to open a window
  and inspect the map after the suite. Assertion macros (`assert!`,
  `assert_eq!`, `assert_ne!`, `panic!`) lower to Lua `error(...)`.
- Multi-tick tests via `factorio_rs::test::steps().step(...).wait(n)...` with a
  shared `TestCtx` for state between steps.
- `factorio-rs build` shows an indicatif spinner and a per-phase time breakdown.

### Fixed

- Example Factorio simulation tests are marked `#[ignore]` so `cargo test` /
  CI skip them; run those suites with `factorio-rs test`.

## [0.1.4] - 2026-07-14

### Added

- Factorio mod dependencies: `[mod].dependencies` in `Factorio.toml`, merged with
  deps from Cargo crates that publish `[package.metadata.factorio]`.
- `#[factorio_rs::export]`: control-stage functions register Factorio
  `remote.add_interface`; shared exports stay requireable and prune-rooted.
  Applied to a `mod`, exports every `pub fn` inside. Supports bare
  `interface` and `interface = "name"`.
- Provider builds publish exports onto the library crate itself
  (`[package.metadata.factorio]` + `src/factorio_exports.rs`, and auto-wire
  `mod factorio_exports` in `lib.rs`). Consumers depend with Cargo
  (`cargo add --path` / `factorio-rs add`); call `provider::fn` with normal
  path/crates.io deps (no separate stub crate). Prefer richer
  `.factorio-rs/exports.json` when loading export catalogs.
- Real typechecking: `factorio-rs check` and `build`/`package`/`install` run
  `cargo check` against Factorio API stubs (methods, arity, types) before
  lowering. `--skip-typecheck` escapes that step.
- Safety lints: `option_if` (E0006), `ambiguous_try` (E0007),
  `ambiguous_method` (E0008), `skipped_mod` (E0009). Typed `Option` `?`
  lowers as nil early-return; expression-closure `?` hoists stay in the
  closure. Identification constructors no longer emit bogus Lua calls.
- Mod assets: `[mod].assets` copies graphics, sounds, and other files into the
  mod output (path-preserving or `{ from, to }` remaps). Thumbnail packaging
  unchanged.
- Persistent `storage.set(key, value)` / `storage.get::<T>(key) -> Option<T>`
  lowering to `storage[key] = value` / `storage[key]` (missing keys are nil).
- CI: workspace tests + `factorio-rs check`/`build` on example mods.

### Fixed

- String (and other non-integer) literal indexes no longer trigger the
  `variable_index` (E0004) lint, so dictionary keys like `storage["counter"]`
  typecheck cleanly.

## [0.1.3] - 2026-07-13

### Added

- `Result` lowering: `Ok` / `Err` as `{ ok = ... }` / `{ err = ... }`, `?` early-return
  hoists, `if let Ok` / `Err`, match arms, and Result methods (`is_ok`, `map`,
  `map_err`, `and_then`, ...).
- Option -> Result bridges: `ok_or` / `ok_or_else`.
- Control flow: `match` (guards, or-patterns, struct patterns), `while`, `loop`,
  `break`; `continue` works inside `for` / `while` / `loop`.
- Closures (`|x| ...` -> Lua `function`), including use with Option/Result helpers.
- Transpile lints (`unwrap`, `expect`, `format_spec`, `variable_index`,
  `identification_ctor`) with Cargo-like diagnostics and `[lints]` in
  `Factorio.toml`.
- Serde / JSON lowering (`serde` feature).
- Standard Lua libraries, `storage`, and `serpent` support.
- Debug / `{:?}` printing guided by Factorio type data.
- Function parameters in more call sites; locale example crate.
- Docs guide: [Option and Result](https://languint.github.io/factorio-rs/guides/option-and-result/).

### Fixed

- `if let Some(x)` uses `x ~= nil` so `Some(false)` / `Some(0)` enter the body.
- Option/Result helpers that reuse a receiver bind side-effecting expressions
  once (e.g. `create_entity(...).ok_or(...)` no longer calls twice).
- Exported function names fully qualified when used as values under prune.

## [0.1.2] - 2026-07-11

### Added

- Identification enums with `.into()` payloads for Factorio ID unions.
- Optional `tracing` feature: macros lower to colored `game.print`.
- Transparent `Some(x)` -> `x` for typed Option stub parameters.

## [0.1.1] - 2026-07-11

### Added

- Initial published SDK and CLI (`factorio-rs` / `factorio-rs-cli`).
- Rust -> IR -> Lua pipeline with stage modules (`control`, `data`, `settings`, ...).
- Project init / build / package / install / open commands.
- Generated Factorio API stubs, concepts, and literal unions.
- `mod_settings!`, `locale!`, events and filters, build profiles, dead-code prune.
- `format!` / `println!`, thumbnails, documentation site.

[Unreleased]: https://github.com/languint/factorio-rs/compare/v0.1.7...HEAD
[0.1.7]: https://github.com/languint/factorio-rs/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/languint/factorio-rs/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/languint/factorio-rs/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/languint/factorio-rs/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/languint/factorio-rs/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/languint/factorio-rs/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/languint/factorio-rs/releases/tag/v0.1.1
