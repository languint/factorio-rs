---
title: Sharing code between mods
description: Export functions from one factorio-rs mod and depend on it from another with Cargo.
---

Want mod B to call a function from mod A? Mark it with `#[factorio_rs::export]`,
build A, then add A as a normal **Cargo** dependency. Call it like any Rust
crate: `provider::greet("hi")`.

```bash
# In the library mod
factorio-rs build
# writes [package.metadata.factorio] + src/factorio_exports.rs

# In the dependent mod - either:
factorio-rs add ../provider
# or:
cargo add --path ../provider

factorio-rs build
```

Working tree: [provider / consumer example](../examples/dependencies/).

## Export a function (library mod)

Only items marked with `#[factorio_rs::export]` are visible to other mods. Plain
`pub` stays private to your transpile (not a Cargo API by itself).

Use **`pub mod`** for stage modules you want dependents to see, and keep control
exports re-exported at the crate root (done automatically on build):

```rust
pub mod shared;

#[factorio_rs::control]
pub mod control {
    #[factorio_rs::export]
    pub fn greet(name: &str) {
        println!("hello, {name}");
    }
}

mod factorio_exports; // generated
pub use factorio_exports::*;
```

You can also put `#[factorio_rs::export]` on a whole `mod` to export every `pub fn`
inside it.

After `factorio-rs build`, factorio-rs:

- Registers control exports with Factorio (`remote.add_interface`)
- Keeps shared exports loadable via Lua `require`
- Writes `[package.metadata.factorio]` on **your** `Cargo.toml` (so Cargo dependents
  are discovered)
- Regenerates `src/factorio_exports.rs` (`pub use` of control remotes at crate root)

### Control vs shared

| Where you export | How other mods call it |
| --- | --- |
| Control stage (`#[factorio_rs::control]`) | Live call into your mod: `remote.call` |
| Shared stage (`shared/...`) | Load your Lua module: `require` |

Optional: rename the Factorio remote interface with
`#[factorio_rs::export(interface = "math")]` or bare `#[factorio_rs::export(interface)]`.

## Depend through Cargo

```toml
# consumer/Cargo.toml
[dependencies]
provider = { path = "../provider" }
# or a crates.io / git dependency once you publish the package
```

`factorio-rs add ../provider` is a thin helper that adds that path dep and merges
Factorio.toml dependency strings from the library’s metadata.

Then call:

```rust
#[factorio_rs::control]
mod control {
    use provider::shared::api;

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_init() {
        provider::greet("world"); // -> remote.call("provider", "greet", ...)
        api::greet("world");      // -> require("__provider__/lua/shared/api")
    }
}
```

Notes:

- Root names like `provider::greet` are control/remote exports (via
  `factorio_exports.rs`).
- Paths like `provider::shared::...` are shared/`require` exports (real modules in
  the library crate).
- At transpile time, those calls become `remote.call` / `require` - the library’s
  Rust bodies are not copied into your mod’s Lua.
- `cargo check` / rust-analyzer use the real library package like any other dep.

## Other Factorio deps (optional)

Non-Rust Factorio deps (DLC, conflicts) still go in `Factorio.toml`:

```toml
[mod]
factorio_version = "2.0"
dependencies = ["? space-age"]
```

`factorio-rs add` appends entries like `provider >= 0.1.3` from Cargo metadata.
A `base >= ...` line is added automatically when missing.

## Lua-only mods (flib, etc.)

Publish or path-depend a small crate with empty `pub fn` stubs and
`[package.metadata.factorio]` - same shape the toolchain writes for factorio-rs
libraries.

## Troubleshooting

| Problem | Fix |
| --- | --- |
| `library exports missing` / no metadata | Run `factorio-rs build` in the library first |
| `provider::greet` not found | Ensure `pub mod control`, rebuild library (regenerates `factorio_exports.rs`), and `mod factorio_exports; pub use factorio_exports::*;` in `lib.rs` |
| Call doesn’t update after changing exports | Rebuild the library, then rebuild the dependent |
