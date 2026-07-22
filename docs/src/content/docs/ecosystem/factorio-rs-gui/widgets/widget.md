---
title: Widget
description: The Widget enum - mountable Frame, Text, and Button nodes.
---

`Widget` is the concrete tree type the runtime mounts:

```rust
pub enum Widget {
    Frame(Frame),
    Text(Text),
    Button(Button),
}
```

## Conversions

`Frame`, `Text`, and `Button` implement `Into<Widget>` / `From`, so
`Frame::child(...)` accepts builders directly:

```rust
.child(Text::new("hi"))
.child(Button::new("Go"))
.child(Frame::new().caption("Nested"))
```

Return `impl Into<Widget>` from your `app` function (usually a root `Frame`).

## Mount

```rust
widget.mount(parent); // creates Factorio elements under `parent`
```

[`runtime::mount`](../../guides/lifecycle/) wraps this for screen roots and
applies the mount `root_name` when the root frame has no explicit `.name(...)`.
