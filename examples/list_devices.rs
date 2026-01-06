//! List available MIDI input devices

use auxide_midi::MidiInputHandler;

fn main() -> anyhow::Result<()> {
    println!("Available MIDI input devices:");
    println!("----------------------------");

    let devices = MidiInputHandler::list_devices()?;

    if devices.is_empty() {
        println!("No MIDI input devices found.");
        println!();
        println!("Troubleshooting:");
        println!("1. Make sure your MIDI keyboard is connected and powered on");
        println!("2. Try a different USB port");
        println!("3. On Windows: Check Device Manager for MIDI devices");
        println!("4. On Mac: Check Audio MIDI Setup");
        println!("5. On Linux: Run 'amidi -l' to list devices");
        return Ok(());
    }

    for (i, device) in devices.iter().enumerate() {
        println!("{}: {}", i, device);
    }

    println!();
    println!("Use device index with other examples:");
    println!(
        "cargo run --example poly_synth -- {}",
        if devices.len() > 0 { 0 } else { 0 }
    );

    Ok(())
}
