//! VAD-based speech segmentation state machine.
//!
//! Idle -> Speaking -> TrailingSilence -> (emit VoiceEnd) -> Idle

use std::time::Duration;

use crate::SpeechEvent;

enum State {
    Idle,
    Speaking,
    TrailingSilence {
        silence_start: Duration,
        /// ETD predicted incomplete → use extended timeout to allow merge.
        etd_incomplete: bool,
    },
}

/// ETD predictor result for streaming early-cut.
pub struct EarlyCutResult {
    pub prediction: bool,
    pub probability: f32,
}

/// Callback type for ETD streaming early-cut prediction.
/// Called with accumulated audio when entering TrailingSilence.
/// Returns `Some(result)` if ETD is available, `None` otherwise.
pub type EtdPredictFn = Box<dyn FnMut(&[i16]) -> Option<EarlyCutResult> + Send>;

pub struct VadSegmenter {
    state: State,
    min_speech_ms: u32,
    silence_timeout_ms: u32,
    /// Extended silence timeout when ETD predicts incomplete (merge mode).
    /// Allows mid-turn pauses to be merged into one segment.
    extended_silence_timeout_ms: u32,
    /// Accumulated audio during current speech segment.
    audio_buffer: Vec<i16>,
    /// Timestamp when voice started.
    voice_start: Duration,
    /// Optional ETD predictor for streaming early-cut mode.
    etd_predict: Option<EtdPredictFn>,
}

impl VadSegmenter {
    pub fn new(min_speech_ms: u32, silence_timeout_ms: u32) -> Self {
        Self {
            state: State::Idle,
            min_speech_ms,
            silence_timeout_ms,
            extended_silence_timeout_ms: silence_timeout_ms * 3,
            audio_buffer: Vec::new(),
            voice_start: Duration::ZERO,
            etd_predict: None,
        }
    }

    /// Set extended silence timeout for ETD-incomplete merge mode (ms).
    /// When ETD predicts "incomplete", the segmenter waits this long
    /// instead of the normal silence_timeout, allowing mid-turn pauses
    /// to be merged into one segment.
    #[allow(dead_code)]
    pub fn set_extended_silence_timeout_ms(&mut self, ms: u32) {
        self.extended_silence_timeout_ms = ms;
    }

