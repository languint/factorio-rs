---
title: Testing
description: Write \#[test] simulations and run them with factorio-rs test.
---

factorio-rs lets you write familiar Rust unit tests that exercise Factorio game
APIs. Those tests are **transpiled to Lua** and executed inside Factorio by
`factorio-rs test` (headless by default).

| Command | What it does |
| --- | --- |
| `cargo test` / `cargo check --tests` | Typechecks `#[cfg(test)]` code against API stubs |
| `factorio-rs test` | Builds the mod, launches Factorio, runs simulations in-game |

Plain `cargo test` cannot run real simulations - the stubs are compile-only.
Use `factorio-rs test` whenever a test touches `game`, surfaces, entities, or
other runtime APIs.

## Authoring

Put tests in a `#[cfg(test)]` module (inline or `mod tests;`) and mark functions
with `#[test]`. Tests can live next to control-stage code in `control_mod!`, in
a `#[factorio_rs::control]` module, or in a sibling file module gated on
`#[cfg(test)]`.

Normal builds skip `#[cfg(test)]` modules. Only the test runner lowers them.

### Smoke tests

Pure logic does not need the Factorio world. Useful while wiring the runner:

```rust
factorio_rs::control_mod! {
    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("hi");
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn arithmetic_smoke() {
            assert_eq!(1 + 1, 2);
        }

        #[test]
        fn truth_holds() {
            assert!(true, "should never fail");
        }
    }
}
```

See [`examples/hello_world`](../examples/hello-world/).

### Reading the game world

Inside a test you can call the same Factorio APIs as control-stage code. The
suite runs on a blank `factorio-rs-test` scenario after map init (and again on
the first tick as a fallback), so surface `1` (Nauvis) is available:

```rust
#[cfg(test)]
mod tests {
    use factorio_rs::prelude::*;

    #[test]
    fn nauvis_exists() {
        let surface = game.get_surface(1.into());
        assert!(surface.is_some());
    }

    #[test]
    fn player_force_exists() {
        let force = game.forces.get("player".into());
        assert!(force.is_some());
    }
}
```

### Placing entities and calling mod logic

For behavior that depends on entities, create them on the surface and call your
mod functions directly - the same Rust functions players hit via events:

```rust
#[cfg(test)]
mod tests {
    use factorio_rs::{factorio_api::classes::LuaSurfaceCreateEntityParams, prelude::*};

    use crate::control;

    fn nauvis() -> factorio_rs::factorio_api::classes::LuaSurface {
        if let Some(surface) = game.get_surface(1.into()) {
            return surface;
        }
        panic!("expected nauvis surface at index 1");
    }

    fn place_chest(x: f64, y: f64) -> factorio_rs::factorio_api::classes::LuaEntity {
        let surface = nauvis();
        if let Some(entity) = surface.create_entity(LuaSurfaceCreateEntityParams {
            name: "iron-chest".into(),
            position: MapPosition { x, y },
            force: Some("player".into()),
            raise_built: Some(false),
            create_build_effect_smoke: Some(false),
            ..Default::default()
        }) {
            return entity;
        }
        panic!("failed to place iron-chest");
    }

    #[test]
    fn isolated_building_survives() {
        let building = place_chest(10.0, 10.0);
        control::check_build_rules(building, 0);
        assert!(building.valid());
    }

    #[test]
    fn two_neighbors_destroy_the_building() {
        let _a = place_chest(30.0, 30.0);
        let _b = place_chest(32.0, 30.0);
        let building = place_chest(31.0, 30.0);
        control::check_build_rules(building, 0);
        assert!(!building.valid());
    }
}
```

Tips for entity tests:

- Prefer distinct coordinates per test so leftover entities from earlier cases
  do not interfere (the suite shares one map).
- Set `raise_built: Some(false)` when you want to call your logic yourself
  instead of going through `on_built_entity`.
- Assert with `entity.valid()` after rules that may `die()` the entity.
- Prefer `if let` / `panic!` over `.unwrap()` / `.expect()` (those lint deny by
  default - see [Lints](lints/)).

