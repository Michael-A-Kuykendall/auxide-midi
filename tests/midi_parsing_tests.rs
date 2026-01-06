//! Tests for MIDI message parsing

use auxide_midi::{MidiEvent, MidiInputHandler};

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
fn midi_bytes_pitch_bend_max() {
    let bytes = [0xE0, 0x7F, 0x7F]; // Pitch bend, maximum
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, Some(MidiEvent::PitchBend(16383)));
}

#[test]
fn midi_bytes_pitch_bend_min() {
    let bytes = [0xE0, 0x00, 0x00]; // Pitch bend, minimum
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, Some(MidiEvent::PitchBend(0)));
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
fn short_messages_ignored() {
    let bytes = [0x90, 60]; // Incomplete note on
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, None);
}

#[test]
fn empty_message_ignored() {
    let bytes = [];
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, None);
}

#[test]
fn system_messages_ignored() {
    let bytes = [0xF0, 0x01, 0x02]; // System exclusive
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, None);
}

#[test]
fn program_change_ignored() {
    let bytes = [0xC0, 42]; // Program change
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, None);
}

#[test]
fn aftertouch_ignored() {
    let bytes = [0xD0, 100]; // Channel aftertouch
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, None);
}

#[test]
fn polyphonic_aftertouch_ignored() {
    let bytes = [0xA0, 60, 100]; // Polyphonic aftertouch
    let event = MidiInputHandler::parse_message(&bytes);
    assert_eq!(event, None);
}
