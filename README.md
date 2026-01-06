# auxide-midi

<img src="https://raw.githubusercontent.com/Michael-A-Kuykendall/auxide-midi/main/assets/auxide-midi-logo.png" alt="auxide-midi logo" width="200"/>

[![Crates.io](https://img.shields.io/crates/v/auxide-midi.svg)](https://crates.io/crates/auxide-midi)
[![Documentation](https://docs.rs/auxide-midi/badge.svg)](https://docs.rs/auxide-midi)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
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

This crate provides real-time MIDI input handling and voice allocation for polyphonic synthesis. It integrates with auxide-dsp nodes but requires auxide kernel updates for full dynamic parameter control.

## Auxide Ecosystem
| Crate | Description | Version |
|-------|-------------|---------|
| [auxide](https://github.com/Michael-A-Kuykendall/auxide) | Real-time-safe audio graph kernel | 0.3.0 |
| **[auxide-dsp](https://github.com/Michael-A-Kuykendall/auxide-dsp)** | DSP nodes library | 0.2.0 |
| [auxide-io](https://github.com/Michael-A-Kuykendall/auxide-io) | Audio I/O layer | 0.2.0 |
| **[auxide-midi](https://github.com/Michael-A-Kuykendall/auxide-midi)** | MIDI integration | 0.2.0 |

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
auxide = "0.3"
auxide-dsp = "0.2"
auxide-io = "0.2"
auxide-midi = "0.2"
```

## Example

```rust
use auxide_midi::midi_input::MidiInput;
use auxide_midi::voice_allocator::VoiceAllocator;

// Set up MIDI input
let midi_input = MidiInput::new()?;
midi_input.start()?;

// Create voice allocator for polyphonic synthesis
let mut allocator = VoiceAllocator::new(8); // 8 voices

// Process MIDI messages
while let Some(msg) = midi_input.recv() {
    allocator.process_midi(msg);
}
```

See `examples/` for more usage.

## Community & Support

• 🐛 Bug Reports: [GitHub Issues](https://github.com/Michael-A-Kuykendall/auxide-midi/issues)
• 💬 Discussions: [GitHub Discussions](https://github.com/Michael-A-Kuykendall/auxide-midi/discussions)
• 📖 Documentation: [docs/](https://github.com/Michael-A-Kuykendall/auxide-midi/tree/main/docs)
• 💝 Sponsorship: [GitHub Sponsors](https://github.com/sponsors/Michael-A-Kuykendall)
• 🤝 Contributing: [CONTRIBUTING.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/CONTRIBUTING.md)
• 📜 Governance: [GOVERNANCE.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/GOVERNANCE.md)
• 🔒 Security: [SECURITY.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/SECURITY.md)

## License & Philosophy

MIT License - forever and always.

**Philosophy**: MIDI integration should be invisible. Auxide is infrastructure.

**Testing Philosophy**: Reliability through comprehensive validation and property-based testing.

**Forever maintainer**: Michael A. Kuykendall  
**Promise**: This will never become a paid product  
**Mission**: Making real-time MIDI processing simple and reliable
