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
    }
}
