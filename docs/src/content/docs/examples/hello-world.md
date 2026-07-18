---
title: hello_world
description: Minimal control-stage example with a filtered built-entity event.
---

Path: `examples/hello_world`.

A single `src/lib.rs` marks an inline control module and registers two events:

```rust
#[factorio_rs::control]
mod control {
    use factorio_rs::factorio_api::events::{OnBuiltEntityEvent, OnBuiltEntityFilter};

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("Hello factorio-rs!");
    }

    #[factorio_rs::event(filter = OnBuiltEntityFilter::name("inserter"))]
    pub fn on_built_entity(event: OnBuiltEntityEvent) {
        let (x, y) = (event.entity.position().x, event.entity.position().y);
        println!("inserter built at: ({x},{y})");
    }
}
```

## Try it

```bash
cd examples/hello_world
factorio-rs build
factorio-rs install --open   # optional
factorio-rs test             # requires Factorio binary; see Testing guide
```

Mod id: Cargo package name `hello_world`.
