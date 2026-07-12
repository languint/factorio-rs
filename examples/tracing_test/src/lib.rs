factorio_rs::control_mod! {
    use factorio_rs::prelude::*;
    use factorio_rs::tracing;

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        tracing::info!("Hello factorio-rs!");
        tracing::error!("Oopsies!");
    }

    #[factorio_rs::event(OnBuiltEntity)]
    pub fn on_built_entity(event: OnBuiltEntityEvent) {
        tracing::info!("entity built {:?}", event.entity);
    }
}
