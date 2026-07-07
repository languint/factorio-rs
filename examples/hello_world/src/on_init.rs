pub fn on_init() {
    let mut player = crate::player::MyPlayer::new();

    player.set_health(player.get_health() - 1);
}
