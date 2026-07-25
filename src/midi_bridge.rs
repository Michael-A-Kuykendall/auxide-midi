//! MIDI to Audio bridge - translates MIDI events into real-time audio parameter changes
//!
//! This module provides the glue between MIDI input and Auxide runtime state updates.
//! It handles voice allocation, parameter routing, and smoothing.
//!
//! ## Runtime-controlled path
//!
//! `MidiToAudioBridge` can be attached to a `RuntimeHandle` + `RuntimeControl`
//! (from `auxide::rt`). When attached, every MIDI note/CC is translated into a
//! lock-free `ControlMsg` that drives the actual audio graph — e.g. a polyphonic
//! bank of `auxide_dsp::Sampler` voices (see `build_rompler_graph`). This is what
//! makes the synth *actually audible* (bead `midi-u4h.1` / `auxide-midi-8la`).

use crate::{MidiEvent, MidiInputHandler, ParamSmoother, ParamTarget, VoiceAllocator, VoicePool};
use auxide::graph::{Edge, Graph, NodeId, NodeType, PortId, Rate};
use auxide::plan::Plan;
use auxide::rt::{RuntimeControl, RuntimeHandle};
use auxide_dsp::nodes::{AdsrEnvelope, Multiply, Sampler, SvfFilter, SvfMode};
use std::sync::Arc;

/// Configuration for MIDI to audio bridge
#[derive(Clone, Debug)]
pub struct MidiBridgeConfig {
    /// Which CC controls map to which parameters
    pub cc_mappings: std::collections::HashMap<u8, ParamTarget>,
    /// Parameter smoothing time in milliseconds
    pub smoothing_ms: f32,
}

impl Default for MidiBridgeConfig {
    fn default() -> Self {
        let mut cc_mappings = std::collections::HashMap::new();
        // CC#74 = Brightness → Filter Cutoff (standard MIDI mapping)
        cc_mappings.insert(74, ParamTarget::FilterCutoff);

        Self {
            cc_mappings,
            smoothing_ms: 20.0,
        }
    }
}

/// MIDI to Audio bridge
///
/// Handles the complete flow: MIDI input → voice allocation → parameter updates → Runtime.
///
/// When constructed with [`MidiToAudioBridge::with_runtime`], note/CC events are
/// translated into `ControlMsg`s that drive a `RuntimeHandle` (e.g. a polyphonic
/// `Sampler` + `AdsrEnvelope` graph built by [`build_rompler_graph`]).
pub struct MidiToAudioBridge {
    midi_handler: MidiInputHandler,
    voice_allocator: VoiceAllocator,
    voice_pool: VoicePool,
    parameter_smoothers: std::collections::HashMap<ParamTarget, ParamSmoother>,
    cc_mappings: std::collections::HashMap<u8, ParamTarget>,
    last_pitch_bend: f32,
    /// Optional attached runtime (the audio graph we drive).
    runtime: Option<RuntimeHandle>,
    /// Optional control endpoint for the attached runtime.
    control: Option<RuntimeControl>,
    /// Per-voice node IDs: `(oscillator_node, envelope_node)`.
    voice_nodes: Vec<(NodeId, NodeId)>,
    /// Optional global filter node ID (driven by CC).
    filter_node: Option<NodeId>,
}

impl MidiToAudioBridge {
    /// Create a new MIDI to audio bridge connected to a real device.
    pub fn new(
        midi_device_index: usize,
        config: MidiBridgeConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut midi_handler = MidiInputHandler::new();
        midi_handler.connect_device(midi_device_index)?;

        Ok(Self::assemble(config, midi_handler))
    }

    /// Create a bridge with NO MIDI device (headless / programmatic control).
    ///
    /// Useful for tests and for driving the synth directly via [`Self::route_note_on`].
    pub fn new_without_device(config: MidiBridgeConfig) -> Self {
        Self::assemble(config, MidiInputHandler::new())
    }

