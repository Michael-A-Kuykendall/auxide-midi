//! Live MicroFreak ROMpler — the plug-in-and-play reference synth.
//!
//! Drives a polyphonic ROMpler from a connected Arturia MicroFreak (or any
//! MIDI input) in real time and plays it through the host sound device via
//! `auxide_io::StreamController`. One recorded timbre (a generated sample
//! here) propagates across the keyboard by pitch-shifting, matching the
//! MicroFreak's own CC map:
//!
//! - CC74  -> filter cutoff   (log-mapped 20 Hz .. 20 kHz)
//! - CC71  -> filter resonance (0 .. 1)
//! - Pitch bend wheel -> +/- 2 semitones
//! - Note on/off -> polyphonic voice allocation (8 voices)
//!
//! REQUIRES HARDWARE: a MIDI input device AND an audio output device. Connect
//! a MicroFreak and run `cargo run --example microfreak_synth` (Ctrl+C to stop).
//!
//! Build: `cargo build --example microfreak_synth`
//! Clippy: `cargo clippy -p auxide-midi -- -D warnings`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use auxide::control::{ControlMsg, PARAM_RESONANCE};
use auxide::rt::RuntimeCore;
use auxide_io::StreamController;
use auxide_midi::midi_bridge::build_rompler_graph;
use auxide_midi::{MidiEvent, MidiInputHandler};

const VOICES: usize = 8;

/// A one-second 440 Hz sine used as the ROMpler sample (stand-in for a
/// recorded timbre; swap for a loaded .wav to taste).
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

/// Map a 0..127 CC value to a log-spaced cutoff in [20 Hz, 20 kHz].
fn cc_to_cutoff(v: u8) -> f32 {
    20.0 * f32::powf(1000.0, v as f32 / 127.0)
}

fn main() -> Result<()> {
    let sr = 44100.0;
    let sample = make_sample(sr);

    // ------------------------------------------------------------------
    // Build the polyphonic ROMpler graph and a RuntimeHandle we can stream.
    // `filter_node` is the shared global lowpass after the mixer.
    // ------------------------------------------------------------------
    let (_graph, plan, voice_pairs, filter_opt) = build_rompler_graph(VOICES, sample, sr, 69);
    let filter_node = filter_opt.expect("ROMpler graph provides a global filter node");
    let (handle, mut control) = RuntimeCore::new_with_channels(plan, &_graph, sr);

    // ------------------------------------------------------------------
    // Stream the ROMpler to the host sound device.
    // ------------------------------------------------------------------
    let controller = StreamController::play_handle(handle)?;
    controller.start()?;
    println!("Playing through the host sound device (Ctrl+C to stop).");

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
    // Voice allocation: note -> slot. Each slot owns (osc, env) node ids.
    // `bend` is the global pitch-bend ratio, applied to active + new voices.
    // ------------------------------------------------------------------
    let mut voice_note: [Option<u8>; VOICES] = [None; VOICES];
    let mut bend: f32 = 1.0;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

    while running.load(Ordering::Relaxed) {
        if let Some(ev) = handler.try_recv() {
            match ev {
                MidiEvent::NoteOn(n, _v) => {
                    let slot = voice_note
                        .iter()
                        .position(|s| s.is_none())
                        .or_else(|| voice_note.iter().position(|s| s.is_some()))
                        .unwrap_or(0);
                    let (osc, env) = voice_pairs[slot];
                    control
                        .send(ControlMsg::SetFrequency {
                            node: osc,
                            hz: note_to_freq(n) * bend,
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
                MidiEvent::ControlChange(74, v) => {
                    let hz = cc_to_cutoff(v);
                    control
                        .send(ControlMsg::SetFilterCutoff {
                            node: filter_node,
                            hz,
                        })
                        .expect("control send");
                    println!("CC74 cutoff -> {:.1} Hz", hz);
                }
                MidiEvent::ControlChange(71, v) => {
                    let res = v as f32 / 127.0;
                    control
                        .send(ControlMsg::SetParam {
                            node: filter_node,
                            param_idx: PARAM_RESONANCE,
                            value: res,
                        })
                        .expect("control send");
                    println!("CC71 resonance -> {:.2}", res);
                }
                MidiEvent::PitchBend(b) => {
                    // +/- 2 semitones across the 0..16383 wheel (center 8192).
                    let semis = (b as f32 - 8192.0) / 8192.0 * 2.0;
                    bend = f32::powf(2.0, semis / 12.0);
                    for slot in 0..VOICES {
                        if let Some(n) = voice_note[slot] {
                            let (osc, _env) = voice_pairs[slot];
                            control
                                .send(ControlMsg::SetFrequency {
                                    node: osc,
                                    hz: note_to_freq(n) * bend,
                                })
                                .expect("control send");
                        }
                    }
                    println!("PitchBend -> {:.2} semitones", semis);
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    controller.stop();
    println!("Stopped.");
    Ok(())
}
