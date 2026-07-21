---
title: Enums
description: User-defined enums as tagged Lua tables, match patterns, and inherent impls.
---

User-defined enums lower to **tagged tables**. This is the language page; for a
worked phase machine see [State machines with enums](../recipes/state-machines/).

## Shapes

| Rust | Lua |
| --- | --- |
| `Color::Red` | `{ tag = "Red" }` |
| `Msg::Move(x, y)` | `{ tag = "Move", _1 = x, _2 = y }` |
| `Msg::Move { x, y }` | `{ tag = "Move", x = x, y = y }` |

```rust
enum Msg {
    Quit,
    Move(i64, i64),
    Say { text: String },
}

impl Msg {
    fn is_quit(&self) -> bool {
        matches!(self, Msg::Quit)
    }
}
```

Inherent methods share a table with unit variant constants, like structs.

## `match`

```rust
match msg {
    Msg::Quit => {}
    Msg::Move(x, y) => println!("{x},{y}"),
    Msg::Say { text } => println!("{text}"),
}
```

Supported patterns include unit / tuple / named variants, guards, and or-patterns.
`matches!(expr, pat)` / `matches!(expr, pat if guard)` is supported and desugars to
a value `match` that yields `true` / `false`.
Full pattern rules: [Supported Rust -> match](../guides/language/#match).

## Not the same as API unions

Factorio literal unions (`GuiDirection`, ...) stay **Lua strings**. Your enums
are tagged tables. Prefer Identification constructors such as
`ForceID::Name("enemy")` (they lower to the Factorio payload) -
see [API types](../guides/api-types/).
