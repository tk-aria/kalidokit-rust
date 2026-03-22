//! dynplug - Cross-platform dynamic plugin loading

pub mod error;
pub use error::PluginError;

pub mod platform;
pub use platform::lib_filename;

pub mod api;
pub use api::{PluginEntryFn, PluginVTable, INTERFACE_VERSION, PLUGIN_ENTRY_SYMBOL};

pub mod export;

pub mod loader;
pub use loader::{BoundFn, LoadedLibrary};

pub mod vtable;
pub use vtable::VTableValidate;
