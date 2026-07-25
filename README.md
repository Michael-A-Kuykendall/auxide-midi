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

This crate provides real-time MIDI input handling and voice allocation for polyphonic synthesis. It integrates with auxide-dsp nodes but requires auxide kernel updates for full dynamic parameter control.

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

```rust
use auxide_midi::{MidiInputHandler, VoiceAllocator, MidiEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // List available MIDI devices
    let devices = MidiInputHandler::list_devices()?;
    
    if devices.is_empty() {
        println!("No MIDI devices found");
        return Ok(());
    }
    
    // Create voice allocator for polyphonic synthesis
    let mut voice_allocator = VoiceAllocator::new();
    
    // Create MIDI input handler
    let mut midi_handler = MidiInputHandler::new();
    
    // Connect to first device
    midi_handler.connect_device(0)?;
    
    // Process MIDI events
    while let Some(event) = midi_handler.try_recv() {
        match event {
            MidiEvent::NoteOn(note, velocity) => {
                if let Some(voice_id) = voice_allocator.allocate_voice(note) {
                    // Trigger synth voice with note/velocity
                    println!("Note on: {} vel: {}", note, velocity);
                }
            }
            MidiEvent::NoteOff(note, _) => {
                voice_allocator.release_voice(note);
                println!("Note off: {}", note);
            }
            MidiEvent::ControlChange(cc, value) => {
                // Map CC to parameters
                println!("CC {}: {}", cc, value);
            }
        }
    }
    
    Ok(())
}
```

See `examples/` for complete working synthesizers.

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
• 📜 Governance: [GOVERNANCE.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/GOVERNANCE.md)
• 🔒 Security: [SECURITY.md](https://github.com/Michael-A-Kuykendall/auxide-midi/blob/main/SECURITY.md)

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
| [auxide-dsp](https://github.com/Michael-A-Kuykendall/auxide-dsp) | DSP nodes library | 0.1.1 |
| [auxide-io](https://github.com/Michael-A-Kuykendall/auxide-io) | Audio I/O layer | 0.1.2 |
| **[auxide-midi](https://github.com/Michael-A-Kuykendall/auxide-midi)** | MIDI integration | 0.1.1 |
