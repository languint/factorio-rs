---
title: Persist with storage
description: Keep mod state across events and save/load with Factorio storage.
---

Rust `static` / `LazyLock` are **not** supported. For values that must survive
events (and save/load), use Factorio’s global `storage` table.

## Write and read

```rust
use factorio_rs::prelude::*;

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {
    storage.set("counter", 0_u32);

    // Missing key -> None (Lua nil). Present key -> Some(value).
    let n = storage.get::<u32>("counter").unwrap_or(0);
    storage.set("counter", n + 1);
    println!("boot count: {}", n + 1);
}
```

| API | Lowers to | Use when |
| --- | --- | --- |
| `storage.set(key, value)` | `storage[key] = value` | Persist a value |
| `storage.get::<T>(key)` | `storage[key]` as `Option<T>` | Typed optional read |
| `storage["key"]` | `storage[key]` as `LuaAny` | Opaque / dump with `{:?}` |

`storage` is a prelude global (`LuaStorage`). It is **mod-local** and persists
across events and save/load. See [API types](../guides/api-types/#globals).

:::tip
Prefer `get` + `if let Some` / `unwrap_or` over indexing when you care about
absence. That matches [Option and Result](../guides/option-and-result/).
:::

## When to use it

| Use `storage` | Don’t |
| --- | --- |
| Flags, caches, tables that must survive saves | Values that should reset every event |
| Sharing data between your own handlers | Emulating Rust `static mut` |
| Typed counters via `get` / `set` | Cross-mod data - use [export](share-api/) |

## Verify with a test

```rust
#[cfg(test)]
mod tests {
    use factorio_rs::prelude::*;

    #[test]
    #[ignore = "requires Factorio (run with factorio-rs test)"]
    fn storage_round_trips() {
        storage.set("marker", 42_u32);
        assert_eq!(storage.get::<u32>("marker"), Some(42));
        assert!(storage.get::<u32>("missing").is_none());
    }
}
```

Run with `factorio-rs test`. Multi-tick helpers: [Testing](../guides/testing/).
