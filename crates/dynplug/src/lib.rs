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

pub mod define;

pub mod backend;
pub use backend::PluginBackend;

pub mod native;
pub use native::NativeBackend;

#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "wasm")]
pub use wasm::WasmBackend;

pub mod manager;
pub use manager::PluginManager;

// Re-export paste for use in define_plugin! macro.
#[doc(hidden)]
pub use paste;

// Re-export extism types when wasm feature is enabled.
#[cfg(feature = "wasm")]
pub use extism;
