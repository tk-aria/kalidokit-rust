use std::time::Instant;

/// Blink animation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlinkMode {
    /// Use webcam tracking for blink detection.
    Tracking,
    /// Automatic periodic blink using xorshift PRNG.
    Auto,
}

/// Automatic blink controller using xorshift32 for random intervals.
pub struct AutoBlink {
    /// Current blink blend shape value (0=open, 1=closed).
    pub value: f32,
    /// xorshift32 state (must be non-zero).
    state: u32,
    /// When the next blink should start.
    next_blink_time: Instant,
    /// Current phase of the blink animation.
    phase: BlinkPhase,
    /// When the current phase started.
    phase_start: Instant,
}

#[derive(Debug, Clone, Copy)]
enum BlinkPhase {
    /// Eyes open, waiting for next blink.
    Idle,
    /// Eyes closing.
    Closing,
    /// Eyes held shut briefly.
    Closed,
    /// Eyes opening back up.
    Opening,
}

/// Duration of the closing phase in seconds (~human average 100-150ms).
// const CLOSE_DURATION: f32 = 0.06; // v1: too fast, felt robotic
const CLOSE_DURATION: f32 = 0.12;
/// Duration the eyes stay fully closed (~50-80ms).
const CLOSED_HOLD: f32 = 0.06;
/// Duration of the opening phase in seconds (~human average 150-250ms).
// const OPEN_DURATION: f32 = 0.12; // v1: too fast, felt robotic
const OPEN_DURATION: f32 = 0.20;

impl AutoBlink {
    pub fn new() -> Self {
        // Seed from system time (lower bits of nanos for variety).
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(12345);
        let seed = if seed == 0 { 1 } else { seed };

        let mut ab = Self {
            value: 0.0,
            state: seed,
            next_blink_time: Instant::now(),
            phase: BlinkPhase::Idle,
            phase_start: Instant::now(),
        };
        ab.schedule_next_blink();
        ab
    }

    /// Update the blink value. Call once per frame.
    pub fn update(&mut self) {
        let now = Instant::now();

        match self.phase {
            BlinkPhase::Idle => {
                self.value = 0.0;
                if now >= self.next_blink_time {
                    self.phase = BlinkPhase::Closing;
                    self.phase_start = now;
                }
            }
            BlinkPhase::Closing => {
                let elapsed = now.duration_since(self.phase_start).as_secs_f32();
                let t = (elapsed / CLOSE_DURATION).min(1.0);
                // Ease in: slow start, fast close (like real eyelids accelerating)
                self.value = t * t;
                if t >= 1.0 {
                    self.value = 1.0;
                    self.phase = BlinkPhase::Closed;
                    self.phase_start = now;
                }
            }
            BlinkPhase::Closed => {
                self.value = 1.0;
                let elapsed = now.duration_since(self.phase_start).as_secs_f32();
                if elapsed >= CLOSED_HOLD {
                    self.phase = BlinkPhase::Opening;
                    self.phase_start = now;
                }
            }
            BlinkPhase::Opening => {
                let elapsed = now.duration_since(self.phase_start).as_secs_f32();
                let t = (elapsed / OPEN_DURATION).min(1.0);
                // Ease out: fast open, slow settle (natural eye opening)
                let eased = t * (2.0 - t); // ease-out quadratic
                self.value = 1.0 - eased;
                if t >= 1.0 {
                    self.value = 0.0;
                    self.phase = BlinkPhase::Idle;
                    self.schedule_next_blink();
                }
            }
        }
    }

    /// Schedule the next blink 2.3–4.0 seconds from now using xorshift32.
    fn schedule_next_blink(&mut self) {
        let rand = self.xorshift32();
        // Map to [2.3, 4.0] range
        let t = (rand as f32) / (u32::MAX as f32); // [0, 1)
        let interval = 2.3 + t * 1.7; // [2.3, 4.0]
        self.next_blink_time = Instant::now() + std::time::Duration::from_secs_f32(interval);
    }

    /// xorshift32 PRNG.
    fn xorshift32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xorshift_produces_varying_values() {
        let mut ab = AutoBlink::new();
        let a = ab.xorshift32();
        let b = ab.xorshift32();
        let c = ab.xorshift32();
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn auto_blink_starts_idle() {
        let ab = AutoBlink::new();
        assert_eq!(ab.value, 0.0);
        assert!(matches!(ab.phase, BlinkPhase::Idle));
    }

    #[test]
    fn blink_mode_equality() {
        assert_eq!(BlinkMode::Tracking, BlinkMode::Tracking);
        assert_ne!(BlinkMode::Tracking, BlinkMode::Auto);
    }
}
