---
title: traits_demo
description: Control-stage example of cross-module traits with defaults, overrides, and dyn Alert dispatch.
---

Path: `examples/traits_demo`.

Shows how powerful same-crate traits are in factorio-rs: `Alert` lives in
`shared.alert`, control `use`s it, a default `announce`, a type-specific
override, static method calls, and one helper that takes `&dyn Alert`
(Lua fat pointer + vtable).

```rust
// shared/alert.rs
pub trait Alert {
    fn title(&self) -> &'static str;
    fn priority(&self) -> i64;

    fn announce(&self) {
        println!("[alert p{}] {}", self.priority(), self.title());
    }
}

// control
use crate::shared::alert::Alert;

struct PowerDrop {
    machine: &'static str,
    percent: i64,
}

impl Alert for PowerDrop {
    fn title(&self) -> &'static str {
        self.machine
    }

    fn priority(&self) -> i64 {
        100 - self.percent
    }
}

// BeltJam overrides announce; ScienceStall uses the default.

fn shout(alert: &dyn Alert) {
    alert.announce();
}

fn priority_of(alert: &dyn Alert) -> i64 {
    alert.priority()
}

let total = priority_of(&power)
    + priority_of(&belt)
    + priority_of(&science);
```

On `OnSingleplayerInit` the example:

1. Calls `.announce()` on concrete values (**static** dispatch).
2. Sums priorities through `priority_of(&...)` (**dyn**).
3. Routes three different alert kinds through the same `shout` helper.

`#[test]`s cover static priorities and the dyn sum (no Factorio binary required
for those).

## Try it

```bash
cd examples/traits_demo
factorio-rs build
factorio-rs install --open   # optional - watch [alert ...] lines on init
factorio-rs test             # unit tests; Factorio only needed for ignored games
```

## Limits (today)

Associated types are supported without bounds/defaults; dyn coerce rejects
traits that declare them. No generics or supertraits - see
[Supported Rust](../guides/language/#traits).
