//! # Auxide MIDI
//!
//! MIDI input integration and polyphonic synthesizer for Auxide DSP graphs.
//!
//! This crate provides:
//! - MIDI input handling with midir
//! - Voice allocation and management for polyphonic synthesis
//! - Real-time-safe parameter updates
//! - Integration with auxide-dsp nodes
//!
//! ## Example
//!
//! Build a polyphonic ROMpler with the real `Synth` facade. Every note is
//! routed through the auxide kernel's runtime control plane — no "all notes
//! play at 440 Hz" overclaim:
//!
//! ```rust
//! use std::sync::Arc;
//! use auxide_midi::Synth;
//!
//! let sr = 44100.0;
//! let sample: Arc<Vec<f32>> = Arc::new(
//!     (0..44100)
//!         .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin())
//!         .collect(),
//! );
//! let mut synth = Synth::new(sample, sr, 8, 69); // 8-voice polyphonic ROMpler
//! let mut out = vec![0.0f32; 64];
//! synth.note_on(69, 100);
//! for _ in 0..10 {
//!     synth.process_block(&mut out).unwrap();
//! }
//! assert!(out.iter().any(|&s| s.abs() > 1e-3), "synth must produce audio");
//! synth.note_off(69);
//! ```

#![forbid(unsafe_code)]

pub mod cc_mapping;
pub mod conversions;
pub mod midi_bridge;
pub mod midi_input;
pub mod smoother;
pub mod synth;
pub mod voice_allocator;
pub mod voice_state;

pub use cc_mapping::*;
pub use conversions::*;
pub use midi_bridge::*;
pub use midi_input::*;
pub use smoother::*;
pub use synth::*;
pub use voice_allocator::*;
pub use voice_state::*;
