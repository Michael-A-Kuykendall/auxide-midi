# auxide-midi

<img src="https://raw.githubusercontent.com/Michael-A-Kuykendall/auxide-midi/main/assets/auxide-midi-logo.png" alt="auxide-midi logo" width="400"/>

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

## Example

Build a polyphonic ROMpler with the real `Synth` facade — every note is
routed through the auxide kernel's runtime control plane (no "all notes
play at 440 Hz" overclaim):

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

**True capability:** polyphonic voices with per-note pitch, ADSR envelopes,
and an SVF filter; real-time CC mapping (cutoff/resonance) and
pitch-bend through the lock-free control queue. A MIDI device is
required for input, but the synth itself renders with no device. For live
CPAL playback, wrap the `RuntimeHandle` with
`auxide_io::StreamController::play_handle`.

See `examples/` for complete, building demos
(`cargo run --example poly_synth`, `cargo run --example note_echo`).

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
| [auxide](https://github.com/Michael-A-Kuykendall/auxide) | Real-time-safe audio graph kernel | 0.3.1 |
| [auxide-dsp](https://github.com/Michael-A-Kuykendall/auxide-dsp) | DSP nodes library | 0.2.0 |
| [auxide-io](https://github.com/Michael-A-Kuykendall/auxide-io) | Audio I/O layer | 0.1.2 |
| **[auxide-midi](https://github.com/Michael-A-Kuykendall/auxide-midi)** | MIDI integration | 0.1.1 |
