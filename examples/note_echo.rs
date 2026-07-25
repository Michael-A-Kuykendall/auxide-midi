//! Echo MIDI input to the console and play it through the real `Synth` facade,
//! rendering the result to `note_echo_demo.wav` (no audio device required
//! for the render itself; a MIDI device is needed for input).

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

    let devices = MidiInputHandler::list_devices()?;
    if devices.is_empty() {
        println!("No MIDI devices found; nothing to echo.");
        return Ok(());
    }
    let idx = devices
        .iter()
        .position(|d| {
            let l = d.to_lowercase();
            l.contains("microfreak") || l.contains("ultrafreak") || l.contains("arturia")
        })
        .unwrap_or(0);
    println!("Echoing MIDI from {} (Ctrl+C to stop)", devices[idx]);
    let mut handler = MidiInputHandler::new();
    handler.connect_device(idx)?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

    while running.load(Ordering::Relaxed) {
        if let Some(ev) = handler.try_recv() {
            match ev {
                MidiEvent::NoteOn(n, v) => {
                    synth.note_on(n, v);
                    println!("NoteOn {} vel {}", n, v);
                }
                MidiEvent::NoteOff(n, _) => {
                    synth.note_off(n);
                    println!("NoteOff {}", n);
                }
                MidiEvent::ControlChange(c, v) => println!("CC {}: {}", c, v),
                MidiEvent::PitchBend(b) => println!("PitchBend {}", b),
            }
        }
        synth.process_block(&mut block).expect("render block");
        rendered.extend_from_slice(&block);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    render_to_wav("note_echo_demo.wav", sr, &rendered)?;
    println!("Wrote note_echo_demo.wav ({} frames)", rendered.len());
    Ok(())
}
