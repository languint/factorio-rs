<div align="center">
    <img src="https://raw.githubusercontent.com/languint/factorio-rs/HEAD/docs/src/assets/logo.svg" alt="factorio-rs" width="160" height="160">
    <h1>factorio-rs</h1>
    <p>Write Factorio mods in Rust. Transpile to loadable Lua mods.</p>
    <p>
      <a href="https://crates.io/crates/factorio-rs"><img alt="crates.io" src="https://img.shields.io/crates/v/factorio-rs.svg"></a>
      <a href="https://crates.io/crates/factorio-rs-cli"><img alt="factorio-rs-cli" src="https://img.shields.io/crates/v/factorio-rs-cli.svg"></a>
      <a href="https://languint.github.io/factorio-rs/"><img alt="docs" src="https://img.shields.io/badge/docs-online-blue"></a>
      <a href="LICENSE"><img alt="license" src="https://img.shields.io/badge/license-MIT-green"></a>
    </p>
</div>

> [!NOTE]
> This project is in development, expect breaking changes.

## Docs

- **Online:** https://languint.github.io/factorio-rs/
- **crates.io:** [factorio-rs](https://crates.io/crates/factorio-rs) · [factorio-rs-cli](https://crates.io/crates/factorio-rs-cli)
- **Changelog:** [CHANGELOG.md](CHANGELOG.md)
- **Local:**

```bash
cd docs
pnpm install
pnpm dev
```

## Quick start

```bash
cargo install factorio-rs-cli
mkdir my-mod && cd my-mod
factorio-rs init --name my-mod
factorio-rs build
```