    fn assemble(config: MidiBridgeConfig, midi_handler: MidiInputHandler) -> Self {
        let voice_allocator = VoiceAllocator::new();
        let voice_pool = VoicePool::new();

        let mut parameter_smoothers = std::collections::HashMap::new();
        let smoother_time_const = config.smoothing_ms / 1000.0;
        parameter_smoothers.insert(
            ParamTarget::FilterCutoff,
            ParamSmoother::with_time_constant(smoother_time_const, 44100.0),
        );

        Self {
            midi_handler,
            voice_allocator,
            voice_pool,
            parameter_smoothers,
            cc_mappings: config.cc_mappings,
            last_pitch_bend: 1.0,
            runtime: None,
            control: None,
            voice_nodes: Vec::new(),
            filter_node: None,
        }
    }

    /// Attach a runtime to drive. `voice_nodes` is a per-voice list of
    /// `(oscillator_node, envelope_node)` pairs; `filter_node` is an optional
    /// global filter node.
    pub fn with_runtime(
        mut self,
        runtime: RuntimeHandle,
        control: RuntimeControl,
        voice_nodes: Vec<(NodeId, NodeId)>,
        filter_node: Option<NodeId>,
    ) -> Self {
        self.runtime = Some(runtime);
        self.control = Some(control);
        self.voice_nodes = voice_nodes;
        self.filter_node = filter_node;
        self
    }

    /// Start a note: allocate a voice, start oscillator and open envelope gate.
    pub fn route_note_on(&mut self, note: u8, velocity: u8) {
        if let Some(voice_id) = self.voice_allocator.allocate_voice(note) {
            self.voice_pool
                .get_voice_mut(voice_id.0)
                .trigger(note, velocity);
            if let (Some(control), Some(&(osc_node, env_node))) =
                (self.control.as_mut(), self.voice_nodes.get(voice_id.0))
            {
                let _ = control.send(auxide::control::ControlMsg::SetFrequency {
                    node: osc_node,
                    hz: crate::conversions::note_to_freq(note),
                });
                let _ = control.send(auxide::control::ControlMsg::TriggerGate {
                    node: osc_node,
                    on: true,
                });
                let _ = control.send(auxide::control::ControlMsg::TriggerGate {
                    node: env_node,
                    on: true,
                });
            }
        }
    }

    /// Stop a note: release the voice and close the envelope gate (release phase).
    ///
    /// The oscillator keeps looping so the envelope's release ramp is audible;
    /// the Multiply node combines them. On voice steal the new note retriggers
    /// both oscillator and envelope via `route_note_on`.
    pub fn route_note_off(&mut self, note: u8) {
        for (voice_id, n) in self.voice_allocator.active_voices() {
            if n == note {
                self.voice_pool.get_voice_mut(voice_id.0).release();
                if let (Some(control), Some(&(_, env_node))) =
                    (self.control.as_mut(), self.voice_nodes.get(voice_id.0))
                {
                    let _ = control.send(auxide::control::ControlMsg::TriggerGate {
                        node: env_node,
                        on: false,
                    });
                }
            }
        }
        self.voice_allocator.release_voice(note);
    }

    /// Poll MIDI events and update internal state (and the attached runtime).
    pub fn poll(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(event) = self.midi_handler.try_recv() {
            match event {
                MidiEvent::NoteOn(note, velocity) => self.route_note_on(note, velocity),
                MidiEvent::NoteOff(note, _) => self.route_note_off(note),
                MidiEvent::ControlChange(cc, value) => {
                    if let Some(target) = self.cc_mappings.get(&cc) {
                        if let Some(smoother) = self.parameter_smoothers.get_mut(target) {
                            let normalized = (value as f32) / 127.0;
                            let param_value = match target {
                                ParamTarget::FilterCutoff => normalized * 5000.0 + 100.0,
                                _ => normalized,
                            };
                            smoother.set_target(param_value);
                            if let (Some(control), Some(filter)) =
                                (self.control.as_mut(), self.filter_node)
                            {
                                let _ =
                                    control.send(auxide::control::ControlMsg::SetFilterCutoff {
                                        node: filter,
                                        hz: param_value,
                                    });
                            }
                        }
                    }
                }
                MidiEvent::PitchBend(bend) => {
                    self.last_pitch_bend = bend as f32 / 8192.0;
                }
                // Transport / clock messages do not directly drive voices.
                MidiEvent::Clock
                | MidiEvent::Start
                | MidiEvent::Continue
                | MidiEvent::Stop
                | MidiEvent::SongPosition(_) => {}
            }
        }

        for smoother in self.parameter_smoothers.values_mut() {
            smoother.next_sample();
        }

        Ok(())
    }

