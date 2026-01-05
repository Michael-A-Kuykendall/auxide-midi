//! MIDI note and parameter conversions

/// Convert MIDI note number to frequency in Hz
/// Formula: 440.0 * 2^((note - 69) / 12.0)
pub fn note_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

/// Convert MIDI velocity to linear gain
/// Formula: (velocity / 127)^2 for natural feel
pub fn velocity_to_gain(velocity: u8) -> f32 {
    (velocity as f32 / 127.0).powf(2.0)
}

/// Convert MIDI pitch bend to frequency ratio
/// Range: ±2 semitones (8192 = center, 0 = -2, 16383 = +2)
pub fn pitch_bend_to_ratio(bend: i16) -> f32 {
    let semitones = ((bend - 8192) as f32 / 8192.0) * 2.0;
    2.0_f32.powf(semitones / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_golden_value() {
        // Note 69 (A4) should be 440 Hz
        assert!((note_to_freq(69) - 440.0).abs() < 0.01);
    }

    #[test]
    fn velocity_gain_curve() {
        // Velocity 0 should be silent
        assert_eq!(velocity_to_gain(0), 0.0);
        // Velocity 127 should be 1.0
        assert!((velocity_to_gain(127) - 1.0).abs() < 0.01);
        // Velocity 64 should be 0.25 (since (64/127)^2 ≈ 0.25)
        assert!((velocity_to_gain(64) - 0.25).abs() < 0.01);
    }

    #[test]
    fn pitch_bend_neutral() {
        // Center position (8192) should be ratio 1.0
        assert!((pitch_bend_to_ratio(8192) - 1.0).abs() < 0.01);
    }

    #[test]
    fn pitch_bend_range() {
        // Minimum (0) should be -2 semitones
        let min_ratio = pitch_bend_to_ratio(0);
        assert!((min_ratio - 2.0_f32.powf(-2.0 / 12.0)).abs() < 0.01);

        // Maximum (16383) should be +2 semitones
        let max_ratio = pitch_bend_to_ratio(16383);
        assert!((max_ratio - 2.0_f32.powf(2.0 / 12.0)).abs() < 0.01);
    }
}