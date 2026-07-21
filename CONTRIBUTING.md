# Contributing to factorio-rs

Thanks for helping improve factorio-rs. This project transpiles a Factorio-oriented
Rust subset into loadable Lua mods. Contributions can be code, docs, examples,
or bug reports.

## Before you start

1. Read the [docs](https://languint.github.io/factorio-rs/) - especially
   [Supported Rust](https://languint.github.io/factorio-rs/guides/language/) and
   [Lints](https://languint.github.io/factorio-rs/guides/lints/).
2. Search [existing issues](https://github.com/languint/factorio-rs/issues) before
   opening a new one.

For questions that are not bug reports, prefer the
[factorio-rs Discord](https://discord.gg/Tq8243rqmn).

## Development setup

Requirements:

- **Rust 1.88+** (edition 2024; MSRV is `1.88` - let-chains in `if` / `while`)
- Optional: Factorio (for `factorio-rs test`) - see
  [Testing](https://languint.github.io/factorio-rs/guides/testing/)
- Optional: Node/pnpm for the docs site under `docs/`

Clone and verify:

```bash
git clone https://github.com/languint/factorio-rs.git
cd factorio-rs
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Build example mods with the CLI:

```bash
cargo run -p factorio-rs-cli -- check --manifest-path examples/hello_world/Cargo.toml
cargo run -p factorio-rs-cli -- build --manifest-path examples/hello_world/Cargo.toml
```

Docs preview:

```bash
cd docs
pnpm install
pnpm dev
```

## Project layout

| Path | Role |
| --- | --- |
| `crates/factorio-rs` | SDK facade + prelude |
| `crates/factorio-rs-cli` | `factorio-rs` CLI |
| `crates/factorio-api` / `factorio-api-gen` | Generated + hand stubs for Factorio APIs |
| `crates/factorio-frontend` | Rust -> IR lowering |
| `crates/factorio-ir` | IR, lints, prune |
| `crates/factorio-codegen` | IR -> Lua |
| `crates/factorio-macros` | Proc macros (`item!`, `recipe!`, stages, ...) |
| `docs/` | Starlight documentation site |
| `examples/` | Sample mods |

## What kinds of changes help most

High leverage:

- Richer prototype companions / more dual-path macros / language surface
  (traits already exist same-module; cross-module traits welcome via issue first)
- Shrink remaining choose-elem filter / graphics `LuaAny` where concrete types fit
- Docs recipes, examples, and clearer frontend errors / lints

Good starter tasks:

- Fix docs typos / add missing cross-links
- Add a failing frontend/codegen test that documents a bug
- Improve an example’s README or comments

Please open an issue first for large features (new prototype macros, language
surface expansions, architectural refactors).

## Coding guidelines

- Match existing style; run `cargo fmt` and clippy with `-D warnings`.
- Prefer focused changes: avoid drive-by refactors in the same PR.
- Frontend features that users write as macros often need a **dual path**:
  proc macro in `factorio-macros` **and** IR expand in `factorio-frontend`
  (see `item!` / `recipe!` / `mod_settings!`).
- Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
- Docs that teach a workflow belong under `docs/src/content/docs/` and in the
  sidebar in `docs/astro.config.mjs`.

## Tests

- Prefer unit/integration tests next to the crate you change
  (`crates/*/tests/`, `#[cfg(test)]` modules).
- For transpile behavior, assert IR or emitted Lua as existing tests do.
- In-game `#[test]` / `factorio-rs test` coverage is valuable but not required for
  every PR; document `FACTORIO_PATH` if you add Factorio-backed tests.

## Pull requests

1. Branch from `main`.
2. Keep PRs small enough to review.
3. Describe **why** the change matters; link issues.
4. Ensure fmt, clippy, and `cargo test --workspace` pass locally.
5. Do not bump the workspace version unless a release is intended.

## Reporting bugs

Include:

- factorio-rs / CLI version (`factorio-rs --version` or crates.io version)
- Rust version (`rustc -V`)
- Minimal repro (crate snippet or example patch)
- Whether it fails at `cargo check`, transpile (`factorio-rs check` / `build`),
  or in Factorio

## Security

API stubs and the transpile pipeline are **not** a sandbox. Do not treat
factorio-rs as isolating untrusted mod code. If you find a vulnerability in the
CLI packaging path or dependency handling, open a private security advisory on
GitHub when available, or contact a maintainer via the address on the GitHub profile - do not file a public issue with exploit details.

## License

By contributing, you agree that your contributions are licensed under the MIT
License (see [LICENSE](LICENSE)).