    /// Render one block of audio through the attached runtime (if any).
    pub fn process_block(&mut self, out: &mut [f32]) -> Result<(), &'static str> {
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.process_block(out)
        } else {
            out.fill(0.0);
            Ok(())
        }
    }

    /// Send a CC value directly (bypass MIDI hardware, useful for tests).
    pub fn send_cc(&mut self, cc: u8, value: u8) -> Option<f32> {
        let target = *self.cc_mappings.get(&cc)?;
        let normalized = (value as f32) / 127.0;
        let param_value = match target {
            ParamTarget::FilterCutoff => normalized * 5000.0 + 100.0,
            _ => normalized,
        };
        if let Some(smoother) = self.parameter_smoothers.get_mut(&target) {
            smoother.set_target(param_value);
        }
        if matches!(target, ParamTarget::FilterCutoff) {
            if let (Some(control), Some(filter)) = (self.control.as_mut(), self.filter_node) {
                let _ = control.send(auxide::control::ControlMsg::SetFilterCutoff {
                    node: filter,
                    hz: param_value,
                });
            }
        }
        Some(param_value)
    }

    /// Get current smoothed value for a parameter
    pub fn get_parameter(&self, target: ParamTarget) -> Option<f32> {
        self.parameter_smoothers
            .get(&target)
            .map(|s| s.current_value())
    }

    /// Get active voice count
    pub fn active_voice_count(&self) -> usize {
        self.voice_allocator.active_voice_count()
    }

    /// Get reference to voice pool for envelope/state access
    pub fn voice_pool(&self) -> &VoicePool {
        &self.voice_pool
    }

    /// Get mutable reference to voice pool
    pub fn voice_pool_mut(&mut self) -> &mut VoicePool {
        &mut self.voice_pool
    }
}

