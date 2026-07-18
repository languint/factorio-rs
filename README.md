<div align="center">
    <img src="https://raw.githubusercontent.com/languint/factorio-rs/HEAD/docs/src/assets/logo.svg" alt="factorio-rs" width="160" height="160">
    <h1>factorio-rs</h1>
    <p>Write Factorio mods in Rust. Transpile to loadable Lua mods.</p>
    <p>
      <a href="https://crates.io/crates/factorio-rs"><img alt="crates.io" src="https://img.shields.io/crates/v/factorio-rs.svg"></a>
      <a href="https://crates.io/crates/factorio-rs-cli"><img alt="factorio-rs-cli" src="https://img.shields.io/crates/v/factorio-rs-cli.svg"></a>
      <a href="https://languint.github.io/factorio-rs/"><img alt="docs" src="https://img.shields.io/badge/docs-online-blue"></a>
      <img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85-blue">
      <a href="LICENSE"><img alt="license" src="https://img.shields.io/badge/license-MIT-green"></a>
    </p>
</div>

> [!NOTE]
> This project is in development; expect breaking changes.

**factorio-rs** is a Rust authoring layer for Factorio mods. You write a
Factorio-oriented subset of Rust; the CLI typechecks against generated API stubs, applies transpile-time safety lints, and emits a normal Factorio mod (Lua + `info.json` + locale/assets). The game still loads Lua, this is not native code inside Factorio.

## Why

- **Typed Factorio APIs** — `cargo check` / rust-analyzer against generated stubs
- **Transpile safety** — lints catch Rust patterns that nil-crash or miscompile in Lua
- **Cargo-shaped deps** — share APIs between mods with `#[factorio_rs::export]` and normal Cargo dependencies
- **Familiar workflow** — `init` / `check` / `build` / `package` / `install` / `test`

| | Lua mods | factorio-rs |
| --- | :---: | :---: |
| Runs as Factorio Lua | ✅ | ✅ |
| Typed Factorio API stubs | ❌ | ✅ |
| rust-analyzer / `cargo check` | ❌ | ✅ |
| Transpile-time safety lints | ❌ | ✅ |
| CLI packaging (`info.json`, stages) | ❌ | ✅ |
| Typed cross-mod exports via Cargo | ❌ | ✅ |
| Full Lua language surface | ✅ | ❌ |
| Rust syntax (supported subset) | ❌ | ✅ |

## Quick start

Requires **Rust 1.85+** (edition 2024).

```bash
cargo install factorio-rs-cli
mkdir my-mod && cd my-mod
factorio-rs init --name my-mod
factorio-rs build
```

`dist/` is a loadable Factorio mod. Use `factorio-rs install` / `open` when you
have a Factorio install; use `factorio-rs test` to run in-game `#[test]`s.

## Pipeline

1. Typecheck with `cargo check` (API stubs + Cargo deps)
2. Discover stage modules (`control`, `settings`, `data`, …)
3. Lower Rust -> IR and apply transpile lints
4. Optionally prune unreachable code
5. Emit Lua under `output_dir` (default `dist/`)

## Docs

- **Book:** https://languint.github.io/factorio-rs/
- **Guides:** [Getting started](https://languint.github.io/factorio-rs/guides/getting-started/) ·
  [Language](https://languint.github.io/factorio-rs/guides/language/) ·
  [Lints](https://languint.github.io/factorio-rs/guides/lints/) ·
  [Dependencies](https://languint.github.io/factorio-rs/guides/dependencies/) ·
  [Testing](https://languint.github.io/factorio-rs/guides/testing/)
- **crates.io:** [factorio-rs](https://crates.io/crates/factorio-rs) ·
  [factorio-rs-cli](https://crates.io/crates/factorio-rs-cli)
- **Changelog:** [CHANGELOG.md](CHANGELOG.md)

Preview the docs site locally:

```bash
cd docs
pnpm install
pnpm dev
```

## Examples

| Example                                                           | What it shows                     |
| ----------------------------------------------------------------- | --------------------------------- |
| [`hello_world`](examples/hello_world)                             | Minimal control-stage event       |
| [`locale_test`](examples/locale_test)                             | `locale!` and mod settings        |
| [`provider`](examples/provider) / [`consumer`](examples/consumer) | Cross-mod exports via Cargo       |
| [`tracing_test`](examples/tracing_test)                           | Optional `tracing` -> `game.print` |
| [`mandatory_spaghetti`](examples/mandatory_spaghetti)             | Larger control-stage sample       |

## License

MIT
