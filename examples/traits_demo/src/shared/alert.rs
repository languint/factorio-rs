//! Shared alert trait for cross-module trait demos.

pub trait Alert {
    fn title(&self) -> &'static str;
    fn priority(&self) -> i64;

    fn announce(&self) {
        println!("[alert p{}] {}", self.priority(), self.title());
    }
}
