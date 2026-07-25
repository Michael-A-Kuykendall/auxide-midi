//! Cross-crate integration gate for the auxide stack.
//!
//! Proves the full chain works end-to-end without any hardware:
//!   1. A DSP graph built with `auxide-dsp`'s `SynthBuilder` is executed by
//!      the `auxide` kernel (`RuntimeCore` + `render_offline_handle`) and
//!      produces non-zero audio.
//!   2. The `auxide-midi` bridge types construct and the voice pool
//!      allocates/releases on note-on/note-off with no device required.

use auxide::rt::{render_offline_handle, RuntimeCore};
use auxide_dsp::builders::SynthBuilder;
use auxide_dsp::SawOsc;
use auxide_midi::MidiBridgeConfig;
use auxide_midi::VoiceAllocator;

#[test]
fn synthbuilder_renders_nonzero_through_kernel() {
    let (graph, plan) = SynthBuilder::new()
        .add_oscillator(SawOsc::new(220.0))
        .build(64)
        .expect("synth graph should compile to a plan");

    let (mut handle, _control) = RuntimeCore::new_with_channels(plan, &graph, 44_100.0);

    let out = render_offline_handle(&mut handle, 44_100).expect("offline render should succeed");

    assert!(
        out.iter().any(|&s| s.abs() > 1e-6),
        "kernel render produced only silence; expected non-zero audio"
    );
}

#[test]
fn midi_voice_pool_allocates_on_note_on_and_off() {
    // Construct the bridge config and drive the voice pool with no hardware.
    let _config = MidiBridgeConfig::default();

    let mut allocator = VoiceAllocator::new();
    let voice = allocator
        .allocate_voice(60)
        .expect("note-on should allocate a voice");
    assert_eq!(allocator.active_voice_count(), 1);

    allocator.release_voice(60);
    assert_eq!(allocator.active_voice_count(), 0);
    assert!(voice.0 < 8, "voice id should be within the pool");
}
