---
title: provider / consumer
description: Two-mod example - export from provider, depend via Cargo.
---

Minimal walkthrough of sharing an API between mods. Full guide:
[Sharing code between mods](../guides/dependencies/).

| Path | Role |
| --- | --- |
| `examples/provider` | Library mod (`#[factorio_rs::export]` + Cargo metadata) |
| `examples/consumer` | `provider = { path = "../provider" }` |

```bash
factorio-rs build --manifest-path examples/provider
factorio-rs add examples/provider --manifest-path examples/consumer   # or cargo add --path
factorio-rs build --manifest-path examples/consumer
```

In the consumer:

```rust
provider::greet("consumer");              // -> remote.call
provider::shared::api::greet("consumer"); // -> require
```
