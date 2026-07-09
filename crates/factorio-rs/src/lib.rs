//! Rust SDK for Factorio's modding API.

pub mod event;

pub use factorio_macros::{control, data, event, shared};
pub use factorio_macros::{control_mod, data_mod, shared_mod};

pub mod prelude {
    pub use crate::event::OnInit;
    pub use crate::{control, control_mod, data, data_mod, event, shared, shared_mod};
}