    /// Set an ETD predictor for streaming early-cut mode.
    /// When entering TrailingSilence, if the predictor returns `prediction=true`,
    /// VoiceEnd is emitted immediately without waiting for silence_timeout.
    #[allow(dead_code)]
    pub fn set_etd_predict(&mut self, predict: EtdPredictFn) {
        self.etd_predict = Some(predict);
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
                    // Streaming ETD: check at silence onset.
                    let mut etd_incomplete = false;
                    if let Some(ref mut predict_fn) = self.etd_predict {
                        if let Some(etd_result) = predict_fn(&self.audio_buffer) {
                            if etd_result.prediction {
                                // ETD predicts turn complete → emit VoiceEnd immediately.
                                let speech_dur = timestamp.saturating_sub(self.voice_start);
                                if speech_dur >= Duration::from_millis(self.min_speech_ms as u64) {
                                    events.push(SpeechEvent::VoiceEnd {
                                        timestamp,
                                        audio: std::mem::take(&mut self.audio_buffer),
                                        duration: speech_dur,
                                        transcript: None,
                                        end_of_turn: Some(true),
                                        turn_probability: Some(etd_result.probability),
                                    });
                                } else {
                                    self.audio_buffer.clear();
                                }
                                self.state = State::Idle;
                                return events;
                            } else {
                                // ETD predicts incomplete → use extended timeout for merge.
                                etd_incomplete = true;
                            }
                        }
                    }
                    self.state = State::TrailingSilence {
                        silence_start: timestamp,
                        etd_incomplete,
                    };
                }
            }
            State::TrailingSilence {
                silence_start,
                etd_incomplete,
            } => {
                self.audio_buffer.extend_from_slice(samples);
                if is_voice {
                    // Voice resumed before timeout → merge into same segment.
                    self.state = State::Speaking;
                } else {
                    let silence_dur = timestamp.saturating_sub(*silence_start);
                    let timeout_ms = if *etd_incomplete {
                        self.extended_silence_timeout_ms
                    } else {
                        self.silence_timeout_ms
                    };
                    if silence_dur >= Duration::from_millis(timeout_ms as u64) {
                        // Silence timeout reached -> emit VoiceEnd
                        let speech_dur = timestamp.saturating_sub(self.voice_start);
                        if speech_dur >= Duration::from_millis(self.min_speech_ms as u64) {
                            events.push(SpeechEvent::VoiceEnd {
                                timestamp,
                                audio: std::mem::take(&mut self.audio_buffer),
                                duration: speech_dur,
                                transcript: None,
                                end_of_turn: Some(!etd_incomplete),
                                turn_probability: None,
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

    #[test]
    fn streaming_early_cut() {
        let mut seg = VadSegmenter::new(0, 500); // 500ms timeout
        let samples = vec![0i16; 256];

        // Set ETD predictor that always returns "complete".
        seg.set_etd_predict(Box::new(|_audio: &[i16]| {
            Some(EarlyCutResult {
                prediction: true,
                probability: 0.95,
            })
        }));

        // Voice starts
        let evts = seg.feed(true, &samples, Duration::from_millis(0));
        assert_eq!(evts.len(), 1);
        assert!(matches!(evts[0], SpeechEvent::VoiceStart { .. }));

        // Voice continues
        seg.feed(true, &samples, Duration::from_millis(50));

        // Silence starts → ETD predicts complete → immediate VoiceEnd
        let evts = seg.feed(false, &samples, Duration::from_millis(100));
        assert_eq!(evts.len(), 1);
        match &evts[0] {
            SpeechEvent::VoiceEnd {
                end_of_turn,
                turn_probability,
                ..
            } => {
                assert_eq!(*end_of_turn, Some(true));
                assert!((turn_probability.unwrap() - 0.95).abs() < 0.01);
            }
            _ => panic!("expected VoiceEnd with ETD fields"),
        }
    }

    #[test]
    fn streaming_no_early_cut() {
        let mut seg = VadSegmenter::new(0, 100); // 100ms timeout
        let samples = vec![0i16; 256];

        // Set ETD predictor that returns "incomplete".
        seg.set_etd_predict(Box::new(|_audio: &[i16]| {
            Some(EarlyCutResult {
                prediction: false,
                probability: 0.3,
            })
        }));

        // Voice starts
        seg.feed(true, &samples, Duration::from_millis(0));

        // Silence starts → ETD predicts incomplete → no early VoiceEnd
        let evts = seg.feed(false, &samples, Duration::from_millis(100));
        assert!(
            evts.is_empty(),
            "should not emit VoiceEnd for incomplete ETD"
        );

        // Normal timeout (100ms) would fire at 250ms, but ETD=incomplete
        // extends timeout to 300ms (100ms * 3). So 250ms is still waiting.
        let evts = seg.feed(false, &samples, Duration::from_millis(250));
        assert!(evts.is_empty(), "extended timeout should still be waiting");

        // Extended timeout reached → VoiceEnd with end_of_turn=Some(false)
        let evts = seg.feed(false, &samples, Duration::from_millis(450));
        assert_eq!(evts.len(), 1);
        match &evts[0] {
            SpeechEvent::VoiceEnd {
                end_of_turn,
                turn_probability,
                ..
            } => {
                assert_eq!(*end_of_turn, Some(false));
                assert_eq!(*turn_probability, None);
            }
            _ => panic!("expected VoiceEnd"),
        }
    }

    #[test]
    fn etd_incomplete_merges_segments() {
        let mut seg = VadSegmenter::new(0, 200); // 200ms normal timeout
        // extended = 200*3 = 600ms
        let samples = vec![0i16; 256];

        // ETD returns incomplete → merge mode.
        seg.set_etd_predict(Box::new(|_audio: &[i16]| {
            Some(EarlyCutResult {
                prediction: false,
                probability: 0.3,
            })
        }));

        // First phrase starts
        seg.feed(true, &samples, Duration::from_millis(0));
        seg.feed(true, &samples, Duration::from_millis(100));

        // Pause begins → ETD=incomplete → extended timeout (600ms)
        seg.feed(false, &samples, Duration::from_millis(200));

        // 400ms of silence: still within extended timeout
        let evts = seg.feed(false, &samples, Duration::from_millis(600));
        assert!(evts.is_empty(), "should still be waiting (extended timeout)");

        // Voice resumes at 650ms → merge into same segment!
        seg.feed(true, &samples, Duration::from_millis(650));
        seg.feed(true, &samples, Duration::from_millis(700));

        // Second silence → ETD=incomplete again
        seg.feed(false, &samples, Duration::from_millis(800));

        // Extended timeout reached (800 + 600 = 1400ms)
        let evts = seg.feed(false, &samples, Duration::from_millis(1500));
        assert_eq!(evts.len(), 1);
        match &evts[0] {
            SpeechEvent::VoiceEnd { audio, .. } => {
                // Should contain audio from BOTH phrases merged together
                // Frames: 0, 100, 200, 600, 650, 700, 800, 1500 = 8 frames
                assert_eq!(audio.len(), 256 * 8);
            }
            _ => panic!("expected VoiceEnd"),
        }
    }

    #[test]
    fn streaming_etd_error_falls_through() {
        let mut seg = VadSegmenter::new(0, 100);
        let samples = vec![0i16; 256];

        // Set ETD predictor that returns None (error case).
        seg.set_etd_predict(Box::new(|_audio: &[i16]| None));

        seg.feed(true, &samples, Duration::from_millis(0));
        // Silence starts → ETD returns None → falls through to normal timeout
        let evts = seg.feed(false, &samples, Duration::from_millis(100));
        assert!(evts.is_empty());

        // Normal timeout
        let evts = seg.feed(false, &samples, Duration::from_millis(250));
        assert_eq!(evts.len(), 1);
        assert!(matches!(evts[0], SpeechEvent::VoiceEnd { .. }));
    }
}
