//! Integration tests for auxide-midi

use auxide_midi::{MidiInputHandler, MidiEvent, VoiceAllocator, CCMap, ParamTarget};

#[test]
fn midi_to_voice_allocation_integration() {
    // Test that MIDI events properly trigger voice allocation
    let mut voice_allocator = VoiceAllocator::new();

    // Simulate Note On event
    let note_on = MidiEvent::NoteOn(60, 100);
    match note_on {
        MidiEvent::NoteOn(note, vel) => {
            let voice_id = voice_allocator.allocate_voice(note).unwrap();
            assert_eq!(voice_id.0, 0);
            assert_eq!(voice_allocator.active_voice_count(), 1);
        }
        _ => panic!("Expected NoteOn"),
    }

    // Simulate Note Off event
    let note_off = MidiEvent::NoteOff(60, 64);
    match note_off {
        MidiEvent::NoteOff(note, _) => {
            voice_allocator.release_voice(note);
            assert_eq!(voice_allocator.active_voice_count(), 0);
        }
        _ => panic!("Expected NoteOff"),
    }
}

#[test]
fn cc_mapping_integration() {
    // Test CC mapping integration
    let cc_map = CCMap::new();

    // Simulate CC 1 (mod wheel) -> FilterCutoff
    let cc_event = MidiEvent::ControlChange(1, 64);
    match cc_event {
        MidiEvent::ControlChange(cc_num, value) => {
            let mapping = cc_map.map_cc(cc_num, value);
            assert_eq!(mapping, Some((ParamTarget::FilterCutoff, 64.0 / 127.0)));
        }
        _ => panic!("Expected ControlChange"),
    }

    // Simulate unmapped CC
    let unmapped_cc = MidiEvent::ControlChange(42, 100);
    match unmapped_cc {
        MidiEvent::ControlChange(cc_num, value) => {
            let mapping = cc_map.map_cc(cc_num, value);
            assert_eq!(mapping, None);
        }
        _ => panic!("Expected ControlChange"),
    }
}

#[test]
fn voice_stealing_integration() {
    // Test voice stealing when all voices are busy
    let mut voice_allocator = VoiceAllocator::new();

    // Fill all 8 voices
    for i in 0..8 {
        let voice_id = voice_allocator.allocate_voice(60 + i as u8).unwrap();
        assert_eq!(voice_id.0, i);
    }
    assert_eq!(voice_allocator.active_voice_count(), 8);

    // Try to allocate 9th voice - should steal oldest
    let stolen_voice = voice_allocator.allocate_voice(100).unwrap();
    assert_eq!(stolen_voice.0, 0); // Should steal voice 0
    assert_eq!(voice_allocator.active_voice_count(), 8);
}

#[test]
fn midi_parser_integration() {
    // Test that raw MIDI bytes are parsed correctly
    let test_cases = vec![
        ([0x90, 60, 100], Some(MidiEvent::NoteOn(60, 100))),
        ([0x80, 64, 0], Some(MidiEvent::NoteOff(64, 0))),
        ([0xB0, 74, 127], Some(MidiEvent::ControlChange(74, 127))),
        ([0xE0, 0x00, 0x40], Some(MidiEvent::PitchBend(8192))),
        ([0xFF, 0xFF, 0xFF], None), // Invalid
    ];

    for (bytes, expected) in test_cases {
        let result = MidiInputHandler::parse_message(&bytes);
        assert_eq!(result, expected, "Failed for bytes: {:?}", bytes);
    }
}

#[test]
fn polyphonic_voice_management() {
    // Test managing multiple simultaneous voices
    let mut voice_allocator = VoiceAllocator::new();

    // Play a chord: C4, E4, G4
    let notes = [60, 64, 67];
    let mut voice_ids = Vec::new();

    for &note in &notes {
        let voice_id = voice_allocator.allocate_voice(note).unwrap();
        voice_ids.push(voice_id);
    }

    assert_eq!(voice_allocator.active_voice_count(), 3);

    // Release middle note
    voice_allocator.release_voice(64);
    assert_eq!(voice_allocator.active_voice_count(), 2);

    // Play another note - should reuse released voice
    let new_voice = voice_allocator.allocate_voice(72).unwrap();
    assert_eq!(voice_allocator.active_voice_count(), 3);

    // Verify active voices
    let active_notes: Vec<_> = voice_allocator.active_voices()
        .map(|(_, note)| note)
        .collect();
    assert_eq!(active_notes.len(), 3);
    assert!(active_notes.contains(&60)); // C4 still active
    assert!(active_notes.contains(&67)); // G4 still active
    assert!(active_notes.contains(&72)); // C5 now active
    assert!(!active_notes.contains(&64)); // E4 released
}

#[test]
fn cc_parameter_range_validation() {
    // Test that CC values are properly normalized
    let cc_map = CCMap::new();

    let test_values = [0, 32, 63, 64, 95, 127];

    for &cc_value in &test_values {
        let mapping = cc_map.map_cc(1, cc_value);
        if let Some((target, normalized)) = mapping {
            assert_eq!(target, ParamTarget::FilterCutoff);
            assert!(normalized >= 0.0 && normalized <= 1.0);
            assert!((normalized - (cc_value as f32 / 127.0)).abs() < 0.001);
        } else {
            panic!("Expected mapping for CC 1");
        }
    }
}