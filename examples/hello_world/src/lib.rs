#[factorio_rs::control]
mod control {
    fn add(a: i64, b: i64) -> i64 {
        a + b
    }

    #[factorio_rs::event(OnInit)]
    pub fn on_init() {
        println!("Hello from factorio-rs!");
    }
}
