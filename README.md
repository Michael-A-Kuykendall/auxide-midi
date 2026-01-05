# auxide-midi

<img src="assets/auxide-midi-logo.png" alt="auxide-midi logo" width="200"/>

MIDI input integration and polyphonic synthesizer for Auxide DSP graphs.

This crate provides real-time MIDI input handling, voice allocation for polyphonic synthesis, and seamless integration with auxide-dsp nodes.

## Features

- Real-time MIDI input with midir
- 8-voice polyphonic voice allocation with stealing
- MIDI CC parameter mapping
- Parameter smoothing to prevent zipper noise
- Lock-free communication between MIDI and audio threads
- MicroFreak keyboard support

## Connecting Your MIDI Keyboard

### Windows
1. Plug in your MIDI keyboard (USB or traditional MIDI interface)
2. Windows should automatically install drivers - no additional setup needed
3. The device will appear as a MIDI input device

### Mac
1. Connect your MIDI keyboard
2. macOS should recognize it automatically
3. Check Audio MIDI Setup if needed

### Linux
1. Connect your MIDI keyboard
2. May need to install ALSA tools: `sudo apt install alsa-utils`
3. Check with `amidi -l` to see available devices

## Running the Demo

```bash
# List available MIDI devices
cargo run --example list_devices

# Run the polyphonic synthesizer
cargo run --example poly_synth
```

The synthesizer will automatically try to connect to devices with "MicroFreak" or "Arturia" in the name, otherwise it will prompt you to select a device.

## Troubleshooting

### No MIDI devices found
- Check that your keyboard is properly connected and powered on
- Try a different USB port
- On Windows: Check Device Manager for MIDI devices
- On Mac: Check Audio MIDI Setup
- On Linux: Run `amidi -l` to list devices

### Audio crackles or dropouts
- Try reducing the buffer size in your audio settings
- Close other audio applications
- Check CPU usage

### MIDI not responding
- Verify the correct device is selected
- Check MIDI channel settings on your keyboard
- Try a different MIDI cable or USB port

## Architecture

The crate uses a lock-free architecture:
- MIDI thread receives events and sends them via crossbeam-channel
- Audio thread processes events without blocking
- Voice allocation uses fixed-size arrays for RT-safety
- Parameter updates use smoothing to prevent artifacts