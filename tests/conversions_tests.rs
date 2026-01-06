//! Tests for MIDI conversions

use auxide_midi::{note_to_freq, velocity_to_gain, pitch_bend_to_ratio};
use proptest::prelude::*;

#[test]
fn a4_golden_value() {
    // Note 69 (A4) should be 440 Hz
    let freq = note_to_freq(69);
    assert!((freq - 440.0).abs() < 0.01);
}

#[test]
fn note_frequency_increases() {
    let c4 = note_to_freq(60); // C4
    let c5 = note_to_freq(72); // C5 (octave higher)

    assert!((c5 - c4 * 2.0).abs() < 0.01);
}

#[test]
fn note_frequency_semitone_ratio() {
    let c4 = note_to_freq(60);
    let csharp4 = note_to_freq(61);

    let ratio = csharp4 / c4;
    assert!((ratio - 2.0_f32.powf(1.0/12.0)).abs() < 0.001);
}

#[test]
fn velocity_gain_curve() {
    // Velocity 0 = silent
    assert_eq!(velocity_to_gain(0), 0.0);

    // Velocity 127 ≈ 1.0
    let max_gain = velocity_to_gain(127);
    assert!((max_gain - 1.0).abs() < 0.01);

    // Velocity 64 should be 0.25 (since (64/127)² ≈ 0.25)
    let mid_gain = velocity_to_gain(64);
    assert!((mid_gain - 0.25).abs() < 0.01);
}

#[test]
fn velocity_gain_is_convex() {
    // Gain should increase faster than linear
    let v64 = velocity_to_gain(64);
    let v96 = velocity_to_gain(96);

    // 96/127 ≈ 0.756, (0.756)² ≈ 0.571
    assert!(v96 > 0.571 * 0.9); // Allow some tolerance
    assert!(v96 < 0.571 * 1.1);
}

#[test]
fn pitch_bend_neutral() {
    // Center position (8192) should be ratio 1.0
    let ratio = pitch_bend_to_ratio(8192);
    assert!((ratio - 1.0).abs() < 0.001);
}

#[test]
fn pitch_bend_up() {
    // Maximum up (16383) should be +2 semitones
    let ratio = pitch_bend_to_ratio(16383);
    let expected = 2.0_f32.powf(2.0 / 12.0);
    assert!((ratio - expected).abs() < 0.001);
}

#[test]
fn pitch_bend_down() {
    // Maximum down (0) should be -2 semitones
    let ratio = pitch_bend_to_ratio(0);
    let expected = 2.0_f32.powf(-2.0 / 12.0);
    assert!((ratio - expected).abs() < 0.001);
}

#[test]
fn pitch_bend_symmetric() {
    let up_ratio = pitch_bend_to_ratio(12288); // +1 semitone
    let down_ratio = pitch_bend_to_ratio(4096); // -1 semitone

    let expected_up = 2.0_f32.powf(1.0 / 12.0);
    let expected_down = 2.0_f32.powf(-1.0 / 12.0);

    assert!((up_ratio - expected_up).abs() < 0.001);
    assert!((down_ratio - expected_down).abs() < 0.001);
    assert!((up_ratio * down_ratio - 1.0).abs() < 0.001); // Should multiply to 1
}

#[test]
fn note_range() {
    // Test some edge cases
    let low_note = note_to_freq(0); // C-1
    let high_note = note_to_freq(127); // G9

    assert!(low_note > 0.0);
    assert!(high_note > low_note);
    assert!(high_note < 100000.0); // Reasonable upper bound
}

proptest! {
    #[test]
    fn note_to_freq_no_panic(note in 0u8..128) {
        let freq = note_to_freq(note);
        // Should be positive and finite
        prop_assert!(freq > 0.0);
        prop_assert!(freq.is_finite());
        // Should be in reasonable range
        prop_assert!(freq >= 8.0); // C-1 ≈ 8.18 Hz
        prop_assert!(freq <= 16000.0); // Above hearing range but reasonable
    }

    #[test]
    fn note_to_freq_monotonic_increasing(note1 in 0u8..127, note2 in 1u8..128) {
        prop_assume!(note1 < note2);
        let freq1 = note_to_freq(note1);
        let freq2 = note_to_freq(note2);
        prop_assert!(freq2 > freq1);
    }

    #[test]
    fn note_to_freq_octave_doubling(note in 0u8..116) { // Leave room for +12
        let freq1 = note_to_freq(note);
        let freq2 = note_to_freq(note + 12);
        let ratio = freq2 / freq1;
        prop_assert!((ratio - 2.0).abs() < 0.001);
    }

    #[test]
    fn velocity_to_gain_no_panic(velocity in 0u8..128) {
        let gain = velocity_to_gain(velocity);
        prop_assert!(gain >= 0.0);
        prop_assert!(gain <= 1.0);
        prop_assert!(gain.is_finite());
    }

    #[test]
    fn velocity_to_gain_monotonic(vel1 in 0u8..127, vel2 in 1u8..128) {
        prop_assume!(vel1 < vel2);
        let gain1 = velocity_to_gain(vel1);
        let gain2 = velocity_to_gain(vel2);
        prop_assert!(gain2 > gain1);
    }

    #[test]
    fn pitch_bend_to_ratio_no_panic(bend in 0i16..16384) {
        let ratio = pitch_bend_to_ratio(bend);
        prop_assert!(ratio > 0.0);
        prop_assert!(ratio.is_finite());
        // Should be within reasonable range (±2 semitones)
        prop_assert!(ratio >= 2.0_f32.powf(-2.0/12.0));
        prop_assert!(ratio <= 2.0_f32.powf(2.0/12.0));
    }

    #[test]
    fn pitch_bend_center_is_unity(bend in 8190i16..8194) {
        let ratio = pitch_bend_to_ratio(bend);
        prop_assert!((ratio - 1.0).abs() < 0.001);
    }
}