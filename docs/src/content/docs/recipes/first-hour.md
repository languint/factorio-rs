---
title: First hour
description: Init, build, install, see a print, then run your first in-game test.
---

A guide to init, build, install, see a print, then run your first in-game test. This guide assumes you already have the required `factorio-rs` binary installed. If you don't, see [Installation](../../installation/).

## 1. Scaffold

```bash
mkdir my-mod && cd my-mod
factorio-rs init --name my-mod
```

You get `Cargo.toml`, `Factorio.toml`, and a sample control-stage handler in
`src/lib.rs`. Details: [Getting started](../guides/getting-started/).

## 2. Build

```bash
factorio-rs check   # cargo check + transpile lints
factorio-rs build   # emit dist/ (loadable Factorio mod)
```

`dist/` should contain `info.json`, `control.lua`, and `lua/control.lua`.

## 3. See it in the game

```bash
factorio-rs install --open
```

Start a new map (or load one). You should see `Initialized` printed when the
control stage runs (`OnSingleplayerInit`).

If Factorio is not installed yet, keep using `factorio-rs build` locally; install
and open once you have a binary (see [Installation](../installation/)).

## 4. Change something small

Edit the handler message, rebuild, reinstall:

```rust
factorio_rs::control_mod! {
    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("Hello from my-mod");
    }
}
```

```bash
factorio-rs build && factorio-rs install --open
```

## 5. Add a smoke test

Attach a `#[cfg(test)]` module next to your control code:

```rust
factorio_rs::control_mod! {
    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("Hello from my-mod");
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn arithmetic_smoke() {
            assert_eq!(1 + 1, 2);
        }
    }
}
```

Run it inside Factorio (not with plain `cargo test`):

```bash
factorio-rs test
```

Plain `cargo test` typechecks stubs only. `factorio-rs test` transpiles the suite
and launches Factorio. Full details: [Testing](../guides/testing/).

## 6. Pick a next recipe

| Goal | Recipe |
| --- | --- |
| Iterate with Bacon (in-game + tests) | [Hot reload with Bacon](hot-reload-bacon/) |
| Remember a counter across events / saves | [Persist with storage](persist-storage/) |
| Toggle behavior from mod settings | [Settings that change gameplay](settings-gameplay/) |
| Map / filter a list of entities | [Filter entity lists](filter-entities/) |
| Model mod phases with enums | [State machines with enums](state-machines/) |
| Ship sprites + data-stage item | [Package graphics](package-graphics/) |
| Open a styled GUI frame | [GUI basics](gui-basics/) |
| Call another mod’s API | [Share an API between mods](share-api/) |
