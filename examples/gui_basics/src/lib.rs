//! Open a framed GUI when a player is created.
//!
//! Walkthrough: docs recipe “GUI basics”.

#[factorio_rs::control]
mod control {
    use factorio_rs::{
        factorio_api::{IndexOrName, classes::LuaGuiElementAddParams},
        prelude::*,
    };

    #[factorio_rs::event(OnPlayerCreated)]
    pub fn on_player_created(event: OnPlayerCreatedEvent) {
        if let Some(player) = game.get_player(IndexOrName::Index(event.player_index)) {
            let frame = player.gui().screen().add(LuaGuiElementAddParams {
                r#type: GuiElementType::Frame,
                name: Some("gui_basics_root".into()),
                caption: Some("GUI basics".into()),
                ..Default::default()
            });

            let label = frame.add(LuaGuiElementAddParams {
                r#type: GuiElementType::Label,
                caption: Some("Hello from factorio-rs".into()),
                ..Default::default()
            });

            // Use the style object for size / spacing (typed LuaStyle).
            frame.style().set_width(280);
            label.style().set_padding(8);

            // Optional: swap the whole style prototype by name.
            // frame.set_style("inside_shallow_frame");
        }
    }
}
