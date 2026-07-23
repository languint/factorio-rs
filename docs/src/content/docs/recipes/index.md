---
title: Recipes
description: Short, job-oriented walkthroughs for common factorio-rs tasks.
---

Recipes are **use-case walkthroughs**. For inventories and flag tables, use
Language / Concepts / Reference instead.

| Recipe | Job |
| --- | --- |
| [First hour](/recipes/first-hour/) | Init -> build -> install -> first `factorio-rs test` |
| [Hot reload with Bacon](/recipes/hot-reload-bacon/) | Bacon jobs for in-game control reload + test `--rerun` |
| [Persist with storage](/recipes/persist-storage/) | Mod-local state across events and saves |
| [Settings that change gameplay](/recipes/settings-gameplay/) | `mod_settings!` + control read + test |
| [Filter entity lists](/recipes/filter-entities/) | `Vec`, ranges, `.map` / `.filter` / `.take` / `.collect` |
| [State machines with enums](/recipes/state-machines/) | Tagged enums + `match` for phases |
| [Package graphics](/recipes/package-graphics/) | Assets + `item!` icons -> `__mod__/...` + `locale!` |
| [GUI basics](/recipes/gui-basics/) | Event -> frame -> caption -> `style().set_width` |
| [Reactive GUI](/recipes/reactive-gui/) | Points to [factorio-rs-gui guides](/ecosystem/factorio-rs-gui/guides/reactive/) |
| [Share an API between mods](/recipes/share-api/) | `#[export]` + `factorio-rs add` |

For data-stage `Item` / `Recipe` / `Technology` stubs and the `item!` /
`recipe!` / `technology!` macros (not just packaging art), see
[Prototypes](/guides/prototypes/).

New to the toolchain? Start with [Getting started](/guides/getting-started/),
then [First hour](/recipes/first-hour/).
