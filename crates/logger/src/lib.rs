//! Pipeline Logger — structured logging for kalidokit-rust processing pipeline.
//!
//! Inspired by ZLogger's Logging Providers design:
//! - Pluggable `LogProvider` trait for multiple output targets
//! - Structured key-value fields per log entry
//! - Pipeline stage classification for filtering and analysis

pub mod provider;

use std::fmt;
use std::sync::OnceLock;

pub use provider::{ConsoleProvider, LogProvider};

/// Processing pipeline stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Camera,
    Tracker,
    Solver,
    Bone,
    Gpu,
    Render,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Camera => write!(f, "Camera"),
            Self::Tracker => write!(f, "Tracker"),
            Self::Solver => write!(f, "Solver"),
            Self::Bone => write!(f, "Bone"),
            Self::Gpu => write!(f, "GPU"),
            Self::Render => write!(f, "Render"),
        }
    }
}

/// A structured pipeline log entry with stage, message, and key-value fields.
pub struct PipelineLog {
    pub level: log::Level,
    pub stage: Stage,
    pub message: String,
    pub fields: Vec<(String, String)>,
}

/// Builder for constructing pipeline log entries.
pub struct LogBuilder {
    level: log::Level,
    stage: Stage,
    message: String,
    fields: Vec<(String, String)>,
}

impl LogBuilder {
    pub fn new(level: log::Level, stage: Stage, message: impl Into<String>) -> Self {
        Self {
            level,
            stage,
            message: message.into(),
            fields: Vec::new(),
        }
    }

    /// Add a key-value field to the log entry.
    pub fn field(mut self, key: impl Into<String>, value: impl fmt::Display) -> Self {
        self.fields.push((key.into(), value.to_string()));
        self
    }

    /// Emit the log entry to all registered providers.
    pub fn emit(self) {
        let entry = PipelineLog {
            level: self.level,
            stage: self.stage,
            message: self.message,
            fields: self.fields,
        };
        if let Some(registry) = REGISTRY.get() {
            for provider in &registry.providers {
                provider.write(&entry);
            }
        }
    }
}

/// Global provider registry.
struct Registry {
    providers: Vec<Box<dyn LogProvider>>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Initialize the pipeline logger with the given providers.
///
/// Must be called once at startup. Subsequent calls are ignored.
pub fn init(providers: Vec<Box<dyn LogProvider>>) {
    let _ = REGISTRY.set(Registry { providers });
}

/// Initialize with a default console provider at the given log level.
pub fn init_console(level: log::Level) {
    init(vec![Box::new(ConsoleProvider::new(level))]);
}

// --- Convenience functions for each stage ---

pub fn camera(level: log::Level, message: impl Into<String>) -> LogBuilder {
    LogBuilder::new(level, Stage::Camera, message)
}

pub fn tracker(level: log::Level, message: impl Into<String>) -> LogBuilder {
    LogBuilder::new(level, Stage::Tracker, message)
}

pub fn solver(level: log::Level, message: impl Into<String>) -> LogBuilder {
    LogBuilder::new(level, Stage::Solver, message)
}

pub fn bone(level: log::Level, message: impl Into<String>) -> LogBuilder {
    LogBuilder::new(level, Stage::Bone, message)
}

pub fn gpu(level: log::Level, message: impl Into<String>) -> LogBuilder {
    LogBuilder::new(level, Stage::Gpu, message)
}

pub fn render(level: log::Level, message: impl Into<String>) -> LogBuilder {
    LogBuilder::new(level, Stage::Render, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct TestProvider {
        logs: Arc<Mutex<Vec<String>>>,
    }

    impl LogProvider for TestProvider {
        fn write(&self, entry: &PipelineLog) {
            let fields: Vec<String> = entry
                .fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            let msg = format!(
                "[{}][{}] {} | {}",
                entry.level,
                entry.stage,
                entry.message,
                fields.join(" ")
            );
            self.logs.lock().unwrap().push(msg);
        }
    }

    #[test]
    fn stage_display() {
        assert_eq!(format!("{}", Stage::Camera), "Camera");
        assert_eq!(format!("{}", Stage::Gpu), "GPU");
    }

    #[test]
    fn log_builder_fields() {
        let builder = LogBuilder::new(log::Level::Info, Stage::Camera, "test")
            .field("width", 640)
            .field("height", 480);
        assert_eq!(builder.fields.len(), 2);
        assert_eq!(builder.fields[0], ("width".to_string(), "640".to_string()));
    }
}
