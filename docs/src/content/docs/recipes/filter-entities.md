---
title: Filter entity lists
description: Use Vec, ranges, and map/filter/collect to process entities in control code.
---

factorio-rs lowers a small iterator subset to Lua. Use it when you already have a
`Vec` (or a numeric range) and want a new table without writing nested `for`
loops by hand.

## Ordered loops

Bindings typed as `Vec<_>` (or `.iter()`) use `ipairs` - order is preserved:

```rust
type Entities = Vec<LuaEntity>;

fn count_inserters(entities: Entities) -> i64 {
    let mut n = 0;
    for entity in entities {
        if entity.name() == "inserter" {
            n += 1;
        }
    }
    n
}
```

Ranges become Lua numeric `for`:

```rust
for i in 0..n {
    // i is 0 .. n-1
}
for i in 0..=n {
    // inclusive end
}
```

## map / filter / collect

Supported chain: range or `Vec` + `.map` / `.filter` / `.take` + `.collect`
(including `.collect::<Vec<_>>()`). Other adapters (`enumerate`, `zip`, ...)
are rejected.

```rust
fn double_positive(values: Vec<i64>) -> Vec<i64> {
    values
        .iter()
        .map(|n| n * 2)
        .filter(|n| *n > 0)
        .collect::<Vec<_>>()
}

fn indices(n: i64) -> Vec<i64> {
    (0..n).map(|i| i + 1).collect::<Vec<_>>()
}
```

That lowers to an immediately invoked Lua function that builds a table.

## With a type alias

```rust
type Scores = Vec<i64>;

fn top_half(scores: Scores) -> Scores {
    scores.iter().filter(|s| *s >= 50).collect::<Vec<_>>()
}
```

Aliases are transparent - see [Type aliases](/language/type-aliases/).

## Limits

| Supported | Not yet |
| --- | --- |
| `for` on `Vec` / `.iter()` / ranges | `enumerate`, `zip`, `flat_map` |
| `.map` / `.filter` / `.take` / `.collect` on that subset | Standing `.iter()` without collect |
| `type Name = Vec<_>` for ipairs detection | Arbitrary `Iterator` trait objects |

Full table: [Collections and iterators](/language/collections/).
