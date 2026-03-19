//! VAD-based speech segmentation state machine.
//!
//! Idle -> Speaking -> TrailingSilence -> (emit VoiceEnd) -> Idle

use std::time::Duration;

use crate::SpeechEvent;

enum State {
    Idle,
    Speaking,
    TrailingSilence { silence_start: Duration },
}

pub struct VadSegmenter {
    state: State,
    min_speech_ms: u32,
    silence_timeout_ms: u32,
    /// Accumulated audio during current speech segment.
    audio_buffer: Vec<i16>,
    /// Timestamp when voice started.
    voice_start: Duration,
}

impl VadSegmenter {
    pub fn new(min_speech_ms: u32, silence_timeout_ms: u32) -> Self {
        Self {
            state: State::Idle,
            min_speech_ms,
            silence_timeout_ms,
            audio_buffer: Vec::new(),
            voice_start: Duration::ZERO,
        }
    }

    /// Returns true if currently in a speaking or trailing-silence state.
    #[allow(dead_code)]
    pub fn is_speaking(&self) -> bool {
        matches!(self.state, State::Speaking | State::TrailingSilence { .. })
    }

    /// Returns accumulated audio for the current speech segment.
    #[allow(dead_code)]
    pub fn accumulated_audio(&self) -> &[i16] {
        &self.audio_buffer
    }

    /// Feed one frame of VAD results. Returns 0-2 events.
    pub fn feed(
        &mut self,
        is_voice: bool,
        samples: &[i16],
        timestamp: Duration,
    ) -> Vec<SpeechEvent> {
        let mut events = Vec::new();

        match &self.state {
            State::Idle => {
                if is_voice {
                    self.voice_start = timestamp;
                    self.audio_buffer.clear();
                    self.audio_buffer.extend_from_slice(samples);
                    self.state = State::Speaking;
                    events.push(SpeechEvent::VoiceStart { timestamp });
                }
            }
            State::Speaking => {
                self.audio_buffer.extend_from_slice(samples);
                if !is_voice {
                    self.state = State::TrailingSilence {
                        silence_start: timestamp,
                    };
                }
            }
            State::TrailingSilence { silence_start } => {
                self.audio_buffer.extend_from_slice(samples);
                if is_voice {
                    // Voice resumed before timeout
                    self.state = State::Speaking;
                } else {
                    let silence_dur = timestamp.saturating_sub(*silence_start);
                    if silence_dur >= Duration::from_millis(self.silence_timeout_ms as u64) {
                        // Silence timeout reached -> emit VoiceEnd
                        let speech_dur = timestamp.saturating_sub(self.voice_start);
                        if speech_dur >= Duration::from_millis(self.min_speech_ms as u64) {
                            events.push(SpeechEvent::VoiceEnd {
                                timestamp,
                                audio: std::mem::take(&mut self.audio_buffer),
                                duration: speech_dur,
                                transcript: None,
                            });
                        } else {
                            // Too short, discard
                            self.audio_buffer.clear();
                        }
                        self.state = State::Idle;
                    }
                }
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_to_speaking_to_end() {
        let mut seg = VadSegmenter::new(0, 100); // no min, 100ms timeout
        let samples = vec![0i16; 256];

        // Voice starts
        let evts = seg.feed(true, &samples, Duration::from_millis(0));
        assert_eq!(evts.len(), 1);
        assert!(matches!(evts[0], SpeechEvent::VoiceStart { .. }));

        // Voice continues
        let evts = seg.feed(true, &samples, Duration::from_millis(50));
        assert!(evts.is_empty());

        // Silence starts
        let evts = seg.feed(false, &samples, Duration::from_millis(100));
        assert!(evts.is_empty()); // trailing silence, not yet timeout

        // Silence timeout
        let evts = seg.feed(false, &samples, Duration::from_millis(250));
        assert_eq!(evts.len(), 1);
        match &evts[0] {
            SpeechEvent::VoiceEnd {
                audio, transcript, ..
            } => {
                assert_eq!(audio.len(), 256 * 4); // 4 frames accumulated
                assert!(transcript.is_none());
            }
            _ => panic!("expected VoiceEnd"),
        }
    }

    #[test]
    fn short_speech_discarded() {
        let mut seg = VadSegmenter::new(300, 50); // min 300ms, 50ms timeout
        let samples = vec![0i16; 256];

        seg.feed(true, &samples, Duration::from_millis(0));
        seg.feed(false, &samples, Duration::from_millis(50));
        let evts = seg.feed(false, &samples, Duration::from_millis(150));
        // Speech was < 300ms, so VoiceEnd should NOT be emitted
        assert!(!evts
            .iter()
            .any(|e| matches!(e, SpeechEvent::VoiceEnd { .. })));
    }

    #[test]
    fn voice_resumes_in_trailing_silence() {
        let mut seg = VadSegmenter::new(0, 200);
        let samples = vec![0i16; 256];

        seg.feed(true, &samples, Duration::from_millis(0)); // VoiceStart
        seg.feed(false, &samples, Duration::from_millis(100)); // trailing silence
        seg.feed(true, &samples, Duration::from_millis(150)); // voice resumes!
                                                              // Should NOT have VoiceEnd
        seg.feed(false, &samples, Duration::from_millis(200));
        let evts = seg.feed(false, &samples, Duration::from_millis(450));
        // Now should end
        assert_eq!(evts.len(), 1);
        assert!(matches!(evts[0], SpeechEvent::VoiceEnd { .. }));
    }
}
