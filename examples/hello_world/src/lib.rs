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
}
