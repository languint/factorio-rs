---
title: Recipes
description: Short, job-oriented walkthroughs for common factorio-rs tasks.
---

Recipes are **use-case walkthroughs**. For inventories and flag tables, use
Language / Concepts / Reference instead.

| Recipe | Job |
| --- | --- |
| [First hour](first-hour/) | Init -> build -> install -> first `factorio-rs test` |
| [Persist with storage](persist-storage/) | Mod-local state across events and saves |
| [Settings that change gameplay](settings-gameplay/) | `mod_settings!` + control read + test |
| [Filter entity lists](filter-entities/) | `Vec`, ranges, `.map` / `.filter` / `.collect` |
| [State machines with enums](state-machines/) | Tagged enums + `match` for phases |
| [Package graphics](package-graphics/) | Assets + `item!` icons -> `__mod__/...` + `locale!` |
| [Share an API between mods](share-api/) | `#[export]` + `factorio-rs add` |

For data-stage `Item` / `Recipe` stubs and the `item!` / `recipe!` macros (not
just packaging art), see [Prototypes](../guides/prototypes/).

New to the toolchain? Start with [Getting started](../guides/getting-started/),
then [First hour](first-hour/).
