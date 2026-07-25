use std::sync::Arc;

use auxide::rt::RuntimeCore;

use crate::midi_bridge::{build_rompler_graph, MidiBridgeConfig, MidiToAudioBridge};

/// A user-facing polyphonic synthesizer that wraps [`MidiToAudioBridge`] +
/// [`RuntimeCore`] into a single `note_on`/`note_off`/`process` API.
///
/// ## Construction
///
/// Call [`Synth::new`] with a sample buffer and sample rate:
///
/// ```ignore
/// let sample = std::sync::Arc::new(
///     (0..44100).map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin()).collect()
/// );
/// let mut synth = auxide_midi::Synth::new(sample, 44100.0, 8, 69);
/// let mut out = vec![0.0; 64];
/// synth.note_on(69, 100);
/// synth.process_block(&mut out).unwrap();
/// ```
pub struct Synth {
    bridge: MidiToAudioBridge,
}

impl Synth {
    /// Build a polyphonic synth from a recorded sample.
    ///
    /// * `sample` — mono PCM buffer (floating-point, -1..1).
    /// * `sample_rate` — stream sample rate (Hz).
    /// * `polyphony` — maximum simultaneous voices (≤ 16 recommended).
    /// * `anchor_note` — MIDI note the sample corresponds to (pitch center).
    pub fn new(sample: Arc<Vec<f32>>, sample_rate: f32, polyphony: usize, anchor_note: u8) -> Self {
        let (graph, plan, voice_nodes, filter_node) =
            build_rompler_graph(polyphony, sample, sample_rate, anchor_note);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, sample_rate);
        let bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, filter_node);
        Synth { bridge }
    }

    /// Start a note (triggers oscillator + envelope).
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.bridge.route_note_on(note, velocity);
    }

    /// Stop a note (starts envelope release phase).
    pub fn note_off(&mut self, note: u8) {
        self.bridge.route_note_off(note);
    }

    /// Render one block of audio (must match the block size from the plan, typically 64).
    pub fn process_block(&mut self, out: &mut [f32]) -> Result<(), &'static str> {
        self.bridge.process_block(out)
    }

    /// Number of currently active (sounding) voices.
    pub fn active_voice_count(&self) -> usize {
        self.bridge.active_voice_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a mono sine sample at `freq` Hz lasting `dur_s` seconds.
    fn make_sine(freq: f32, dur_s: f32, sr: f32) -> Arc<Vec<f32>> {
        let n = (dur_s * sr) as usize;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            v.push((2.0 * std::f32::consts::PI * freq * (i as f32) / sr).sin());
        }
        Arc::new(v)
    }

    #[test]
    fn synth_plays() {
        let sample = make_sine(440.0, 2.0, 44100.0);
        let mut synth = Synth::new(sample, 44100.0, 8, 69);
        let mut out = vec![0.0; 64];

        // Before any note, output should be silence
        synth.process_block(&mut out).unwrap();
        let peak_before = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak_before < 1e-6, "silence before note_on: {peak_before}");

        // Note on → non-zero audio
        synth.note_on(69, 100);
        for _ in 0..10 {
            synth.process_block(&mut out).unwrap();
        }
        let peak_playing = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak_playing > 0.01,
            "should produce audio during note: {peak_playing}"
        );

        // Note off → eventual silence after release
        synth.note_off(69);
        for _ in 0..200 {
            synth.process_block(&mut out).unwrap();
        }
        let peak_release = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak_release < 1e-3,
            "should decay after note_off: {peak_release}"
        );
    }

    #[test]
    fn synth_polyphony() {
        let sample = make_sine(440.0, 2.0, 44100.0);
        let mut synth = Synth::new(sample, 44100.0, 8, 69);

        // 8 simultaneous notes
        for note in [60u8, 64, 67, 69, 72, 76, 79, 81] {
            synth.note_on(note, 100);
        }
        assert_eq!(synth.active_voice_count(), 8, "8 voices active");

        // All 8 should produce audio
        let mut out = vec![0.0; 64];
        for _ in 0..10 {
            synth.process_block(&mut out).unwrap();
        }
        let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "8 voices should produce audio: {peak}");

        // 9th note steals oldest — still 8 active, no panic
        synth.note_on(84, 100);
        assert_eq!(synth.active_voice_count(), 8, "still 8 after steal");
        for _ in 0..10 {
            synth.process_block(&mut out).unwrap();
        }
        let peak_steal = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak_steal > 0.01, "still playing after steal: {peak_steal}");
    }
}
