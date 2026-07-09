#[factorio::control]
mod control {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    #[factorio::event(OnInit)]
    pub fn on_init() {
        println!("Hello from factorio-rs!");
    }
}
