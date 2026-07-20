---
title: Serde / JSON
description: Optional serde_json support lowered to helpers.table_to_json and string.pack.
---

factorio-rs can lower `serde_json` encode/decode calls to Factorio builtins.
There is **no serde runtime** in Factorio; this is transpile-time rewriting,
like [Tracing](tracing/).

## Enable the feature

In your mod’s `Cargo.toml`:

```toml
[dependencies]
factorio-rs = { version = "0.2.0", features = ["serde"] }
```

That pulls in `serde` and `serde_json` so derives and calls type-check under
`cargo check`. Re-exports:

```rust
use factorio_rs::prelude::*; // Serialize, Deserialize
use factorio_rs::serde_json;

#[derive(Serialize, Deserialize)]
struct SaveData {
    tick: u32,
}
```

The **CLI** lowers these calls by default (`factorio-rs-cli` feature `serde`).

## What lowers

| Rust | Lua |
| --- | --- |
| `serde_json::to_string(v)` / `to_string_pretty(v)` | `helpers.table_to_json(v)` |
| `serde_json::from_str(s)` | `helpers.json_to_table(s)` |
| `serde_json::to_value(v)` / `from_value(v)` | `v` |
| `serde_json::to_vec(v)` | `string.pack("s", helpers.table_to_json(v))` |
| `serde_json::from_slice(b)` | `helpers.json_to_table(string.unpack("s", b))` |

Binary uses Lua’s size-prefixed `"s"` format on Factorio’s `string.pack` /
`unpack`. Prefer `.unwrap()` / `.expect(...)` on `Result` so types match real
serde usage under `cargo check` - the transpile strips those calls (and may
lint). For fallible gameplay code prefer `?` / `if let Ok` - see
[Option and Result](option-and-result/).

`#[derive(Serialize, Deserialize)]` is **typecheck-only** - the transpiler
never runs serde; struct values are already Lua tables. Use this when you need
JSON for debugging or a remote payload, not as a general Rust serde runtime.

Not supported: `serde_json::json!`, streaming serializers, and other APIs.

## Feature matrix

| Crate | Feature | Role |
| --- | --- | --- |
| `factorio-rs` | `serde` | Optional `serde` + `serde_json` deps + prelude re-exports |
| `factorio-rs-cli` | `serde` (default) | Enable frontend lowering when building mods |
| `factorio-frontend` | `serde` | Implementation of the call lowering |
