//! Polyphonic MIDI synthesizer demo using the real `Synth` facade.
//!
//! The `Synth` facade wraps the auxide kernel's runtime control plane: every
//! note is routed through `note_on`/`note_off` into the lock-free control
//! queue, so polyphony, per-note pitch, ADSR, and the SVF filter all work
//! for real (no "all notes 440 Hz" overclaim).
//!
//! With no MIDI device present this demo renders a short chord to
//! `poly_synth_demo.wav` as audible proof. For live CPAL output, wrap the
//! `RuntimeHandle` with `auxide_io::StreamController::play_handle`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use auxide_midi::{MidiEvent, MidiInputHandler, Synth};

/// A 1-second 440 Hz sine, used as the ROMpler sample.
fn make_sample(sr: f32) -> Arc<Vec<f32>> {
    Arc::new(
        (0..sr as usize)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin())
            .collect(),
    )
}

fn render_to_wav(path: &str, sr: f32, rendered: &[f32]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sr as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in rendered {
        w.write_sample((s * 32767.0) as i16)?;
    }
    w.finalize()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let sr = 44100.0;
    let mut synth = Synth::new(make_sample(sr), sr, 8, 69);
    let mut block = vec![0.0f32; 64];
    let mut rendered = Vec::new();

    match MidiInputHandler::list_devices() {
        Ok(devices) if !devices.is_empty() => {
            let idx = devices
                .iter()
                .position(|d| {
                    let l = d.to_lowercase();
                    l.contains("microfreak") || l.contains("ultrafreak") || l.contains("arturia")
                })
                .unwrap_or(0);
            println!("MIDI device: {}", devices[idx]);
            let mut handler = MidiInputHandler::new();
            handler.connect_device(idx)?;

            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

            println!("Playing — Ctrl+C to stop. Rendering to poly_synth_demo.wav");
            while running.load(Ordering::Relaxed) {
                if let Some(ev) = handler.try_recv() {
                    match ev {
                        MidiEvent::NoteOn(n, v) => synth.note_on(n, v),
                        MidiEvent::NoteOff(n, _) => synth.note_off(n),
                        _ => {}
                    }
                }
                synth.process_block(&mut block).expect("render block");
                rendered.extend_from_slice(&block);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        _ => {
            println!("No MIDI device — rendering a demo chord to poly_synth_demo.wav");
            for n in [60u8, 64, 67, 72] {
                synth.note_on(n, 100);
            }
            for _ in 0..300 {
                synth.process_block(&mut block).expect("render block");
                rendered.extend_from_slice(&block);
            }
            for n in [60u8, 64, 67, 72] {
                synth.note_off(n);
            }
            for _ in 0..400 {
                synth.process_block(&mut block).expect("render block");
                rendered.extend_from_slice(&block);
            }
        }
    }

    render_to_wav("poly_synth_demo.wav", sr, &rendered)?;
    println!("Wrote poly_synth_demo.wav ({} frames)", rendered.len());
    Ok(())
}
