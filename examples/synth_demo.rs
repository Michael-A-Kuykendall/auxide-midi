//! Synth demo - proves the full MIDI → Audio stack works
//!
//! Uses the new RuntimeCore/RuntimeControl architecture for proper
//! MIDI → parameter control → audio output flow.
//!
//! Usage:
//!   cargo run --example synth_demo
//!
//! Then:
//!   1. Connect your MIDI keyboard
//!   2. Press keys to hear audio
//!   3. Turn CC knobs to modulate parameters
//!   4. Stop with Ctrl+C

use auxide::graph::{Graph, NodeId, NodeType, PortId, Rate};
use auxide::plan::Plan;
use auxide::rt::RuntimeCore;
use auxide_io::stream_controller::StreamController;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Simple voice tracking (which oscillator node maps to which MIDI note)
struct VoiceManager {
    /// NodeId of each oscillator
    osc_nodes: Vec<NodeId>,
    /// NodeId of each gain (for mute/unmute per voice)
    gain_nodes: Vec<NodeId>,
    /// Which note each voice is playing (None = free)
    voice_notes: Vec<Option<u8>>,
    /// Note to voice mapping for quick lookup
    note_to_voice: HashMap<u8, usize>,
}

impl VoiceManager {
    fn new(osc_nodes: Vec<NodeId>, gain_nodes: Vec<NodeId>) -> Self {
        let num_voices = osc_nodes.len();
        Self {
            osc_nodes,
            gain_nodes,
            voice_notes: vec![None; num_voices],
            note_to_voice: HashMap::new(),
        }
    }

    /// Allocate a voice for a note. Returns (voice_idx, osc_node, gain_node) or None if full.
    fn note_on(&mut self, note: u8) -> Option<(usize, NodeId, NodeId)> {
        // Already playing?
        if self.note_to_voice.contains_key(&note) {
            return None;
        }

        // Find free voice
        for (idx, voice_note) in self.voice_notes.iter_mut().enumerate() {
            if voice_note.is_none() {
                *voice_note = Some(note);
                self.note_to_voice.insert(note, idx);
                return Some((idx, self.osc_nodes[idx], self.gain_nodes[idx]));
            }
        }
        None // All voices busy
    }

    /// Release a voice. Returns (voice_idx, gain_node) or None if not playing.
    fn note_off(&mut self, note: u8) -> Option<(usize, NodeId)> {
        if let Some(idx) = self.note_to_voice.remove(&note) {
            self.voice_notes[idx] = None;
            Some((idx, self.gain_nodes[idx]))
        } else {
            None
        }
    }

    fn active_count(&self) -> usize {
        self.voice_notes.iter().filter(|v| v.is_some()).count()
    }
}

