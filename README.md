# auxide-midi

<img src="assets/auxide-midi-logo.png" alt="auxide-midi logo" width="400"/>

[![Crates.io](https://img.shields.io/crates/v/auxide-midi.svg)](https://crates.io/crates/auxide-midi)
[![Documentation](https://docs.rs/auxide-midi/badge.svg)](https://docs.rs/auxide-midi)
[![CI](https://github.com/Michael-A-Kuykendall/auxide-midi/workflows/CI/badge.svg)](https://github.com/Michael-A-Kuykendall/auxide-midi/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 💝 Support Auxide's Growth

🚀 If Auxide helps you build amazing audio tools, consider [sponsoring](https://github.com/sponsors/Michael-A-Kuykendall) — 100% of support goes to keeping it free forever.

• $5/month: Coffee tier ☕ - Eternal gratitude + sponsor badge
• $25/month: Bug prioritizer 🐛 - Priority support + name in [SPONSORS.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/SPONSORS.md)
• $100/month: Corporate backer 🏢 - Logo placement + monthly office hours
• $500/month: Infrastructure partner 🚀 - Direct support + roadmap input

**[🎯 Become a Sponsor](https://github.com/sponsors/Michael-A-Kuykendall)** | See our amazing [sponsors](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/SPONSORS.md) 🙏

MIDI input integration and voice allocation for Auxide DSP graphs.

This crate provides real-time MIDI input handling and voice allocation for polyphonic synthesis. It integrates with auxide-dsp nodes and drives the auxide kernel's runtime control plane — RT-safe `note_on`/`note_off`, CC mapping, and pitch-bend routed through the lock-free control queue.

## Status

- ✅ MIDI Input: Real-time MIDI input handling with midir
- ✅ Voice Allocation: Polyphonic voice management with note stealing
- ✅ RT-Safety: Verified zero allocations in process paths
- ✅ Integration: Seamless auxide-dsp node parameter control
- 📋 CC Mapping: Basic MIDI CC parameter mapping implemented

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
auxide = "0.3"
auxide-dsp = "0.2"
auxide-midi = "0.2"
```

## Examples

### Polyphonic ROMpler with Synth Facade

```rust
use std::sync::Arc;
use auxide_midi::Synth;

let sr = 44100.0;
let sample: Arc<Vec<f32>> = Arc::new(
    (0..44100)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin())
        .collect(),
);
let mut synth = Synth::new(sample, sr, 8, 69); // 8-voice polyphonic ROMpler
let mut out = vec![0.0f32; 64];
synth.note_on(69, 100);
for _ in 0..10 {
    synth.process_block(&mut out).unwrap();
}
assert!(out.iter().any(|&s| s.abs() > 1e-3));
synth.note_off(69);
```

Each note routes through `RuntimeCore`'s lock-free control queue — `SetFrequency` + `TriggerGate` on per-voice oscillator/envelope pairs.

### MidiInputHandler — Receiving MIDI Events

<details>
<summary>Expand</summary>

```rust
use auxide_midi::MidiInputHandler;

let mut handler = MidiInputHandler::new();
// List available devices
for (i, name) in handler.list_devices().unwrap().iter().enumerate() {
    println!("{}: {}", i, name);
}
// Connect to first device
handler.connect_device(0).unwrap();
// Poll for events in real-time loop
while running {
    if let Some(event) = handler.try_recv() {
        match event {
            MidiEvent::NoteOn(note, vel) => { /* route */ }
            MidiEvent::NoteOff(note, _) => { /* release */ }
            MidiEvent::ControlChange(cc, value) => { /* map */ }
            MidiEvent::PitchBend(bend) => { /* 14-bit, center=8192 */ }
            _ => {}
        }
    }
}
```
</details>

### VoiceAllocator — Polyphonic Voice Management

<details>
<summary>Expand</summary>

```rust
use auxide_midi::voice_allocator::VoiceAllocator;

let mut alloc = VoiceAllocator::new(); // 8 voices max
if let Some(id) = alloc.allocate_voice(60) {
    // id.0 = slot index (0..7), note 60 = middle C
    activate_voice(id, 60);
}
alloc.release_voice(60); // frees the slot
// When all slots full, steals oldest voice (LRU)
```
</details>

### CC Mapping — Control Parameters

<details>
<summary>Expand</summary>

```rust
use auxide_midi::cc_mapping::{CCMap, ParamTarget};

let mut map = CCMap::new();
// Default: CC1 → FilterCutoff, CC74 → FilterResonance
// Custom mapping:
map.set_mapping(7, ParamTarget::AttackTime);
map.set_mapping(64, ParamTarget::ReleaseTime);

if let Some((target, value)) = map.map_cc(7, 100) {
    // target = AttackTime, value ≈ 0.787 (100/127)
}
```
</details>

### MidiToAudioBridge — Full Integrated Pipeline

The `MidiToAudioBridge` connects a MIDI device to a kernel runtime, enabling live polyphonic playback with automatic note routing, CC-driven filter control, and parameter smoothing.

See [`examples/microfreak_synth.rs`](examples/microfreak_synth.rs) for the reference live implementation — a fully-featured hardware ROMpler built with:

- `MidiInputHandler` for USB MIDI from an Arturia MicroFreak
- `build_rompler_graph` for the per-voice Sampler → Multiply ← Envelope topology
- `StreamController::play_handle` for live CPAL audio output
- Per-slot release tracking (250 ms guard against retrigger click)
- CC74/CC71 → filter cutoff/resonance
- Full pitch-bend retuning (±2 semitones, per-voice)
- Periodic `diagnostics()` logging with glitch detection

Run it: `cargo run --release --example microfreak_synth` (requires a MIDI controller).

### Transport MIDI Clock

```rust
use auxide_midi::midi_input::Transport;

let mut transport = Transport::new();
transport.start();
// Feed MIDI Clock messages (24 PPQ)
transport.tick();
println!("beat={}, bar={}, phase={}", transport.beat(), transport.bar(), transport.ppq_phase());
```

### Parameter Smoothing

Avoid clicks from abrupt parameter changes:

```rust
use auxide_midi::ParamSmoother;

let mut smooth = ParamSmoother::new(); // 10 ms time constant @ 44.1k
smooth.set_target(0.5);
for _ in 0..441 {
    let v = smooth.next_sample(); // approaches 0.5 over ~10 ms
}
```

## Features

- **MIDI Input Handler**: Connect to MIDI devices, receive events in real-time
- **Voice Allocator**: Manage polyphonic voices with intelligent note stealing
- **CC Mapping**: Map MIDI CC messages to DSP parameters
- **Parameter Smoothing**: Smooth parameter changes to avoid clicks/pops
- **RT-Safe**: Zero allocations in audio processing paths

## Community & Support

• 🐛 Bug Reports: [GitHub Issues](https://github.com/Michael-A-Kuykendall/auxide-midi/issues)
• 💬 Discussions: [GitHub Discussions](https://github.com/Michael-A-Kuykendall/auxide-midi/discussions)
• 📖 Documentation: [docs.rs](https://docs.rs/auxide-midi)
• 💝 Sponsorship: [GitHub Sponsors](https://github.com/sponsors/Michael-A-Kuykendall)
• 🤝 Contributing: [CONTRIBUTING.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/CONTRIBUTING.md)
• 🔒 Governance: [GOVERNANCE.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/GOVERNANCE.md)
• 🔐 Security: [SECURITY.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/SECURITY.md)

## License & Philosophy

MIT License - forever and always.

**Philosophy**: MIDI infrastructure should be invisible. Auxide is infrastructure.

**Testing Philosophy**: Reliability through comprehensive validation.

**Forever maintainer**: Michael A. Kuykendall
**Promise**: This will never become a paid product
**Mission**: Making real-time MIDI integration simple and reliable

## Auxide Ecosystem
| Crate | Description | Version |
|-------|-------------|---------|
| [auxide](https://github.com/Michael-A-Kuykendall/auxide) | Real-time-safe audio graph kernel | 0.3.2 |
| [auxide-dsp](https://github.com/Michael-A-Kuykendall/auxide-dsp) | DSP nodes library | 0.2.1 |
| [auxide-io](https://github.com/Michael-A-Kuykendall/auxide-io) | Audio I/O layer | 0.1.3 |
| **[auxide-midi](https://github.com/Michael-A-Kuykendall/auxide-midi)** | MIDI integration | 0.1.2 |
