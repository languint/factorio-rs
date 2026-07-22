#[factorio_rs::control]
mod control {
    use factorio_rs::{
        factorio_api::{IndexOrName, lua_fn, lua_fn0},
        prelude::*,
    };

    use factorio_rs_gui::shared::button::Button;
    use factorio_rs_gui::shared::frame::Frame;
    use factorio_rs_gui::shared::text::Text;
    use factorio_rs_gui::shared::widget::Widget;

    const ROOT: &str = "gui_counter";

    fn app() -> impl Into<Widget> {
        let count = factorio_rs_gui::state!(0);

        let label = format!("Count: {}", count.get());

        let increment = lua_fn(move |event: OnGuiClickEvent| {
            let _ = event;
            count.set(count.get() + 1);
        });

        Frame::new()
            .caption("Hello factorio-rs!")
            .centered()
            .align_horizontal(LuaStyleHorizontalAlign::Left)
            .align_vertical(LuaStyleVerticalAlign::Top)
            .direction(GuiDirection::Vertical)
            .child(Text::new(&label))
            .child(Button::new("Increment counter").on_click(increment))
    }

    #[factorio_rs::event(OnPlayerCreated)]
    pub fn on_player_created(event: OnPlayerCreatedEvent) {
        if let Some(player) = game.get_player(IndexOrName::Index(event.player_index)) {
            factorio_rs_gui::shared::runtime::mount(player.gui().screen(), ROOT, lua_fn0(app));
        }
    }

    #[factorio_rs::event(OnTick)]
    pub fn on_tick(_event: OnTickEvent) {
        factorio_rs_gui::shared::runtime::install(ROOT, lua_fn0(app));
    }
}
