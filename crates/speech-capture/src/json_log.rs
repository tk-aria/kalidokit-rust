//! Verbose JSON logging for speech events.
//!
//! When enabled, every [`SpeechEvent`](crate::SpeechEvent) is serialized to a
//! single-line JSON object and written to the configured output (stdout, file, etc.).

use std::io::Write;
use std::time::Duration;

use serde::Serialize;

use crate::SpeechEvent;

/// A JSON-serializable record for one speech event.
#[derive(Debug, Serialize)]
#[serde(tag = "event")]
pub enum SpeechRecord {
    #[serde(rename = "voice_start")]
    VoiceStart { timestamp_ms: u64 },

    #[serde(rename = "transcript_interim")]
    TranscriptInterim { timestamp_ms: u64, text: String },

    #[serde(rename = "voice_end")]
    VoiceEnd {
        timestamp_ms: u64,
        duration_ms: u64,
        samples: usize,
        transcript: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        end_of_turn: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_probability: Option<f32>,
    },

    #[serde(rename = "vad_status")]
    VadStatus {
        timestamp_ms: u64,
        probability: f32,
        is_voice: bool,
    },
}

impl SpeechRecord {
    /// Convert a [`SpeechEvent`] into a JSON-serializable record.
    pub fn from_event(event: &SpeechEvent) -> Self {
        match event {
            SpeechEvent::VoiceStart { timestamp } => SpeechRecord::VoiceStart {
                timestamp_ms: dur_ms(timestamp),
            },
            SpeechEvent::TranscriptInterim { timestamp, text } => SpeechRecord::TranscriptInterim {
                timestamp_ms: dur_ms(timestamp),
                text: text.clone(),
            },
            SpeechEvent::VoiceEnd {
                timestamp,
                audio,
                duration,
                transcript,
                end_of_turn,
                turn_probability,
            } => SpeechRecord::VoiceEnd {
                timestamp_ms: dur_ms(timestamp),
                duration_ms: dur_ms(duration),
                samples: audio.len(),
                transcript: transcript.clone(),
                end_of_turn: *end_of_turn,
                turn_probability: *turn_probability,
            },
            SpeechEvent::VadStatus {
                timestamp,
                probability,
                is_voice,
            } => SpeechRecord::VadStatus {
                timestamp_ms: dur_ms(timestamp),
                probability: *probability,
                is_voice: *is_voice,
            },
        }
    }
}

fn dur_ms(d: &Duration) -> u64 {
    d.as_millis() as u64
}

/// Verbose JSON logger that writes one JSON line per event.
pub struct JsonLogger<W: Write> {
    writer: W,
}

impl<W: Write> JsonLogger<W> {
    /// Create a new JSON logger writing to the given output.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Log a speech event as a single JSON line.
    pub fn log(&mut self, event: &SpeechEvent) {
        let record = SpeechRecord::from_event(event);
        if let Ok(json) = serde_json::to_string(&record) {
            let _ = writeln!(self.writer, "{json}");
            let _ = self.writer.flush();
        }
    }
}

/// Convenience: create a JSON logger writing to stdout.
pub fn stdout_logger() -> JsonLogger<std::io::Stdout> {
    JsonLogger::new(std::io::stdout())
}

/// Convenience: create a JSON logger writing to a file.
pub fn file_logger(path: &str) -> std::io::Result<JsonLogger<std::io::BufWriter<std::fs::File>>> {
    let file = std::fs::File::create(path)?;
    Ok(JsonLogger::new(std::io::BufWriter::new(file)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_end_to_json() {
        let event = SpeechEvent::VoiceEnd {
            timestamp: Duration::from_millis(3200),
            audio: vec![0i16; 8704],
            duration: Duration::from_millis(500),
            transcript: Some("hello".to_string()),
            end_of_turn: None,
            turn_probability: None,
        };
        let record = SpeechRecord::from_event(&event);
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"event\":\"voice_end\""));
        assert!(json.contains("\"timestamp_ms\":3200"));
        assert!(json.contains("\"duration_ms\":500"));
        assert!(json.contains("\"samples\":8704"));
        assert!(json.contains("\"transcript\":\"hello\""));
    }

    #[test]
    fn voice_start_to_json() {
        let event = SpeechEvent::VoiceStart {
            timestamp: Duration::from_millis(1000),
        };
        let json = serde_json::to_string(&SpeechRecord::from_event(&event)).unwrap();
        assert!(json.contains("\"event\":\"voice_start\""));
        assert!(json.contains("\"timestamp_ms\":1000"));
    }

    #[test]
    fn logger_writes_newline_delimited() {
        let mut buf = Vec::new();
        let mut logger = JsonLogger::new(&mut buf);
        logger.log(&SpeechEvent::VoiceStart {
            timestamp: Duration::from_millis(100),
        });
        logger.log(&SpeechEvent::VoiceEnd {
            timestamp: Duration::from_millis(600),
            audio: vec![],
            duration: Duration::from_millis(500),
            transcript: None,
            end_of_turn: None,
            turn_probability: None,
        });
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("voice_start"));
        assert!(lines[1].contains("voice_end"));
    }
}
