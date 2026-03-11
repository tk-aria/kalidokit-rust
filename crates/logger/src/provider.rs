use crate::PipelineLog;

/// Logging Provider trait (inspired by ZLogger's ILoggerProvider).
///
/// Each provider decides how to format and where to output pipeline logs.
/// Multiple providers can be registered simultaneously (fan-out).
pub trait LogProvider: Send + Sync {
    /// Write a structured pipeline log entry.
    fn write(&self, log: &PipelineLog);

    /// Flush any buffered output.
    fn flush(&self) {}
}

/// Console provider: writes structured pipeline logs to stderr via the `log` crate.
///
/// Format: `[Stage] message | key=value key=value ...`
pub struct ConsoleProvider {
    min_level: log::Level,
}

impl ConsoleProvider {
    pub fn new(min_level: log::Level) -> Self {
        Self { min_level }
    }
}

impl LogProvider for ConsoleProvider {
    fn write(&self, entry: &PipelineLog) {
        if entry.level > self.min_level {
            return;
        }

        let fields = if entry.fields.is_empty() {
            String::new()
        } else {
            let pairs: Vec<String> = entry
                .fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            format!(" | {}", pairs.join(" "))
        };

        let msg = format!("[{}] {}{}", entry.stage, entry.message, fields);

        match entry.level {
            log::Level::Error => log::error!(target: "pipeline", "{}", msg),
            log::Level::Warn => log::warn!(target: "pipeline", "{}", msg),
            log::Level::Info => log::info!(target: "pipeline", "{}", msg),
            log::Level::Debug => log::debug!(target: "pipeline", "{}", msg),
            log::Level::Trace => log::trace!(target: "pipeline", "{}", msg),
        }
    }
}
