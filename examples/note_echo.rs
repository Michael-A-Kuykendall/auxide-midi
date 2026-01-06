//! Echo MIDI note events to console

use auxide_midi::MidiInputHandler;
use std::io::{self, Write};

fn main() -> anyhow::Result<()> {
    println!("MIDI Note Echo");
    println!("==============");
    println!();

    let devices = MidiInputHandler::list_devices()?;

    if devices.is_empty() {
        println!("No MIDI input devices found.");
        return Ok(());
    }

    // Auto-select MicroFreak or Arturia devices
    let mut selected_index = None;
    for (i, device) in devices.iter().enumerate() {
        if device.to_lowercase().contains("microfreak") || device.to_lowercase().contains("arturia")
        {
            selected_index = Some(i);
            break;
        }
    }

    if selected_index.is_none() {
        println!("Available devices:");
        for (i, device) in devices.iter().enumerate() {
            println!("{}: {}", i, device);
        }
        print!("Select device (0-{}): ", devices.len() - 1);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        selected_index = input.trim().parse().ok();
    }

    let device_index = match selected_index {
        Some(idx) if idx < devices.len() => idx,
        _ => {
            println!("Invalid device selection");
            return Ok(());
        }
    };

    println!("Connecting to: {}", devices[device_index]);

    let mut midi_handler = MidiInputHandler::new();
    midi_handler.connect_device(device_index)?;

    println!("Listening for MIDI events... (Ctrl+C to exit)");
    println!();

    loop {
        if let Some(event) = midi_handler.try_recv() {
            match event {
                auxide_midi::MidiEvent::NoteOn(note, vel) => {
                    let note_name = note_to_name(note);
                    println!("NoteOn: {} ({}) velocity {}", note_name, note, vel);
                }
                auxide_midi::MidiEvent::NoteOff(note, vel) => {
                    let note_name = note_to_name(note);
                    println!("NoteOff: {} ({}) velocity {}", note_name, note, vel);
                }
                auxide_midi::MidiEvent::ControlChange(cc, val) => {
                    println!("CC {}: {}", cc, val);
                }
                auxide_midi::MidiEvent::PitchBend(bend) => {
                    println!("PitchBend: {}", bend);
                }
            }
        }

        // Small sleep to prevent busy waiting
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn note_to_name(note: u8) -> String {
    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (note / 12) as i32 - 1;
    let note_in_octave = (note % 12) as usize;
    format!("{}{}", note_names[note_in_octave], octave)
}
