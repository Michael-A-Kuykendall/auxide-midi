//! Live ROMpler — drives a polyphonic ROMpler from a MIDI device (Arturia
//! MicroFreak or any input) in real time and plays it through the system
//! sound device via `auxide_io::StreamController`.
//!
//! This is the live counterpart to `rompler_demo.rs` (which renders offline to
//! a .wav). It requires a MIDI input device AND a sound output device. The
//! default output device is used; pass an index via `--device N` to choose a
//! specific one (see `auxide_io::StreamController::play_handle_on_device`).
//!
//! Build: `cargo build --example live_rompler`
//! Run:   `cargo run --example live_rompler`   (Ctrl+C to stop)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use auxide::control::ControlMsg;
use auxide::rt::RuntimeCore;
use auxide_io::StreamController;
use auxide_midi::midi_bridge::build_rompler_graph;
use auxide_midi::{MidiEvent, MidiInputHandler};

const VOICES: usize = 8;

/// A one-second 440 Hz sine used as the ROMpler sample.
fn make_sample(sr: f32) -> Arc<Vec<f32>> {
    Arc::new(
        (0..sr as usize)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin())
            .collect(),
    )
}

/// MIDI note number -> frequency in Hz (A4 = 69 = 440 Hz).
fn note_to_freq(n: u8) -> f32 {
    440.0 * f32::powf(2.0, (n as f32 - 69.0) / 12.0)
}

fn main() -> Result<()> {
    let sr = 44100.0;
    let sample = make_sample(sr);

    // ------------------------------------------------------------------
    // Build the polyphonic ROMpler graph and a RuntimeHandle we can stream.
    // ------------------------------------------------------------------
    let (_graph, plan, voice_pairs, _filter_node) = build_rompler_graph(VOICES, sample, sr, 69);
    let (handle, mut control) = RuntimeCore::new_with_channels(plan, &_graph, sr);

    // ------------------------------------------------------------------
    // Stream the ROMpler to the system sound device.
    // ------------------------------------------------------------------
    let controller = StreamController::play_handle(handle)?;
    controller.start()?;
    println!("Playing through the default sound device (Ctrl+C to stop).");

    // ------------------------------------------------------------------
    // Open the MIDI input device (prefer a MicroFreak / Arturia).
    // ------------------------------------------------------------------
    let devices = MidiInputHandler::list_devices()?;
    if devices.is_empty() {
        eprintln!("No MIDI input devices found. Connect a MicroFreak and retry.");
        controller.stop();
        return Ok(());
    }
    let idx = devices
        .iter()
        .position(|d| {
            let l = d.to_lowercase();
            l.contains("microfreak") || l.contains("ultrafreak") || l.contains("arturia")
        })
        .unwrap_or(0);
    println!("MIDI from {}:", devices[idx]);
    let mut handler = MidiInputHandler::new();
    handler.connect_device(idx)?;

    // ------------------------------------------------------------------
    // Simple voice allocator: note -> slot. Each slot owns (osc, env) nodes.
    // ------------------------------------------------------------------
    let mut voice_note: [Option<u8>; VOICES] = [None; VOICES];

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

    while running.load(Ordering::Relaxed) {
        if let Some(ev) = handler.try_recv() {
            match ev {
                MidiEvent::NoteOn(n, _v) => {
                    // Steal the oldest free (or first) slot.
                    let slot = voice_note
                        .iter()
                        .position(|s| s.is_none())
                        .or_else(|| voice_note.iter().position(|s| s.is_some()))
                        .unwrap_or(0);
                    let (osc, env) = voice_pairs[slot];
                    control
                        .send(ControlMsg::SetFrequency {
                            node: osc,
                            hz: note_to_freq(n),
                        })
                        .expect("control send");
                    control
                        .send(ControlMsg::TriggerGate {
                            node: osc,
                            on: true,
                        })
                        .expect("control send");
                    control
                        .send(ControlMsg::TriggerGate {
                            node: env,
                            on: true,
                        })
                        .expect("control send");
                    voice_note[slot] = Some(n);
                    println!("NoteOn {} -> voice {}", n, slot);
                }
                MidiEvent::NoteOff(n, _) => {
                    if let Some(slot) = voice_note.iter().position(|s| *s == Some(n)) {
                        let (_osc, env) = voice_pairs[slot];
                        control
                            .send(ControlMsg::TriggerGate {
                                node: env,
                                on: false,
                            })
                            .expect("control send");
                        voice_note[slot] = None;
                        println!("NoteOff {} -> voice {}", n, slot);
                    }
                }
                MidiEvent::ControlChange(c, v) => println!("CC {}: {}", c, v),
                MidiEvent::PitchBend(b) => println!("PitchBend {}", b),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    controller.stop();
    println!("Stopped.");
    Ok(())
}
