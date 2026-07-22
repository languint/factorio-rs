---
title: Text
description: Text builder - a Factorio label with an optional element name.
---

A caption label (`GuiElementType::Label`).

```rust
use factorio_rs_gui::shared::text::Text;

Text::new("Count: 0")
Text::new(&format!("Count: {n}")).name("count_label")
```

## Builder API

| Method | Effect |
| --- | --- |
| `new(&str)` | Label with caption |
| `name(&str)` | Optional stable element name |

There is no reactive binding on the caption itself - recompute strings in
`app` with `format!` and rebuild when [`state!`](../../guides/state/) changes.
