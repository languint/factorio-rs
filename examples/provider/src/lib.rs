pub mod shared;

#[factorio_rs::control]
pub mod control {
    #[factorio_rs::export]
    pub fn greet(name: &str) {
        println!("hello from provider remote, {name}!");
    }

    #[factorio_rs::event(OnSingleplayerInit)]
    pub fn on_singleplayer_init() {
        greet("provider-self");
        crate::shared::api::greet("provider");
        println!("provider API version: {}", crate::shared::api::VERSION);
    }
}

mod factorio_exports;
pub use factorio_exports::*;
