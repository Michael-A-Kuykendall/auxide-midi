//! MIDI input handling with midir

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use midir::{MidiInput, MidiInputConnection};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    NoteOn(u8, u8),        // note, velocity
    NoteOff(u8, u8),       // note, velocity
    ControlChange(u8, u8), // cc_num, value
    PitchBend(i16),        // bend value
}

pub struct MidiInputHandler {
    connection: Option<MidiInputConnection<()>>,
    event_sender: Sender<MidiEvent>,
    event_receiver: Receiver<MidiEvent>,
    running: Arc<AtomicBool>,
}

impl MidiInputHandler {
    pub fn new() -> Self {
        let (sender, receiver) = bounded(256); // Bounded queue to prevent unbounded growth
        Self {
            connection: None,
            event_sender: sender,
            event_receiver: receiver,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn list_devices() -> Result<Vec<String>> {
        let midi_in = MidiInput::new("auxide-midi")?;
        Ok(midi_in
            .ports()
            .into_iter()
            .filter_map(|port| midi_in.port_name(&port).ok())
            .collect())
    }

    pub fn connect_device(&mut self, index: usize) -> Result<()> {
        let midi_in = MidiInput::new("auxide-midi")?;
        let ports = midi_in.ports();

        if index >= ports.len() {
            return Err(anyhow::anyhow!("Device index {} out of range", index));
        }

        let port = &ports[index];
        let running = self.running.clone();
        let sender = self.event_sender.clone();

        let connection = midi_in.connect(
            port,
            "auxide-midi-input",
            move |_, message, _| {
                if !running.load(Ordering::Relaxed) {
                    return;
                }

                if let Some(event) = Self::parse_message(message) {
                    // Non-blocking send - drop message if queue is full
                    let _ = sender.try_send(event);
                }
            },
            (),
        )?;

        self.connection = Some(connection);
        Ok(())
    }

    pub fn try_recv(&self) -> Option<MidiEvent> {
        self.event_receiver.try_recv().ok()
    }

    pub fn disconnect(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(_connection) = self.connection.take() {
            // Connection will be dropped, closing the MIDI port
        }
    }

    pub fn parse_message(bytes: &[u8]) -> Option<MidiEvent> {
        if bytes.is_empty() {
            return None;
        }

        let status = bytes[0];

        match status & 0xF0 {
            0x90 => {
                // Note On
                if bytes.len() >= 3 && bytes[2] > 0 {
                    Some(MidiEvent::NoteOn(bytes[1], bytes[2]))
                } else if bytes.len() >= 3 {
                    // Note On with velocity 0 is Note Off
                    Some(MidiEvent::NoteOff(bytes[1], bytes[2]))
                } else {
                    None
                }
            }
            0x80 => {
                // Note Off
                if bytes.len() >= 3 {
                    Some(MidiEvent::NoteOff(bytes[1], bytes[2]))
                } else {
                    None
                }
            }
            0xB0 => {
                // Control Change
                if bytes.len() >= 3 {
                    Some(MidiEvent::ControlChange(bytes[1], bytes[2]))
                } else {
                    None
                }
            }
            0xE0 => {
                // Pitch Bend
                if bytes.len() >= 3 {
                    let bend = ((bytes[2] as i16) << 7) | (bytes[1] as i16);
                    Some(MidiEvent::PitchBend(bend))
                } else {
                    None
                }
            }
            _ => None, // Ignore other message types for now
        }
    }
}

impl Default for MidiInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MidiInputHandler {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_bytes_to_note_on() {
        let bytes = [0x90, 60, 100]; // Note On, C4, velocity 100
        let event = MidiInputHandler::parse_message(&bytes);
        assert_eq!(event, Some(MidiEvent::NoteOn(60, 100)));
    }

    #[test]
    fn midi_bytes_to_note_off() {
        let bytes = [0x80, 60, 64]; // Note Off, C4, velocity 64
        let event = MidiInputHandler::parse_message(&bytes);
        assert_eq!(event, Some(MidiEvent::NoteOff(60, 64)));
    }

    #[test]
    fn midi_bytes_to_cc() {
        let bytes = [0xB0, 74, 127]; // CC, number 74, value 127
        let event = MidiInputHandler::parse_message(&bytes);
        assert_eq!(event, Some(MidiEvent::ControlChange(74, 127)));
    }

    #[test]
    fn midi_bytes_pitch_bend() {
        let bytes = [0xE0, 0x00, 0x40]; // Pitch bend, center position
        let event = MidiInputHandler::parse_message(&bytes);
        assert_eq!(event, Some(MidiEvent::PitchBend(8192)));
    }

    #[test]
    fn garbage_bytes_none() {
        let bytes = [0xFF, 0xFF, 0xFF]; // Invalid MIDI
        let event = MidiInputHandler::parse_message(&bytes);
        assert_eq!(event, None);
    }

    #[test]
    fn note_on_velocity_zero_is_note_off() {
        let bytes = [0x90, 60, 0]; // Note On with velocity 0
        let event = MidiInputHandler::parse_message(&bytes);
        assert_eq!(event, Some(MidiEvent::NoteOff(60, 0)));
    }
}
