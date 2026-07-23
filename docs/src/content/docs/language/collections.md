---
title: Collections and iterators
description: Vec, ranges, ipairs/pairs loops, and the supported map/filter/take/collect subset.
---

Language reference for collections. Recipe-style walkthrough:
[Filter entity lists](/recipes/filter-entities/).

## `Vec` and loops

```rust
let mut list: Vec<i64> = Vec::new();
list.push(1);
for item in list {
    // ...
}
```

| Rust | Lua behaviour |
| --- | --- |
| `Vec::new()` | `{}` |
| `push` / `len` / `is_empty` | `table.insert` / `#` / `# == 0` |
| `vec![...]` | Prefer `Vec::new()` + `push`. Builds expand `vec!` through `alloc` helpers that are not lowered yet. |
| `for x in v` | `ipairs(v)` when `v` is typed `Vec<_>`; else unordered `pairs(v)` |
| `for x in v.iter()` / `v.into_iter()` | ordered `ipairs(v)` |
| `for i in start..end` | numeric `for i = start, end - 1 do` |
| `for i in start..=end` | numeric `for i = start, end do` |

## map / filter / take / collect

Supported: range or `Vec` iteration, then `.map(|x| ...)`, `.filter(|x| ...)`,
and/or `.take(n)`, ending in `.collect()` / `.collect::<Vec<_>>()`.

```rust
let mapped = (0..n).map(|i| i + 1).collect::<Vec<_>>();
let odds = (0..=n).iter().filter(|i| *i % 2 == 1).collect::<Vec<_>>();
let first_odds = (0..=n).filter(|i| *i % 2 == 1).take(5).collect::<Vec<_>>();
let both = values
    .iter()
    .map(|i| i + 1)
    .filter(|i| *i > 1)
    .collect::<Vec<_>>();
```

`.iter()` / `.into_iter()` on a range is optional (same as `(0..=n).filter(...).collect()`).
`.take(n)` respects adapter order (e.g. `.filter(...).take(5)` keeps five
passing elements). Lowers to an immediately invoked Lua function that builds a
table.

**Not supported:** `enumerate`, `flat_map`, `zip`, standing `.iter()` without
collect, and arbitrary `Iterator` trait objects.

## Related

- [Type aliases](/language/type-aliases/) - `type Entities = Vec<_>` still detects `ipairs`
- [Supported Rust](/guides/language/) - full inventory
