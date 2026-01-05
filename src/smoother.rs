//! Parameter smoothing to prevent zipper noise

#[derive(Debug, Clone)]
pub struct ParamSmoother {
    current: f32,
    target: f32,
    coeff: f32,
}

impl ParamSmoother {
    /// Create a new smoother with default 10ms time constant at 44.1kHz
    pub fn new() -> Self {
        Self::with_time_constant(0.01, 44100.0) // 10ms at 44.1kHz
    }

    /// Create a smoother with specific time constant and sample rate
    pub fn with_time_constant(time_constant_seconds: f32, sample_rate: f32) -> Self {
        let coeff = (-1.0 / (time_constant_seconds * sample_rate)).exp();
        Self {
            current: 0.0,
            target: 0.0,
            coeff,
        }
    }

    /// Set the target value (instantaneous)
    pub fn set_target(&mut self, new_target: f32) {
        self.target = new_target;
    }

    /// Get the next smoothed sample
    pub fn next_sample(&mut self) -> f32 {
        self.current = self.current * self.coeff + self.target * (1.0 - self.coeff);
        self.current
    }

    /// Get current value without advancing
    pub fn current_value(&self) -> f32 {
        self.current
    }

    /// Reset to a specific value
    pub fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }
}

impl Default for ParamSmoother {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoother_converges() {
        let mut smoother = ParamSmoother::with_time_constant(0.001, 44100.0); // 1ms for faster convergence
        smoother.set_target(1.0);

        // Should asymptotically approach 1.0
        for _ in 0..1000 {
            let _ = smoother.next_sample();
        }

        let final_value = smoother.current_value();
        assert!((final_value - 1.0).abs() < 0.01);
    }

    #[test]
    fn step_input_smooth_output() {
        let mut smoother = ParamSmoother::new();
        smoother.reset(0.0);

        // Step to 1.0
        smoother.set_target(1.0);

        let sample1 = smoother.next_sample();
        let sample2 = smoother.next_sample();

        // Should be increasing but not instant
        assert!(sample1 > 0.0 && sample1 < 1.0);
        assert!(sample2 > sample1 && sample2 < 1.0);
    }

    #[test]
    fn reset_works() {
        let mut smoother = ParamSmoother::new();
        smoother.set_target(1.0);

        for _ in 0..100 {
            let _ = smoother.next_sample();
        }

        smoother.reset(0.5);
        assert!((smoother.current_value() - 0.5).abs() < 0.01);
    }
}