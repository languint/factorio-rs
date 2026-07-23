#[factorio_rs::control]
mod control {
    use factorio_rs::{
        factorio_api::classes::{LuaEntity, LuaSurfaceCreateEntityParams},
        prelude::*,
    };

    pub fn try_place_entity(
        params: LuaSurfaceCreateEntityParams,
    ) -> Result<LuaEntity, &'static str> {
        if let Some(surface) = game.get_surface(0.into()) {
            surface
                .create_entity(params)
                .ok_or("failed to place entity, engine returned None!")
        } else {
            Err("failed to place entity, surface does not exist!")
        }
    }

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        println!("Hello factorio-rs!");
        match try_place_entity(LuaSurfaceCreateEntityParams {
            name: "inserter".into(),
            position: MapPosition { x: 0., y: 0. },
            ..Default::default()
        }) {
            Ok(_) => {
                println!("Successfully placed inserter!")
            }
            Err(e) => println!("[ERR] {e}!"),
        }
    }

    #[factorio_rs::event(filter = OnBuiltEntityFilter::name("inserter"))]
    pub fn on_built_entity(event: OnBuiltEntityEvent) {
        let (x, y) = (event.entity.position().x, event.entity.position().y);

        println!("inserter built at: ({x},{y})");
    }

    #[cfg(test)]
    mod tests {
        use factorio_rs::prelude::*;

        #[test]
        fn arithmetic_smoke() {
            assert_eq!(1 + 1, 2);
        }

        #[test]
        #[allow(clippy::assertions_on_constants)]
        fn truth_holds() {
            assert!(true);
        }

        #[test]
        #[ignore = "requires Factorio (run with factorio-rs test)"]
        fn tick_advances_across_waits() {
            let _ = factorio_rs::test::steps()
                .step(|ctx| {
                    ctx.set("t0", game.tick());
                })
                .wait(5)
                .step(|ctx| {
                    assert!(game.tick() >= ctx.fetch_u32("t0") + 5);
                });
        }

        #[test]
        #[ignore = "requires Factorio (run with factorio-rs test)"]
        fn parity_mod_vs_band_bench() {
            // Keep limits as literals so peephole does not mutate a shared `n_iters`
            // when lowering `0..n_iters` → `for i = 0, n_iters - 1`.
            let two = 2_u32;

            let mut sink = 0_u32;
            for n in 0..1_000 {
                if n % two == 1 {
                    sink += 1;
                }
                if n % 2 == 1 {
                    sink += 1;
                }
            }
            assert!(sink > 0);

            let p_opaque = helpers.create_profiler(None);
            sink = 0;
            for n in 0..20_000_000 {
                if n % two == 1 {
                    sink += 1;
                }
            }
            p_opaque.stop();
            assert_eq!(sink, 10_000_000);

            let p_literal = helpers.create_profiler(None);
            sink = 0;
            for n in 0..20_000_000 {
                if n % 2 == 1 {
                    sink += 1;
                }
            }
            p_literal.stop();
            assert_eq!(sink, 10_000_000);

            println!("parity_bench N=20000000 (next two log lines: opaque_%_two, literal_%_2)");
            log(p_opaque);
            log(p_literal);
        }
    }
}