A fuller suite lives in
[`examples/mandatory_spaghetti`](../examples/mandatory-spaghetti/)
(`src/control.rs`).

### Assertions

Supported macros lower to Lua `error(...)` on failure:

| Macro | Notes |
| --- | --- |
| `assert!(cond)` / `assert!(cond, "...")` | Optional message |
| `assert_eq!(left, right)` / `assert_ne!(...)` | Same |
| `panic!("...")` | Always fails the test |

Anything that panics or returns `Err` from `?` (where supported) also fails the
test via Lua's `pcall` around each case.

## Running

```bash
# Requires a Factorio binary
export FACTORIO_PATH=/path/to/factorio   # or rely on PATH / Steam install paths

factorio-rs test
factorio-rs test smoke                   # name filter (substring)
factorio-rs test --timeout 180
factorio-rs test --gui                    # windowed; stays open after the suite
factorio-rs test --skip-typecheck         # skip cargo check --tests (not recommended)
```

`factorio-rs test` will:

1. Run `cargo check --tests` (unless `--skip-typecheck`)
2. Build the mod into `.factorio-rs/test-run/mods/`
3. Launch Factorio with a blank `factorio-rs-test` scenario
   - default: headless dedicated server (`--start-server-load-scenario`)
   - `--gui`: singleplayer window (`--load-scenario`)
4. Print a colored report and exit non-zero on failures

Example output:

```text
running 6 tests
[OK]   tests::rails_are_on_adjacency_blacklist
[OK]   tests::poles_and_pipes_are_on_pattern_blacklist
[OK]   tests::isolated_building_survives_adjacency_check
[OK]   tests::building_survives_with_one_neighbor
[OK]   tests::building_explodes_with_two_neighbors
[OK]   tests::building_explodes_next_to_three_adjacent_buildings

test result: ok. 6 passed; 0 failed; 0 ignored
```

Colors are enabled when stdout is a TTY. Set `NO_COLOR=1` for plain text.

### Watching with `--gui`

```bash
factorio-rs test --gui
```

Opens Factorio so you can see the scenario load. The suite still finishes on
init / the first tick (usually too fast to step through), then Factorio **stays
open** so you can inspect leftover entities. Close the window to finish the
CLI. Increase `--timeout` if graphics startup is slow on your machine.

## How it works

1. **Discover** - the frontend finds `#[test]` fns under `#[cfg(test)]`.
2. **Emit** - those tests become `dist/lua/factorio_rs_tests.lua`, and a small
   harness is appended to `control.lua`.
3. **Isolate** - an ephemeral write-data tree under `.factorio-rs/test-run/`
   holds mods, `config.ini`, the scenario, and `script-output/`. Your normal
   `~/.factorio` data is untouched. Ignore this directory in git (new projects
   from `factorio-rs init` already do).
4. **Run** - each test is wrapped in `pcall`. Pass/fail lines go to stdout via
   `localised_print`, with a results file as backup.
5. **Report** - the CLI parses the protocol and prints `[OK]` / `[FAIL]`.

## Requirements

- A real Factorio **binary** (`FACTORIO_PATH`, common Steam paths, or `PATH`).
  Steam `steam://` protocol launches cannot pass scenario load flags.
- On Linux Steam installs, `steam-run` is used automatically when available.
  Set `FACTORIO_RS_NO_STEAM_RUN=1` to force a direct launch.

## Limitations

- Tests share one map and run sequentially in discovery order; clean up or use
  unique positions when placing entities.
- There is no per-test Factorio restart and no built-in tick stepping /
  multi-tick scenarios yet - the harness runs the whole suite at init.
- Host-only crates and `std` APIs that are not lowered will not work inside
  tests (same rules as control-stage code). See
  [Language support](language/).

## See also

- [CLI reference](../reference/cli/) - full `factorio-rs test` flags
- [hello_world](../examples/hello-world/) - minimal smoke tests
- [mandatory_spaghetti](../examples/mandatory-spaghetti/) - adjacency
  simulations with `create_entity`
- [API types](api-types/) - typed params for `create_entity` / filters
