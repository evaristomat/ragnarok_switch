//! Switch-specific runtime initialization.
pub mod horizon {
    pub use bevy_horizon::{init_switch_logging, flush_switch_logs};
    pub use bevy_horizon::register_release_hook;
}
