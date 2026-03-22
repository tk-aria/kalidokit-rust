//! Avatar SDK — shared state and action definitions.
//!
//! Pure data structures with no external dependencies.
//! Used as the bridge between the app's internal state and
//! scripting runtimes (Lua, etc.).

pub mod action;
pub mod state;

pub use action::{ActionQueue, AvatarAction};
pub use state::{
    AvatarState, DisplayState, InfoState, LightState, LightingState, TrackingState,
};
