---
title: Benchmarking
description: Time generated Lua in Factorio with #[factorio_rs::bench] and factorio-rs bench.
---

Microbenchmark transpiled (or raw Lua) code inside Factorio using
`helpers.create_profiler`. Mark functions with `#[factorio_rs::bench]` and run
them with `factorio-rs bench`.

| Command | What it does |
| --- | --- |
| `factorio-rs bench` | Builds (default **release**), launches Factorio, times benches |
| `cargo test` | Does **not** run benches (they are not `#[test]`) |

## Authoring

Full suite: [`examples/benchmarking`](/examples/benchmarking/).

```rust
#[cfg(test)]
mod benches {
    use factorio_rs::prelude::*;

    #[factorio_rs::bench(iterations = 5)]
    fn collect_map_range() {
        let _xs: Vec<i64> = (0..50_000).map(|i| i * 2).collect();
    }

    /// Raw Lua escape hatch, must be `unsafe` (raw lua cannot be checked).
    #[factorio_rs::bench(iterations = 5)]
    unsafe fn append_hash_index() {
        lua! {
            local out = {}
            for i = 1, 100000 do
                out[#out + 1] = i
            end
        }
    }
}
```

| Attribute | Meaning |
| --- | --- |
| `#[factorio_rs::bench]` | Run once per measurement |
| `#[factorio_rs::bench(iterations = N)]` | Time the body `N` times; report min/mean/max-style sample list |

Wall clock is roughly **body time × iterations** (plus Factorio startup). Heavy
bodies need modest `N` or a higher `--timeout` (default 120s).

Benches may live in `#[cfg(test)]` modules or next to control-stage code.
Prefer release profiles when comparing IR opts (`--profile release` is the
default for `factorio-rs bench`).

### `lua! { … }`

Emits the enclosed text as raw Lua into the function body. The frontend does
not parse or typecheck it.

- Only allowed in an `unsafe fn` or an `unsafe { ... }` block
- Use for idiom comparisons the transpile cannot express 1:1
- Prefer lowered Rust when possible so opts and checks still apply

## Running

```bash
factorio-rs bench
factorio-rs bench array_indexing          # name filter (substring)
factorio-rs bench --profile release
factorio-rs bench --timeout 180
factorio-rs bench --gui
```

Example report (Criterion-style `[min mean max] ±stddev`; unit scales with the mean):

```text
running 2 benches
bench control::benches::append_hash_index   ... time: [28.1 ms 30.7 ms 33.5 ms] ±2.70 ms
bench control::benches::tiny_work           ... time: [20.0 µs 25.0 µs 30.0 µs] ±5.00 µs
```

## How it works

1. Discover `#[factorio_rs::bench]` functions.
2. Emit `dist/lua/factorio_rs_benches.lua` and append a harness to `control.lua`.
3. For each sample: `helpers.create_profiler()` -> run body -> `stop()` →
   `localised_print` with the profiler (Factorio expands `Duration: …ms`).
4. The CLI parses those lines (Lua cannot read raw profiler numbers).

## See also

- [benchmarking example](/examples/benchmarking/) - collect vs push, parity, `lua!`
- [Testing](/guides/testing/) - correctness simulations (`#[test]`)
- [CLI reference](/reference/cli/) - full `factorio-rs bench` flags
- [Profiles](/guides/profiles/) - `optimize_ir` / release
