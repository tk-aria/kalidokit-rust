use std::time::Duration;

/// Manages video playback timing and state.
pub struct PlaybackState {
    pub position: Duration,
    pub duration: Duration,
    pub fps: f64,
    pub looping: bool,
    pub paused: bool,
    frame_interval: Duration,
    elapsed_since_frame: Duration,
}

impl PlaybackState {
    pub fn new(duration: Duration, fps: f64, looping: bool) -> Self {
        let frame_interval = if fps > 0.0 {
            Duration::from_secs_f64(1.0 / fps)
        } else {
            Duration::from_millis(33) // fallback ~30fps
        };
        Self {
            position: Duration::ZERO,
            duration,
            fps,
            looping,
            paused: false,
            frame_interval,
            elapsed_since_frame: Duration::ZERO,
        }
    }

    /// Advance by dt. Returns true if a new frame should be decoded.
    pub fn tick(&mut self, dt: Duration) -> bool {
        if self.paused {
            return false;
        }
        self.position = self.position.saturating_add(dt);
        self.elapsed_since_frame = self.elapsed_since_frame.saturating_add(dt);
        if self.elapsed_since_frame >= self.frame_interval {
            self.elapsed_since_frame = self.elapsed_since_frame.saturating_sub(self.frame_interval);
            true
        } else {
            false
        }
    }

    /// Check if stream ended. If looping, reset position and return true.
    /// Returns true if playback should continue (looped), false if end-of-stream.
    pub fn check_end_of_stream(&mut self) -> bool {
        if self.position >= self.duration {
            if self.looping {
                self.position = Duration::ZERO;
                self.elapsed_since_frame = Duration::ZERO;
                true // continue (looped)
            } else {
                false // end of stream
            }
        } else {
            true // not at end
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn seek(&mut self, position: Duration) {
        self.position = position;
        self.elapsed_since_frame = Duration::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_30fps_frame_boundary() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 30.0, false);
        // 33ms >= 33.33ms? No — frame_interval is ~33.333ms
        // Actually 33ms < 33.333ms, so first tick should return false
        // We need >= 33.334ms to trigger
        assert!(!state.tick(Duration::from_millis(33)));
        // Another 1ms pushes us over
        assert!(state.tick(Duration::from_millis(1)));
    }

    #[test]
    fn tick_30fps_large_step() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 30.0, false);
        // 34ms > 33.33ms → should decode
        assert!(state.tick(Duration::from_millis(34)));
    }

    #[test]
    fn tick_16ms_no_frame() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 30.0, false);
        // 16ms < 33.33ms → no frame
        assert!(!state.tick(Duration::from_millis(16)));
    }

    #[test]
    fn loop_resets_position() {
        let mut state = PlaybackState::new(Duration::from_secs(1), 30.0, true);
        state.position = Duration::from_millis(1500);
        assert!(state.check_end_of_stream()); // looping → true
        assert_eq!(state.position, Duration::ZERO);
    }

    #[test]
    fn no_loop_end_of_stream() {
        let mut state = PlaybackState::new(Duration::from_secs(1), 30.0, false);
        state.position = Duration::from_millis(1500);
        assert!(!state.check_end_of_stream()); // not looping → false
    }

    #[test]
    fn pause_returns_false() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 30.0, false);
        state.pause();
        assert!(!state.tick(Duration::from_millis(100)));
        assert_eq!(state.position, Duration::ZERO);
    }

    #[test]
    fn resume_after_pause() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 30.0, false);
        state.pause();
        assert!(!state.tick(Duration::from_millis(100)));
        state.resume();
        assert!(state.tick(Duration::from_millis(100)));
    }

    #[test]
    fn fps_zero_does_not_panic() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 0.0, false);
        // Should use fallback 33ms interval
        assert!(!state.tick(Duration::from_millis(32)));
        assert!(state.tick(Duration::from_millis(2)));
    }

    #[test]
    fn duration_zero_check_end_of_stream() {
        let mut state = PlaybackState::new(Duration::ZERO, 30.0, false);
        // position(0) >= duration(0) → end of stream, not looping → false
        assert!(!state.check_end_of_stream());
    }

    #[test]
    fn duration_zero_looping() {
        let mut state = PlaybackState::new(Duration::ZERO, 30.0, true);
        // position(0) >= duration(0) → looping → resets and returns true
        assert!(state.check_end_of_stream());
        assert_eq!(state.position, Duration::ZERO);
    }

    #[test]
    fn seek_resets_elapsed() {
        let mut state = PlaybackState::new(Duration::from_secs(10), 30.0, false);
        state.tick(Duration::from_millis(20));
        state.seek(Duration::from_secs(5));
        assert_eq!(state.position, Duration::from_secs(5));
        // elapsed_since_frame should be reset, so small tick shouldn't trigger frame
        assert!(!state.tick(Duration::from_millis(1)));
    }
}