/// Build a polyphonic ROMpler graph with ADSR shaping and a global filter.
///
/// Per voice: `Sampler → Multiply ← AdsrEnvelope`.  The 8 voices sum through a
/// `Mixer`, then pass through a `SvfFilter` (lowpass, 10 kHz) to `OutputSink`.
///
/// Returns `(graph, plan, voice_node_pairs, filter_node)` where each element of
/// `voice_node_pairs` is `(oscillator_node_id, envelope_node_id)`.
pub fn build_rompler_graph(
    num_voices: usize,
    sample: Arc<Vec<f32>>,
    file_sample_rate: f32,
    anchor_note: u8,
) -> (Graph, Plan, Vec<(NodeId, NodeId)>, Option<NodeId>) {
    let mut graph = Graph::new();
    // Triples during construction; last step drops the multiply node ID.
    let mut triples: Vec<(NodeId, NodeId, NodeId)> = Vec::with_capacity(num_voices);

    for _ in 0..num_voices {
        let osc = graph.add_external_node(Sampler::new(
            sample.clone(),
            file_sample_rate,
            anchor_note,
            true, // loop mode — sustain while gate is held
        ));
        let env = graph.add_external_node(AdsrEnvelope {
            attack_ms: 10.0,
            decay_ms: 50.0,
            sustain_level: 0.7,
            release_ms: 200.0,
            curve: 2.0,
        });
        let mul = graph.add_external_node(Multiply);
        graph
            .add_edge(Edge {
                from_node: osc,
                from_port: PortId(0),
                to_node: mul,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();
        graph
            .add_edge(Edge {
                from_node: env,
                from_port: PortId(0),
                to_node: mul,
                to_port: PortId(1),
                rate: Rate::Audio,
            })
            .unwrap();
        triples.push((osc, env, mul));
    }

    // Multi-input summing bus
    let mix = graph.add_external_node(auxide_dsp::Mixer::new(num_voices));
    for (i, &(_, _, mul)) in triples.iter().enumerate() {
        graph
            .add_edge(Edge {
                from_node: mul,
                from_port: PortId(0),
                to_node: mix,
                to_port: PortId(i),
                rate: Rate::Audio,
            })
            .unwrap();
    }

    // Global lowpass filter after the mixer
    let filter = graph.add_external_node(SvfFilter {
        cutoff: 10000.0,
        resonance: 0.3,
        mode: SvfMode::Lowpass,
    });

    graph
        .add_edge(Edge {
            from_node: mix,
            from_port: PortId(0),
            to_node: filter,
            to_port: PortId(0),
            rate: Rate::Audio,
        })
        .unwrap();

    let sink = graph.add_node(NodeType::OutputSink);
    graph
        .add_edge(Edge {
            from_node: filter,
            from_port: PortId(0),
            to_node: sink,
            to_port: PortId(0),
            rate: Rate::Audio,
        })
        .unwrap();

    let plan = Plan::compile(&graph, 64).unwrap();
    let voice_pairs: Vec<(NodeId, NodeId)> = triples
        .into_iter()
        .map(|(osc, env, _)| (osc, env))
        .collect();
    (graph, plan, voice_pairs, Some(filter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use auxide::control::ControlMsg;
    use auxide::rt::RuntimeCore;

    /// Generate a mono sine sample buffer.
    fn make_sample(freq: f32, dur_s: f32, sr: f32) -> Arc<Vec<f32>> {
        let n = (dur_s * sr) as usize;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            v.push((2.0 * std::f32::consts::PI * freq * (i as f32) / sr).sin());
        }
        Arc::new(v)
    }

    #[allow(dead_code)]
    fn zc_freq(out: &[f32], sr: f32) -> f32 {
        let mut zc = 0u32;
        for w in out.windows(2) {
            if (w[0] <= 0.0 && w[1] > 0.0) || (w[0] >= 0.0 && w[1] < 0.0) {
                zc += 1;
            }
        }
        (zc as f32) / 2.0 / (out.len() as f32 / sr)
    }

    #[test]
    fn adsr_envelope_gate_triggers_attack() {
        // Isolate the envelope: verify gate(true) transitions phase to Attack
        // and process_block produces rising level.
        let mut graph = Graph::new();
        let env = graph.add_external_node(AdsrEnvelope {
            attack_ms: 10.0,
            decay_ms: 50.0,
            sustain_level: 0.7,
            release_ms: 200.0,
            curve: 0.0,
        });
        let sink = graph.add_node(NodeType::OutputSink);
        graph
            .add_edge(Edge {
                from_node: env,
                from_port: PortId(0),
                to_node: sink,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();
        let plan = Plan::compile(&graph, 64).unwrap();
        let (mut handle, mut control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);

        control
            .send(ControlMsg::TriggerGate {
                node: env,
                on: true,
            })
            .unwrap();
        let mut out = vec![0.0; 64];
        handle.process_block(&mut out).unwrap();
        // After 1 block (~1.45 ms of 10 ms attack), the envelope should
        // have risen noticeably from zero.
        let first = out[0];
        let last = out[out.len() - 1];
        assert!(
            last > 0.01,
            "envelope should rise during attack: first={first}, last={last}"
        );
        assert!(
            last > first,
            "envelope should be increasing during attack: first={first}, last={last}"
        );
    }

    #[test]
    fn single_voice_sampler_env_mul_chain() {
        // One voice: Sampler -> Multiply <- AdsrEnv -> filter -> sink
        let sample = make_sample(440.0, 1.0, 44100.0);
        let mut graph = Graph::new();
        let osc = graph.add_external_node(Sampler::new(sample, 44100.0, 69, true));
        let env = graph.add_external_node(AdsrEnvelope {
            attack_ms: 5.0,
            decay_ms: 20.0,
            sustain_level: 0.7,
            release_ms: 100.0,
            curve: 0.0,
        });
        let mul = graph.add_external_node(Multiply);
        graph
            .add_edge(Edge {
                from_node: osc,
                from_port: PortId(0),
                to_node: mul,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();
        graph
            .add_edge(Edge {
                from_node: env,
                from_port: PortId(0),
                to_node: mul,
                to_port: PortId(1),
                rate: Rate::Audio,
            })
            .unwrap();
        let filter = graph.add_external_node(SvfFilter {
            cutoff: 10000.0,
            resonance: 0.0,
            mode: SvfMode::Lowpass,
        });
        graph
            .add_edge(Edge {
                from_node: mul,
                from_port: PortId(0),
                to_node: filter,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();
        let sink = graph.add_node(NodeType::OutputSink);
        graph
            .add_edge(Edge {
                from_node: filter,
                from_port: PortId(0),
                to_node: sink,
                to_port: PortId(0),
                rate: Rate::Audio,
            })
            .unwrap();
        let plan = Plan::compile(&graph, 64).unwrap();
        let (mut handle, mut control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);

        control
            .send(ControlMsg::SetFrequency {
                node: osc,
                hz: 440.0,
            })
            .unwrap();
        control
            .send(ControlMsg::TriggerGate {
                node: osc,
                on: true,
            })
            .unwrap();
        control
            .send(ControlMsg::TriggerGate {
                node: env,
                on: true,
            })
            .unwrap();

        let mut out = vec![0.0; 64];
        let mut all = Vec::new();
        for _ in 0..30 {
            handle.process_block(&mut out).unwrap();
            all.extend_from_slice(&out);
        }
        // sampler * envelope should produce non-zero audio
        assert!(
            all.iter().any(|&x| x.abs() > 0.01),
            "single voice chain should produce audio"
        );
    }

    #[test]
    fn rompler_graph_direct_control() {
        // Build the full ROMpler graph and drive it directly via RuntimeControl,
        // bypassing MidiToAudioBridge entirely.
        let sample = make_sample(440.0, 1.0, 44100.0);
        let (graph, plan, voice_pairs, _) = build_rompler_graph(8, sample, 44100.0, 69);
        let (mut handle, mut control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);

        // Drive voice 0 directly
        let (osc0, env0) = voice_pairs[0];
        control
            .send(ControlMsg::SetFrequency {
                node: osc0,
                hz: 440.0,
            })
            .unwrap();
        control
            .send(ControlMsg::TriggerGate {
                node: osc0,
                on: true,
            })
            .unwrap();
        control
            .send(ControlMsg::TriggerGate {
                node: env0,
                on: true,
            })
            .unwrap();

        let mut out = vec![0.0; 64];
        let mut all = Vec::new();
        for _ in 0..30 {
            handle.process_block(&mut out).unwrap();
            all.extend_from_slice(&out);
        }
        let peak = all.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            peak > 0.01,
            "direct-drive ROMpler should produce audio, peak={peak}"
        );
    }
    #[test]
    fn bridge_routes_to_correct_voice() {
        let sample = make_sample(440.0, 1.0, 44100.0);
        let (graph, plan, voice_nodes, filter_node) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, filter_node);

        assert_eq!(bridge.voice_nodes.len(), 8, "should have 8 voice pairs");
        bridge.route_note_on(69, 100);
        assert_eq!(bridge.active_voice_count(), 1, "1 voice active");

        let mut out = vec![0.0; 64];
        let mut all = Vec::new();
        for _ in 0..30 {
            bridge.process_block(&mut out).unwrap();
            all.extend_from_slice(&out);
        }
        let peak = all.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "bridge should produce audio, peak={peak}");
    }

    #[test]
    fn runtime_path_note_on_produces_audio() {
        let sample = make_sample(440.0, 1.0, 44100.0);
        let (graph, plan, voice_nodes, filter_node) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, filter_node);

        bridge.route_note_on(69, 100);
        let mut out = vec![0.0; 64];
        let mut peaks = Vec::new();
        for _ in 0..60 {
            bridge.process_block(&mut out).unwrap();
            peaks.push(out.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
        }
        let overall = peaks.iter().copied().fold(0.0f32, f32::max);
        assert!(overall > 0.1, "note_on must produce audio, peak={overall}");
        let late = &peaks[20..];
        let late_peak = late.iter().copied().fold(0.0f32, f32::max);
        assert!(late_peak > 0.1, "sustain amplitude > 0.1, got {late_peak}");
    }

    #[test]
    fn runtime_path_note_off_release_decays_to_zero() {
        let sample = make_sample(440.0, 2.0, 44100.0);
        let (graph, plan, voice_nodes, filter_node) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, filter_node);

        bridge.route_note_on(69, 100);
        let mut out = vec![0.0; 64];
        // 60 blocks (~87 ms) — well past attack+decay (60 ms), into sustain
        for _ in 0..60 {
            bridge.process_block(&mut out).unwrap();
        }
        let sustain_peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            sustain_peak > 0.1,
            "should sustain after attack+decay, peak={sustain_peak}"
        );

        bridge.route_note_off(69);
        // 140 blocks (~203 ms) — longer than 200 ms release
        for _ in 0..140 {
            bridge.process_block(&mut out).unwrap();
        }
        let final_peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            final_peak < 1e-3,
            "note_off ADSR release must decay to ~0, final_peak={final_peak}"
        );
    }

    #[test]
    fn runtime_path_polyphony() {
        let sample = make_sample(440.0, 2.0, 44100.0);
        let (graph, plan, voice_nodes, filter_node) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, filter_node);

        for note in [60u8, 64, 67, 69, 72, 76, 79, 81] {
            bridge.route_note_on(note, 100);
        }
        assert_eq!(bridge.active_voice_count(), 8, "8 voices active");
        bridge.route_note_on(84, 100);
        assert_eq!(bridge.active_voice_count(), 8, "still 8 (steal)");
    }

    #[test]
    fn cc_moves_cutoff() {
        // CC74 (brightness) should lower the audible filter cutoff.
        // With cutoff at 10 kHz almost no energy is filtered from a 440 Hz
        // tone. After CC74=0 (minimum) the cutoff drops to ~100 Hz,
        // which should attenuate the 440 Hz fundamental significantly.
        let sample = make_sample(440.0, 2.0, 44100.0);
        let (graph, plan, voice_nodes, filter_node) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, filter_node);

        bridge.route_note_on(69, 100);
        let mut out = vec![0.0; 64];
        let mut before = Vec::new();
        for _ in 0..20 {
            bridge.process_block(&mut out).unwrap();
            before.extend_from_slice(&out);
        }
        let peak_before = before.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

        // CC74 = 0 → filter cutoff near 100 Hz, should heavily attenuate 440 Hz
        bridge.send_cc(74, 0);
        let mut after = Vec::new();
        for _ in 0..40 {
            bridge.process_block(&mut out).unwrap();
            after.extend_from_slice(&out);
        }
        // Use last 10 blocks (after filter transient settles) for comparison
        let late = &after[after.len() - 64 * 10..];
        let late_peak = late.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

        assert!(
            late_peak < peak_before * 0.5,
            "CC74=0 should attenuate 440 Hz: peak_before={peak_before}, late_peak={late_peak}"
        );
    }
}