/// Convert MIDI note to frequency
fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════╗");
    println!("║  Auxide Synth Demo (New Architecture)  ║");
    println!("║  Connect your MIDI keyboard            ║");
    println!("║  Press Ctrl+C to exit                  ║");
    println!("╚════════════════════════════════════════╝\n");

    // =========================================================================
    // Build a MINIMAL synth graph for debugging: just 1 oscillator → gain → output
    // =========================================================================
    let mut graph = Graph::new();

    // Single sine oscillator (built-in)
    let osc = graph.add_node(NodeType::SineOsc { freq: 440.0 });

    // Single gain node - starts at 1.0 so we can hear initial tone
    let gain = graph.add_node(NodeType::Gain { gain: 1.0 });

    // Connect osc → gain
    graph
        .add_edge(auxide::graph::Edge {
            from_node: osc,
            from_port: PortId(0),
            to_node: gain,
            to_port: PortId(0),
            rate: Rate::Audio,
        })
        .expect("edge osc->gain");

    // Output sink
    let sink = graph.add_node(NodeType::OutputSink);
    graph
        .add_edge(auxide::graph::Edge {
            from_node: gain,
            from_port: PortId(0),
            to_node: sink,
            to_port: PortId(0),
            rate: Rate::Audio,
        })
        .expect("edge gain->sink");

    println!(
        "✓ Minimal graph: osc({:?}) → gain({:?}) → sink({:?})",
        osc, gain, sink
    );

    // For voice manager, just use a single voice
    let osc_nodes = vec![osc];
    let gain_nodes = vec![gain];

    // =========================================================================
    // Compile and create runtime with control channels
    // =========================================================================
    let plan = Plan::compile(&graph, 512).expect("compile plan");

    let sample_rate = StreamController::get_best_sample_rate(44100.0).unwrap_or(48000.0);
    println!("✓ Using sample rate: {}Hz", sample_rate as u32);

    // NEW ARCHITECTURE: RuntimeCore::new_with_channels returns (handle, control)
    let (handle, mut control) = RuntimeCore::new_with_channels(plan, &graph, sample_rate);

    // Create voice manager
    let mut voices = VoiceManager::new(osc_nodes, gain_nodes);

    // =========================================================================
    // Start audio stream with RuntimeHandle
    // =========================================================================
    println!("✓ Starting audio stream...");
    let stream_controller = StreamController::play_handle(handle)?;
    stream_controller.start()?;
    println!("✓ Audio running\n");

    // =========================================================================
    // Setup MIDI input
    // =========================================================================
    println!("Scanning MIDI devices...");
    let midi_in = midir::MidiInput::new("auxide-synth")?;
    let ports = midi_in.ports();

    if ports.is_empty() {
        eprintln!("✗ No MIDI devices found. Using keyboard fallback.");
        println!("\nPress keys 1-4 to trigger notes, Q to quit\n");

        // Keyboard fallback loop
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

        // Simple keyboard input (won't work well in all terminals)
        println!("╔════════════════════════════════════════╗");
        println!("║ No MIDI - audio is running silently    ║");
        println!("║ Use --example note_echo to test MIDI   ║");
        println!("║ Press Ctrl+C to exit                   ║");
        println!("╚════════════════════════════════════════╝\n");

        while running.load(Ordering::Relaxed) {
            // Drain invariant signals periodically (for monitoring)
            let signals = control.drain_invariant_signals();
            if !signals.is_empty() {
                // Signals are being generated - RT is running
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    } else {
        // List devices
        println!("Available MIDI devices:");
        for (idx, port) in ports.iter().enumerate() {
            let name = midi_in.port_name(port).unwrap_or_default();
            println!("  [{}] {}", idx, name);
        }

        // Auto-select first device (or MicroFreak if found)
        let port_idx = ports
            .iter()
            .position(|p| {
                midi_in
                    .port_name(p)
                    .map(|n| n.contains("MicroFreak") || n.contains("Arturia"))
                    .unwrap_or(false)
            })
            .unwrap_or(0);

        let port_name = midi_in.port_name(&ports[port_idx]).unwrap_or_default();
        println!("\n✓ Connecting to: {}", port_name);

        // Setup MIDI callback
        let (midi_tx, midi_rx) = crossbeam_channel::unbounded::<Vec<u8>>();

        let _midi_conn = midi_in.connect(
            &ports[port_idx],
            "auxide-input",
            move |_timestamp, message, _| {
                let _ = midi_tx.send(message.to_vec());
            },
            (),
        )?;

        println!("✓ MIDI connected\n");

        println!("╔════════════════════════════════════════╗");
        println!("║ Ready to play!                         ║");
        println!("║ • Play notes on your MIDI keyboard     ║");
        println!("║ • Watch voice count below              ║");
        println!("║ • Press Ctrl+C to exit                 ║");
        println!("╚════════════════════════════════════════╝\n");

        // Setup graceful shutdown
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || r.store(false, Ordering::Relaxed))?;

        let mut last_voice_count = 0;

        // Main loop: process MIDI → send control messages → audio runs
        while running.load(Ordering::Relaxed) {
            // Process all pending MIDI messages
            while let Ok(msg) = midi_rx.try_recv() {
                if msg.len() >= 3 {
                    let status = msg[0] & 0xF0;
                    let note = msg[1];
                    let velocity = msg[2];

                    match status {
                        0x90 if velocity > 0 => {
                            // Note On
                            if let Some((voice_idx, osc_node, gain_node)) = voices.note_on(note) {
                                let freq = midi_to_freq(note);
                                println!(
                                    "🎵 Note ON:  {} ({}Hz) → voice {}",
                                    note, freq as u32, voice_idx
                                );

                                // Send control messages to RT
                                let _ = control.set_frequency(osc_node, freq);
                                let _ = control.set_gain(gain_node, velocity as f32 / 127.0);
                            }
                        }
                        0x80 | 0x90 => {
                            // Note Off (0x80) or Note On with velocity 0
                            if let Some((voice_idx, gain_node)) = voices.note_off(note) {
                                println!("🔇 Note OFF: {} → voice {}", note, voice_idx);
                                // Mute the voice by setting gain to 0
                                let _ = control.set_gain(gain_node, 0.0);
                            }
                        }
                        0xB0 => {
                            // CC message
                            let cc_num = msg[1];
                            let cc_val = msg[2];
                            println!("🎛  CC#{}: {}", cc_num, cc_val);
                            // Could map CC to master gain, filter, etc.
                        }
                        _ => {}
                    }
                }
            }

            // Show voice count changes
            let active = voices.active_count();
            if active != last_voice_count {
                println!("   Active voices: {}/4", active);
                last_voice_count = active;
            }

            // Drain invariant signals (verify RT is healthy)
            let _signals = control.drain_invariant_signals();

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    // Cleanup
    println!("\n✓ Shutting down...");
    stream_controller.stop();
    std::thread::sleep(std::time::Duration::from_millis(100));
    println!("✓ Goodbye! 👋\n");

    Ok(())
}
