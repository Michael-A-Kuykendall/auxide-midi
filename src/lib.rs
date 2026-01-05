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
//! ```rust
//! use auxide_midi::{MidiInputHandler, VoiceAllocator, MidiEvent};
//!
//! // List available MIDI devices
//! let devices = MidiInputHandler::list_devices();
//!
//! // Create voice allocator
//! let mut voice_allocator = VoiceAllocator::new();
//!
//! // Create MIDI input handler
//! let mut midi_handler = MidiInputHandler::new();
//!
//! // Connect to first device if available
//! if !devices.is_empty() {
//!     midi_handler.connect_device(0).unwrap();
//!
//!     // Process MIDI events
//!     while let Some(event) = midi_handler.try_recv() {
//!         match event {
//!             MidiEvent::NoteOn(note, vel) => {
//!                 if let Some(voice_id) = voice_allocator.allocate_voice(note) {
//!                     // Trigger voice
//!                 }
//!             }
//!             MidiEvent::NoteOff(note, _) => {
//!                 voice_allocator.release_voice(note);
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```

#![forbid(unsafe_code)]

pub mod conversions;
pub mod voice_allocator;
pub mod midi_input;
pub mod cc_mapping;
pub mod smoother;
pub mod voice_state;

pub use conversions::*;
pub use voice_allocator::*;
pub use midi_input::*;
pub use cc_mapping::*;
pub use smoother::*;
pub use voice_state::*;