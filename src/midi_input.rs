//! MIDI input handling with midir backend.
//!
//! Provides event-driven MIDI input with non-blocking access to incoming messages.

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use midir::{MidiInput, MidiInputConnection};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// MIDI events received from input devices.
#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    NoteOn(u8, u8),        // note, velocity
    NoteOff(u8, u8),       // note, velocity
    ControlChange(u8, u8), // cc_num, value
    PitchBend(i16),        // bend value
    Clock,                 // 0xF8 real-time timing clock (24 PPQ)
    Start,                 // 0xFA transport start
    Continue,              // 0xFB transport continue
    Stop,                  // 0xFC transport stop
    SongPosition(u16),     // 0xF2 SPP: 16-bit LSB-first position
}

/// Manages MIDI input from devices with non-blocking event queuing.
pub struct MidiInputHandler {
    connection: Option<MidiInputConnection<()>>,
    event_sender: Sender<MidiEvent>,
    event_receiver: Receiver<MidiEvent>,
    running: Arc<AtomicBool>,
}

impl MidiInputHandler {
    /// Creates a new MIDI input handler with a bounded event queue.
    pub fn new() -> Self {
        let (sender, receiver) = bounded(256); // Bounded queue to prevent unbounded growth
        Self {
            connection: None,
            event_sender: sender,
            event_receiver: receiver,
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Lists all available MIDI input devices.
    pub fn list_devices() -> Result<Vec<String>> {
        let midi_in = MidiInput::new("auxide-midi")?;
        Ok(midi_in
            .ports()
            .into_iter()
            .filter_map(|port| midi_in.port_name(&port).ok())
            .collect())
    }

    /// Connects to a MIDI device by index from the device list.
    pub fn connect_device(&mut self, index: usize) -> Result<()> {
        let midi_in = MidiInput::new("auxide-midi")?;
        let ports = midi_in.ports();

        if index >= ports.len() {
            return Err(anyhow::anyhow!("Device index {} out of range", index));
        }

        let port = &ports[index];
        let running = self.running.clone();
        let sender = self.event_sender.clone();

        let connection = midi_in
            .connect(
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
            )
            .map_err(|e| anyhow::anyhow!("MIDI connect error: {:?}", e))?;

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

        // System real-time / common messages (0xF0..=0xFF) carry no channel.
        match status {
            0xF8 => return Some(MidiEvent::Clock),
            0xFA => return Some(MidiEvent::Start),
            0xFB => return Some(MidiEvent::Continue),
            0xFC => return Some(MidiEvent::Stop),
            0xF2 => {
                // Song Position Pointer: 2 data bytes (LSB, MSB) as a
                // 16-bit LSB-first value.
                if bytes.len() >= 3 {
                    let lsb = bytes[1] as u16;
                    let msb = bytes[2] as u16;
                    return Some(MidiEvent::SongPosition(lsb | (msb << 8)));
                }
                return None;
            }
            _ => {}
        }

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

/// MIDI transport state derived from clock / transport messages.
///
/// `clocks` counts received 24-PPQ timing-clock ticks. With a 4/4
/// meter, `beat = clocks / 24` and `bar = clocks / 96`.
/// `Start` resets the position to 0; `Stop` holds it; `Continue`
/// resumes from the held position.
#[derive(Debug, Clone, Copy)]
pub struct Transport {
    clocks: u64,
    running: bool,
}

impl Transport {
    /// A stopped transport at position 0.
    pub fn new() -> Self {
        Self {
            clocks: 0,
            running: false,
        }
    }

    /// Transport start: reset position and begin running.
    pub fn start(&mut self) {
        self.clocks = 0;
        self.running = true;
    }

    /// Transport stop: hold the current position, stop running.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Transport continue: resume running from the held position.
    pub fn cont(&mut self) {
        self.running = true;
    }

    /// Advance one timing-clock tick.
    pub fn tick(&mut self) {
        self.clocks = self.clocks.saturating_add(1);
    }

    /// Set the position from a Song Position Pointer value (in 16th notes).
    ///
    /// 1 quarter note = 24 clocks = 4 16th notes, so the value is
    /// scaled by 6 to keep `beat`/`bar` consistent with clock ticks.
    pub fn song_position(&mut self, sixteenths: u16) {
        self.clocks = (sixteenths as u64) * 6;
    }

    /// Current beat (quarter notes) within the bar.
    pub fn beat(&self) -> u64 {
        self.clocks / 24
    }

    /// Current bar (groups of 4 beats).
    pub fn bar(&self) -> u64 {
        self.clocks / 96
    }

    /// Phase within the current beat (0..24 ticks).
    pub fn ppq_phase(&self) -> u64 {
        self.clocks % 24
    }

    /// Whether the transport is currently running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Apply a parsed MIDI event to the transport state.
    pub fn update(&mut self, ev: &MidiEvent) {
        match ev {
            MidiEvent::Start => self.start(),
            MidiEvent::Stop => self.stop(),
            MidiEvent::Continue => self.cont(),
            MidiEvent::Clock => self.tick(),
            MidiEvent::SongPosition(p) => self.song_position(*p),
            _ => {}
        }
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn parse_transport() {
        // Transport + real-time clock + SPP parse to distinct events.
        assert_eq!(
            MidiInputHandler::parse_message(&[0xFA]),
            Some(MidiEvent::Start)
        );
        assert_eq!(
            MidiInputHandler::parse_message(&[0xF8]),
            Some(MidiEvent::Clock)
        );
        assert_eq!(
            MidiInputHandler::parse_message(&[0xFB]),
            Some(MidiEvent::Continue)
        );
        assert_eq!(
            MidiInputHandler::parse_message(&[0xFC]),
            Some(MidiEvent::Stop)
        );
        // SPP: bytes 0x08 (LSB), 0x01 (MSB) -> 16-bit LSB-first = 0x108.
        assert_eq!(
            MidiInputHandler::parse_message(&[0xF2, 0x08, 0x01]),
            Some(MidiEvent::SongPosition(0x108))
        );
    }

    #[test]
    fn transport_counts_clocks_and_bars() {
        let mut t = Transport::new();
        t.start();
        for _ in 0..96 {
            t.tick();
        }
        assert_eq!(t.beat(), 4, "96 clocks = 4 beats");
        assert_eq!(t.bar(), 1, "96 clocks = 1 bar (4/4)");
        assert!(t.is_running());
    }

    #[test]
    fn transport_start_resets_stop_holds_continue_resumes() {
        let mut t = Transport::new();
        t.start();
        t.tick();
        t.tick();
        assert!(t.is_running());
        assert_eq!(t.clocks, 2);

        t.stop();
        assert!(!t.is_running());
        assert_eq!(t.clocks, 2, "stop must hold position");

        t.cont();
        assert!(t.is_running(), "continue resumes");

        let mut t2 = Transport::new();
        t2.start();
        assert_eq!(t2.clocks, 0, "start resets position");
    }

    #[test]
    fn transport_update_from_events() {
        let mut t = Transport::new();
        t.update(&MidiEvent::Start);
        t.update(&MidiEvent::Clock);
        t.update(&MidiEvent::Clock);
        t.update(&MidiEvent::Clock);
        assert_eq!(t.clocks, 3);
        assert!(t.is_running());

        t.update(&MidiEvent::Stop);
        assert!(!t.is_running());
        assert_eq!(t.clocks, 3, "stop holds");

        t.update(&MidiEvent::SongPosition(0x108));
        assert_eq!(t.clocks, 0x108_u64 * 6, "SPP sets position");
    }
}
