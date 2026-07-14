# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Factorio mod dependencies: `[mod].dependencies` in `Factorio.toml`, merged with
  deps from Cargo crates that publish `[package.metadata.factorio]`.
- Publishable Rust binding crates: typed stubs map `use other_crate::...` to
  foreign `__mod__/...` requires for type-safe cross-mod APIs.
- `#[factorio_rs::export]`: control-stage functions register Factorio
  `remote.add_interface`; shared exports stay requireable and prune-rooted.
  Applied to a `mod`, exports every `pub fn` inside. Supports bare
  `interface` and `interface = "name"`.
- Provider builds publish exports onto the library crate itself
  (`[package.metadata.factorio]` + `src/factorio_exports.rs`). Consumers depend
  with Cargo (`cargo add --path` / `factorio-rs add`); call `provider::fn` with
  normal path/crates.io deps (no separate stub crate).
- Real typechecking: `factorio-rs check` and `build`/`package`/`install` run
  `cargo check` against Factorio API stubs (methods, arity, types) before
  lowering. `--skip-typecheck` escapes that step.

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

[Unreleased]: https://github.com/languint/factorio-rs/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/languint/factorio-rs/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/languint/factorio-rs/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/languint/factorio-rs/releases/tag/v0.1.1
