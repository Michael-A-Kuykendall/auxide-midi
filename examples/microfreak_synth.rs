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
//! Every MIDI event and every control message sent to the audio thread is
//! timestamped so you can correlate what you hear with what the code did
//! at that exact millisecond. Audio-thread health (callback count, xruns,
//! recovery flag, peak level, latency) is printed every 500 ms from the
//! main loop so you can see whether a crackle coincides with an xrun or
//! an overflow.
//!
//! Voice slot management: a slot is only re-usable after its ADSR release
//! window has fully elapsed (250ms guard, matching the 200ms ADSR release).
//! This eliminates the retrigger click that occurs when a slot is reused
//! while its ADSR release is still actively decaying.
//!
//! REQUIRES HARDWARE: a MIDI input device AND an audio output device. Connect
//! a MicroFreak and run `cargo run --example microfreak_synth` (Ctrl+C to stop).
//!
//! Build: `cargo build --example microfreak_synth`
//! Clippy: `cargo clippy -p auxide-midi -- -D warnings`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use auxide::control::{ControlMsg, PARAM_RESONANCE};
use auxide::rt::RuntimeCore;
use auxide_io::StreamController;
use auxide_midi::midi_bridge::build_rompler_graph;
use auxide_midi::{MidiEvent, MidiInputHandler};

const VOICES: usize = 8;
/// Hold a slot busy for this long after note-off so the ADSR release fully
/// decays before the slot can be re-used (prevents retrigger click).
const RELEASE_GUARD_MS: u64 = 250;

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

/// Per-voice tracking: what note (if any) is assigned, and when the release
/// window ends so the slot can be safely re-used.
struct VoiceSlot {
    note: Option<u8>,
    release_until: Option<Instant>,
}

/// Timestamped log line: `[T+<ms>ms] <formatted-args>`.
macro_rules! log {
    ($start:expr, $($arg:tt)*) => {
        println!("[T+{}ms] {}", $start.elapsed().as_millis(), format_args!($($arg)*));
    };
}

fn main() -> Result<()> {
    let sr = 44100.0;
    let sample = make_sample(sr);
    let start = Instant::now();

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
    let diag_interval = Duration::from_millis(500);
    let mut last_diag = start;
    log!(
        start,
        "playing through the host sound device (Ctrl+C to stop)"
    );

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
    log!(start, "MIDI from {}", devices[idx]);
    let mut handler = MidiInputHandler::new();
    handler.connect_device(idx)?;

    // ------------------------------------------------------------------
    // Voice allocation: each slot tracks its MIDI note and when the
    // ADSR release window ends.  A slot may only be re-used once both
    // conditions hold: note is None AND release_until has elapsed.
    // This eliminates the retrigger click that occurs when a slot is
    // reused while its ADSR release is still actively decaying.
    // ------------------------------------------------------------------
    let mut voices: [VoiceSlot; VOICES] = std::array::from_fn(|_| VoiceSlot {
        note: None,
        release_until: None,
    });
    let mut bend: f32 = 1.0;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

    while running.load(Ordering::Relaxed) {
        // Periodic audio-thread health snapshot (main thread, non-RT).
        if Instant::now() >= last_diag + diag_interval {
            last_diag = Instant::now();
            let snap = controller.diagnostics();
            log!(
                start,
                "AUDIO cb={} overflow={} glitch={} recovery={} peak={:.4} latency={:?}",
                snap.callback_count,
                snap.overflow_count,
                snap.glitch_count,
                controller.recovery_needed(),
                snap.peak,
                snap.latency,
            );
        }

        if let Some(ev) = handler.try_recv() {
            match ev {
                MidiEvent::NoteOn(n, _v) => {
                    let now = Instant::now();
                    let slot = voices.iter().position(|v| {
                        v.note.is_none() && v.release_until.map_or(true, |t| now >= t)
                    });
                    let slot = match slot {
                        Some(i) => i,
                        None => {
                            log!(
                                start,
                                "DROPPED note-on {} (all {} voices in use or releasing)",
                                n,
                                VOICES
                            );
                            continue;
                        }
                    };
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
                    voices[slot].note = Some(n);
                    voices[slot].release_until = None;
                    log!(
                        start,
                        "note-on  {} slot {} freq {:.1} Hz gate-on osc+env",
                        n,
                        slot,
                        note_to_freq(n) * bend,
                    );
                }
                MidiEvent::NoteOff(n, _) => {
                    if let Some(slot) = voices.iter().position(|v| v.note == Some(n)) {
                        let (_osc, env) = voice_pairs[slot];
                        control
                            .send(ControlMsg::TriggerGate {
                                node: env,
                                on: false,
                            })
                            .expect("control send");
                        voices[slot].note = None;
                        voices[slot].release_until =
                            Some(Instant::now() + Duration::from_millis(RELEASE_GUARD_MS));
                        log!(
                            start,
                            "note-off {} slot {} gate-off env (release {} ms)",
                            n,
                            slot,
                            RELEASE_GUARD_MS
                        );
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
                    log!(start, "CC74  cutoff -> {:.1} Hz", hz);
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
                    log!(start, "CC71  resonance -> {:.2}", res);
                }
                MidiEvent::PitchBend(b) => {
                    let semis = (b as f32 - 8192.0) / 8192.0 * 2.0;
                    bend = f32::powf(2.0, semis / 12.0);
                    for slot in 0..VOICES {
                        if let Some(n) = voices[slot].note {
                            let (osc, _env) = voice_pairs[slot];
                            control
                                .send(ControlMsg::SetFrequency {
                                    node: osc,
                                    hz: note_to_freq(n) * bend,
                                })
                                .expect("control send");
                        }
                    }
                    log!(
                        start,
                        "pitch-bend -> {:.2} semitones  bend {:.3}x",
                        semis,
                        bend
                    );
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    controller.stop();
    log!(start, "stopped.");
    Ok(())
}
