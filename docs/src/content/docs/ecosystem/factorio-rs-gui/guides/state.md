---
title: State
description: Reactive state! hooks that survive GUI rebuilds.
---

`state!(init)` allocates a hook slot that survives destroy/rebuild cycles. Slots
are ordered by call site and namespaced by the current GUI `root_name`.

```rust
fn app() -> impl Into<Widget> {
    let count = factorio_rs_gui::state!(0);
    let label = format!("Count: {}", count.get());
    // ...
}
```

## API

| Call | Effect |
| --- | --- |
| `state!(init)` | Create / resume a hook with initial `i32` |
| `state.get()` | Read current value |
| `state.set(value)` | Write value and rebuild this root |

`state!` expands to `State::use_state`. Prefer the macro at the top of `app`.

## Rules

1. Call hooks in a **stable order** every rebuild (same as React hooks).
2. Do not put hooks behind conditionals that change between rebuilds.
3. Captions are not live bindings, use `format!` and let `set` rebuild the tree.
4. Values live in **your** mod's `storage` under keys like `frg:{root}:hook_N`.

## Types today

Hooks are limited to `i32` (for now). Store richer data in your own `storage` keys and keep a
counter (or flag) in `state!` to trigger rebuilds.

See also: [Lifecycle](../lifecycle/), [Reactive GUI](../reactive/).
