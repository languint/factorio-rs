---
title: benchmarking
description: In-game microbenchmarks with #[factorio_rs::bench], collect vs push, and lua!.
---

Path: `examples/benchmarking`.

A small control mod whose `#[cfg(test)]` module is a suite of
[`#[factorio_rs::bench]`](/guides/benchmarking/) functions, lowered Rust (`map`/`filter`/`collect`), parity `%` experiments, and raw `lua!` append idioms.

## Try it

```bash
cd examples/benchmarking
factorio-rs bench                         # all benches (default --profile release)
factorio-rs bench pre_allocate            # name filter (substring)
```

Example report:

```text
running 2 benches
bench benches::no_pre_allocate ... time: [9.490 ms 9.967 ms 16.872 ms] ±0.383 ms
bench benches::pre_allocate    ... time: [9.489 ms 9.951 ms 16.487 ms] ±0.474 ms
```

Requires a Factorio binary (`FACTORIO_PATH`). Details:
[Benchmarking](/guides/benchmarking/).

## What’s inside

| Bench | What it times |
| --- | --- |
| `no_pre_allocate` | fill a fresh Lua table `1..100000` |
| `pre_allocate` | same loop after a sparse `x[1024] = nil` pre-size |

See also: [Benchmarking](/guides/benchmarking/), [Collections](/language/collections/),
[CLI](/reference/cli/).
