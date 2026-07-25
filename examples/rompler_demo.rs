//! Offline ROMpler demo — produces a .wav proving the full stack works.
//!
//! This demo:
//! 1. Generates a synthetic sample (440 Hz sine wave)
//! 2. Builds the full ROMpler graph via `build_rompler_graph()`
//! 3. Creates a `RuntimeCore` with lock-free control channels
//! 4. Sends `TriggerGate` + `SetFrequency` via the control queue
//! 5. Renders ~1.5 seconds of audio offline
//! 6. Writes the result to `rompler_demo.wav`
//!
//! No MIDI hardware or audio device required.

use std::sync::Arc;

use auxide::control::ControlMsg;
use auxide::rt::{render_offline_handle, RuntimeCore};
use auxide_midi::midi_bridge::build_rompler_graph;

/// Generate a sine tone to use as the ROMpler sample.
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

    println!("Generating sample (440 Hz sine, 1 second)...");
    let sample = make_sample(440.0, 1.0, sample_rate);

    // ------------------------------------------------------------------
    // Build the ROMpler graph
    // ------------------------------------------------------------------
    println!("Building 8-voice ROMpler graph...");
    let (_graph, plan, voice_pairs, _filter_node) = build_rompler_graph(8, sample, sample_rate, 69);

    // ------------------------------------------------------------------
    // Create runtime with control channels
    // ------------------------------------------------------------------
    let (mut handle, mut control) = RuntimeCore::new_with_channels(plan, &_graph, sample_rate);

    // ------------------------------------------------------------------
    // Trigger voice 0: send SetFrequency + TriggerGate for oscillator
    // and envelope over the lock-free control queue.
    // ------------------------------------------------------------------
    let (osc0, env0) = voice_pairs[0];
    println!("Triggering voice 0 (osc={:?}, env={:?})...", osc0, env0);

    control
        .send(ControlMsg::SetFrequency {
            node: osc0,
            hz: 440.0,
        })
        .expect("control queue send");

    control
        .send(ControlMsg::TriggerGate {
            node: osc0,
            on: true,
        })
        .expect("control queue send");

    control
        .send(ControlMsg::TriggerGate {
            node: env0,
            on: true,
        })
        .expect("control queue send");

    // ------------------------------------------------------------------
    // Render 1.5 seconds offline via the new render_offline_handle
    // ------------------------------------------------------------------
    let frames = (1.5 * sample_rate) as usize;
    println!(
        "Rendering {} samples ({:.1} s) offline...",
        frames,
        frames as f32 / sample_rate
    );

    let output = render_offline_handle(&mut handle, frames).expect("offline rendering succeeded");

    // ------------------------------------------------------------------
    // Verify we got non-silent audio
    // ------------------------------------------------------------------
    let peak = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let rms = {
        let sum: f32 = output.iter().map(|s| s * s).sum();
        (sum / output.len() as f32).sqrt()
    };

    println!("Peak: {:.4}, RMS: {:.4}", peak, rms);
    assert!(peak > 0.001, "ROMpler produced silence! peak={}", peak);
    assert!(rms > 0.0001, "ROMpler RMS too low: {}", rms);

    // ------------------------------------------------------------------
    // Write to .wav
    // ------------------------------------------------------------------
    let path = "rompler_demo.wav";
    println!("Writing {}...", path);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec).expect("failed to create WAV file");

    for &sample in &output {
        writer
            .write_sample((sample * 32767.0) as i16)
            .expect("failed to write sample");
    }

    writer.finalize().expect("failed to finalize WAV");

    println!("✓ Successfully wrote {}", path);
    println!();
    println!("Summary:");
    println!("  Graph     : 8-voice ROMpler (Sampler→Multiply←AdsrEnvelope per voice, mixed → SvfFilter)");
    println!("  Control   : Lock-free SPSC queue (SetFrequency + TriggerGate)");
    println!("  Render    : render_offline_handle (new RuntimeHandle API)");
    println!(
        "  Output    : 1 channel, {} Hz, 16-bit PCM",
        sample_rate as u32
    );
    println!("  Duration  : {:.2} s", output.len() as f32 / sample_rate);
    println!("  Peak      : {:.4}", peak);
    println!("  RMS       : {:.4}", rms);
    println!();
    println!("Open rompler_demo.wav in any audio player to hear the result.");
}
