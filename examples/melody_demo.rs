//! Melody demo — proves the full MIDI → ROMpler → .wav path with musical note changes.
//!
//! Feeds a sequence of `note_on`/`note_off` events through the Synth facade,
//! rendering the result to `melody_demo.wav`. No MIDI hardware required.

use std::sync::Arc;

use auxide_midi::Synth;

/// A timed MIDI event.
struct Event {
    /// Sample offset from start.
    at_sample: usize,
    kind: EventKind,
}

enum EventKind {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
}

fn make_sample(freq: f32, dur_s: f32, sr: f32) -> Arc<Vec<f32>> {
    let n = (dur_s * sr) as usize;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        v.push((2.0 * std::f32::consts::PI * freq * (i as f32) / sr).sin());
    }
    Arc::new(v)
}

fn main() {
    let sample_rate = 44100.0;
    let block_size = 64;

    println!("Generating sample...");
    let sample = make_sample(440.0, 1.0, sample_rate);

    println!("Building 8-voice ROMpler via Synth facade...");
    let mut synth = Synth::new(sample, sample_rate, 8, 69);
    let mut out = vec![0.0; block_size];
    let mut all_samples: Vec<f32> = Vec::new();

    // A simple melody: C4(60), E4(64), G4(67), C5(72) — quarter notes at 120 BPM
    // 120 BPM = 0.5 s per beat = 22050 samples per beat at 44100 Hz
    let beat_samples = (0.5 * sample_rate) as usize;

    let events = vec![
        // Bar 1: C4 major triad ascending
        Event {
            at_sample: 0,
            kind: EventKind::NoteOn {
                note: 60,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples,
            kind: EventKind::NoteOff { note: 60 },
        },
        Event {
            at_sample: beat_samples,
            kind: EventKind::NoteOn {
                note: 64,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 2,
            kind: EventKind::NoteOff { note: 64 },
        },
        Event {
            at_sample: beat_samples * 2,
            kind: EventKind::NoteOn {
                note: 67,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 3,
            kind: EventKind::NoteOff { note: 67 },
        },
        Event {
            at_sample: beat_samples * 3,
            kind: EventKind::NoteOn {
                note: 72,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 4,
            kind: EventKind::NoteOff { note: 72 },
        },
        // Bar 2: descending
        Event {
            at_sample: beat_samples * 4,
            kind: EventKind::NoteOn {
                note: 72,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 5,
            kind: EventKind::NoteOff { note: 72 },
        },
        Event {
            at_sample: beat_samples * 5,
            kind: EventKind::NoteOn {
                note: 67,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 6,
            kind: EventKind::NoteOff { note: 67 },
        },
        Event {
            at_sample: beat_samples * 6,
            kind: EventKind::NoteOn {
                note: 64,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 7,
            kind: EventKind::NoteOff { note: 64 },
        },
        Event {
            at_sample: beat_samples * 7,
            kind: EventKind::NoteOn {
                note: 60,
                velocity: 100,
            },
        },
        Event {
            at_sample: beat_samples * 8,
            kind: EventKind::NoteOff { note: 60 },
        },
    ];

    let total_frames = beat_samples * 8 + block_size; // 8 beats + tail
    let mut event_idx = 0;

    println!("Rendering melody...");
    let mut processed = 0;
    while processed < total_frames {
        // Fire events that fall in this block
        while event_idx < events.len() && events[event_idx].at_sample <= processed {
            match &events[event_idx].kind {
                EventKind::NoteOn { note, velocity } => {
                    synth.note_on(*note, *velocity);
                }
                EventKind::NoteOff { note } => {
                    synth.note_off(*note);
                }
            }
            event_idx += 1;
        }

        synth.process_block(&mut out).unwrap();
        all_samples.extend_from_slice(&out);
        processed += block_size;
    }

    // Let release tail ring out for another second
    let tail_blocks = (sample_rate as usize / block_size) + 1;
    for _ in 0..tail_blocks {
        synth.process_block(&mut out).unwrap();
        all_samples.extend_from_slice(&out);
    }

    let peak = all_samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let rms = {
        let sum: f32 = all_samples.iter().map(|s| s * s).sum();
        (sum / all_samples.len() as f32).sqrt()
    };

    println!("Peak: {:.4}, RMS: {:.4}", peak, rms);

    let path = "melody_demo.wav";
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for &s in &all_samples {
        writer.write_sample((s * 32767.0) as i16).unwrap();
    }
    writer.finalize().unwrap();

    println!(
        "✓ Wrote {} ({:.1}s, {:.1} MB)",
        path,
        all_samples.len() as f32 / sample_rate,
        (all_samples.len() * 2) as f32 / 1_048_576.0
    );
    println!("Open in any audio player to hear C4-E4-G4-C5 ascending/descending.");
}
