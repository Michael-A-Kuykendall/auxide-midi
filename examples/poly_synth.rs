//! Polyphonic MIDI synthesizer demo
//!
//! ⚠️ LIMITATION: This demo plays all notes at 440Hz due to auxide graph immutability.
//! Voice allocation and MIDI handling work correctly, but dynamic pitch requires
//! graph rebuilding (see issue #X for auxide kernel parameter updates).

use auxide::graph::{Graph, NodeType, PortId, Rate};
use auxide::plan::Plan;
use auxide::rt::Runtime;
use auxide_dsp::nodes::envelopes::AdsrEnvelope;
use auxide_dsp::nodes::filters::SvfFilter;
use auxide_dsp::nodes::filters::SvfMode;
use auxide_dsp::nodes::oscillators::SawOsc;
use auxide_io::stream_controller::StreamController;
use auxide_midi::{
    note_to_freq, pitch_bend_to_ratio, velocity_to_gain, CCMap, EnvStage, MidiEvent,
    MidiInputHandler, ParamSmoother, ParamTarget, VoiceAllocator, VoiceId, VoicePool, VoiceState,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Message from MIDI thread to audio thread
#[derive(Debug, Clone)]
enum SynthMessage {
    NoteOn {
        voice: VoiceId,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        note: u8,
    },
    ControlChange {
        target: ParamTarget,
        value: f32,
    },
    PitchBend {
        ratio: f32,
    },
}

struct Synth {
    voice_pool: VoicePool,
    voice_allocator: VoiceAllocator,
    cc_map: CCMap,
    filter_cutoff_smoother: ParamSmoother,
    pitch_bend_ratio: f32,
    message_sender: Sender<SynthMessage>,
    message_receiver: Receiver<SynthMessage>,
}

impl Synth {
    fn new() -> Self {
        let (sender, receiver) = bounded(256);
        Self {
            voice_pool: VoicePool::new(),
            voice_allocator: VoiceAllocator::new(),
            cc_map: CCMap::new(),
            filter_cutoff_smoother: ParamSmoother::new(),
            pitch_bend_ratio: 1.0,
            message_sender: sender,
            message_receiver: receiver,
        }
    }

    fn build_graph() -> (Graph, Plan) {
        let mut graph = Graph::new();

        // Create 8 voices, each with: SawOsc -> SvfFilter -> ADSR -> Gain
        let mut voice_outputs = Vec::new();

        for voice_idx in 0..8 {
            let osc = graph.add_external_node(SawOsc { freq: 440.0 });
            let filter = graph.add_external_node(SvfFilter {
                cutoff: 1000.0,
                resonance: 0.5,
                mode: SvfMode::Lowpass,
            });
            let adsr = graph.add_external_node(AdsrEnvelope {
                attack_ms: 10.0,
                decay_ms: 100.0,
                sustain_level: 0.8,
                release_ms: 200.0,
                curve: 0.0,
            });
            let gain = graph.add_node(NodeType::Gain { gain: 0.0 });

            // Connect: Osc -> Filter -> ADSR -> Gain
            graph
                .add_edge(auxide::graph::Edge {
                    from_node: osc,
                    from_port: PortId(0),
                    to_node: filter,
                    to_port: PortId(0),
                    rate: Rate::Audio,
                })
                .unwrap();

            graph
                .add_edge(auxide::graph::Edge {
                    from_node: filter,
                    from_port: PortId(0),
                    to_node: adsr,
                    to_port: PortId(0),
                    rate: Rate::Audio,
                })
                .unwrap();

            graph
                .add_edge(auxide::graph::Edge {
                    from_node: adsr,
                    from_port: PortId(0),
                    to_node: gain,
                    to_port: PortId(0),
                    rate: Rate::Audio,
                })
                .unwrap();

            voice_outputs.push(gain);
        }

        // Create mixers for voices (tree structure since Mix only takes 2 inputs)
        let mut mix_outputs = Vec::new();

        // Mix voices in pairs: 0+1, 2+3, 4+5, 6+7
        for i in (0..8).step_by(2) {
            let mix = graph.add_node(NodeType::Mix);
            graph
                .add_edge(auxide::graph::Edge {
                    from_node: voice_outputs[i],
                    from_port: PortId(0),
                    to_node: mix,
                    to_port: PortId(0),
                    rate: Rate::Audio,
                })
                .unwrap();
            graph
                .add_edge(auxide::graph::Edge {
                    from_node: voice_outputs[i + 1],
                    from_port: PortId(0),
                    to_node: mix,
                    to_port: PortId(1),
                    rate: Rate::Audio,
                })
                .unwrap();
            mix_outputs.push(mix);
        }

        // Mix the pair results: (0+1)+(2+3), (4+5)+(6+7)
        let mut final_mixes = Vec::new();
        for i in (0..4).step_by(2) {
            let mix = graph.add_node(NodeType::Mix);
            graph
                .add_edge(auxide::graph::Edge {
                    from_node: mix_outputs[i],
                    from_port: PortId(0),
                    to_node: mix,
                    to_port: PortId(0),
                    rate: Rate::Audio,
                })
                .unwrap();
            graph
                .add_edge(auxide::graph::Edge {
                    from_node: mix_outputs[i + 1],
                    from_port: PortId(0),
                    to_node: mix,
                    to_port: PortId(1),
                    rate: Rate::Audio,
                })
                .unwrap();
            final_mixes.push(mix);
        }

        // Final mix: mix the two remaining signals
        let final_mix = graph.add_node(NodeType::Mix);
        graph
            .add_edge(auxide::graph::Edge {
                from_node: final_mixes[0],
                from_port: PortId(0),
                to_node: final_mix,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();
        graph
            .add_edge(auxide::graph::Edge {
                from_node: final_mixes[1],
                from_port: PortId(0),
                to_node: final_mix,
                to_port: PortId(1),
                rate: Rate::Audio,
            })
            .unwrap();

        // Create output sink
        let sink = graph.add_node(NodeType::OutputSink);
        graph
            .add_edge(auxide::graph::Edge {
                from_node: final_mix,
                from_port: PortId(0),
                to_node: sink,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();

        let plan = Plan::compile(&graph, 64).unwrap();
        (graph, plan)
    }

    fn handle_midi_event(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn(note, velocity) => {
                if let Some(voice_id) = self.voice_allocator.allocate_voice(note) {
                    let _ = self.message_sender.send(SynthMessage::NoteOn {
                        voice: voice_id,
                        note,
                        velocity,
                    });
                }
            }
            MidiEvent::NoteOff(note, _) => {
                self.voice_allocator.release_voice(note);
                let _ = self.message_sender.send(SynthMessage::NoteOff { note });
            }
            MidiEvent::ControlChange(cc_num, value) => {
                if let Some((target, normalized_value)) = self.cc_map.map_cc(cc_num, value) {
                    let _ = self.message_sender.send(SynthMessage::ControlChange {
                        target,
                        value: normalized_value,
                    });
                }
            }
            MidiEvent::PitchBend(bend) => {
                let ratio = pitch_bend_to_ratio(bend);
                let _ = self.message_sender.send(SynthMessage::PitchBend { ratio });
            }
        }
    }

    fn process_messages(&mut self) {
        while let Ok(message) = self.message_receiver.try_recv() {
            match message {
                SynthMessage::NoteOn {
                    voice,
                    note,
                    velocity,
                } => {
                    let voice_state = self.voice_pool.get_voice_mut(voice.0);
                    voice_state.trigger(note, velocity);

                    // Update oscillator frequency
                    let freq = note_to_freq(note) as f32;
                    // Note: In this simplified example, we don't update oscillator frequency
                    // as auxide nodes are immutable. For dynamic frequency, you'd need
                    // to recreate the graph or use a different architecture.
                }
                SynthMessage::NoteOff { note } => {
                    // Find voice playing this note and release it
                    for i in 0..8 {
                        let voice_state = self.voice_pool.get_voice_mut(i);
                        if voice_state.active && voice_state.note == note {
                            voice_state.release();
                            self.voice_allocator.release_voice(note);
                            break;
                        }
                    }
                }
                SynthMessage::ControlChange { target, value } => {
                    match target {
                        ParamTarget::FilterCutoff => {
                            self.filter_cutoff_smoother
                                .set_target(value * 5000.0 + 100.0);
                        }
                        _ => {} // Other parameters not implemented in this demo
                    }
                }
                SynthMessage::PitchBend { ratio } => {
                    self.pitch_bend_ratio = ratio;
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    println!("Auxide MIDI Polyphonic Synthesizer");
    println!("===================================");
    println!();

    // Build the synth graph once
    println!("Building 8-voice synthesizer graph...");

    // First, determine the best sample rate for audio output
    let target_sample_rate = 44100.0;
    let actual_sample_rate =
        StreamController::get_best_sample_rate(target_sample_rate).unwrap_or(target_sample_rate);

    if (actual_sample_rate - target_sample_rate).abs() > 100.0 {
        println!(
            "Using sample rate: {}Hz (requested {}Hz)",
            actual_sample_rate, target_sample_rate
        );
    }

    let (_graph, plan) = Synth::build_graph();
    let runtime = Runtime::new(plan, &_graph, actual_sample_rate);
    println!("Graph compiled successfully");
    println!();

    // Setup MIDI
    let devices = MidiInputHandler::list_devices()?;

    if devices.is_empty() {
        println!("No MIDI input devices found.");
        println!("Please connect a MIDI keyboard and try again.");
        return Ok(());
    }

    // Auto-select MicroFreak or prompt user
    let mut selected_index = None;
    for (i, device) in devices.iter().enumerate() {
        if device.to_lowercase().contains("microfreak") || device.to_lowercase().contains("arturia")
        {
            selected_index = Some(i);
            break;
        }
    }

    if selected_index.is_none() {
        println!("Available MIDI devices:");
        for (i, device) in devices.iter().enumerate() {
            println!("{}: {}", i, device);
        }
        print!("Select device (0-{}): ", devices.len() - 1);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        selected_index = input.trim().parse().ok();
    }

    let device_index = match selected_index {
        Some(idx) if idx < devices.len() => idx,
        _ => {
            println!("Invalid device selection");
            return Ok(());
        }
    };

    println!("Connecting to: {}", devices[device_index]);

    let mut midi_handler = MidiInputHandler::new();
    midi_handler.connect_device(device_index)?;
    println!("MIDI connected successfully");
    println!();

    // Create synth
    let mut synth = Synth::new();

    // Setup audio streaming
    println!("Starting audio stream...");
    let stream_controller = StreamController::play(runtime)?;

    // Setup graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })?;

    println!("Synthesizer running! Play notes on your MIDI keyboard.");
    println!("Active voices: 0");
    println!("Press Ctrl+C to exit");
    println!();

    // Main loop
    while running.load(Ordering::Relaxed) {
        // Handle MIDI events
        while let Some(event) = midi_handler.try_recv() {
            synth.handle_midi_event(event);
        }

        // Process synth messages (simplified - in real implementation this would be RT-safe communication)
        synth.process_messages();

        // Update display
        let active_voices = synth.voice_allocator.active_voice_count();
        print!("\rActive voices: {} ", active_voices);
        io::stdout().flush()?;

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    println!();
    println!("Shutting down...");
    stream_controller.stop();
    midi_handler.disconnect();

    println!("Goodbye!");
    Ok(())
}
