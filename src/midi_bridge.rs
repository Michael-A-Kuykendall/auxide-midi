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
use auxide_dsp::Sampler;
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
/// `Sampler` graph built by [`build_rompler_graph`]).
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
    /// Per-voice node IDs in the runtime graph (e.g. Sampler nodes).
    voice_nodes: Vec<NodeId>,
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

    /// Attach a runtime to drive. `voice_nodes` are the per-voice node IDs
    /// (e.g. `Sampler` nodes); `filter_node` is an optional global filter node.
    pub fn with_runtime(
        mut self,
        runtime: RuntimeHandle,
        control: RuntimeControl,
        voice_nodes: Vec<NodeId>,
        filter_node: Option<NodeId>,
    ) -> Self {
        self.runtime = Some(runtime);
        self.control = Some(control);
        self.voice_nodes = voice_nodes;
        self.filter_node = filter_node;
        self
    }

    /// Start a note: allocate a voice and articulate the runtime node.
    pub fn route_note_on(&mut self, note: u8, velocity: u8) {
        if let Some(voice_id) = self.voice_allocator.allocate_voice(note) {
            self.voice_pool
                .get_voice_mut(voice_id.0)
                .trigger(note, velocity);
            if let (Some(control), Some(node)) =
                (self.control.as_mut(), self.voice_nodes.get(voice_id.0))
            {
                let _ = control.send(auxide::control::ControlMsg::SetFrequency {
                    node: *node,
                    hz: crate::conversions::note_to_freq(note),
                });
                let _ = control.send(auxide::control::ControlMsg::TriggerGate {
                    node: *node,
                    on: true,
                });
            }
        }
    }

    /// Stop a note: release the voice and silence the runtime node.
    pub fn route_note_off(&mut self, note: u8) {
        for (voice_id, n) in self.voice_allocator.active_voices() {
            if n == note {
                self.voice_pool.get_voice_mut(voice_id.0).release();
                if let (Some(control), Some(node)) =
                    (self.control.as_mut(), self.voice_nodes.get(voice_id.0))
                {
                    let _ = control.send(auxide::control::ControlMsg::TriggerGate {
                        node: *node,
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

/// Build a polyphonic ROMpler graph: `num_voices` `Sampler` voices summed into a
/// `Mix` node and out to `OutputSink`. Each Sampler plays `sample` (recorded at
/// `file_sample_rate`, representing MIDI note `anchor_note`).
///
/// Returns `(graph, plan, voice_node_ids)`.
pub fn build_rompler_graph(
    num_voices: usize,
    sample: Arc<Vec<f32>>,
    file_sample_rate: f32,
    anchor_note: u8,
) -> (Graph, Plan, Vec<NodeId>) {
    let mut graph = Graph::new();
    let mut voice_nodes = Vec::with_capacity(num_voices);
    for _ in 0..num_voices {
        let node = graph.add_external_node(Sampler::new(
            sample.clone(),
            file_sample_rate,
            anchor_note,
            false,
        ));
        voice_nodes.push(node);
    }
    // Multi-input summing bus (the kernel Mix node has a single input port and
    // would violate the single-writer rule with N voices).
    let mix = graph.add_external_node(auxide_dsp::Mixer::new(num_voices));
    let sink = graph.add_node(NodeType::OutputSink);
    for (i, &v) in voice_nodes.iter().enumerate() {
        graph
            .add_edge(Edge {
                from_node: v,
                from_port: PortId(0),
                to_node: mix,
                to_port: PortId(i),
                rate: Rate::Audio,
            })
            .unwrap();
    }
    graph
        .add_edge(Edge {
            from_node: mix,
            from_port: PortId(0),
            to_node: sink,
            to_port: PortId(0),
            rate: Rate::Audio,
        })
        .unwrap();
    let plan = Plan::compile(&graph, 64).unwrap();
    (graph, plan, voice_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn runtime_path_note_on_produces_audio() {
        let sample = make_sample(440.0, 0.3, 44100.0);
        let (graph, plan, voice_nodes) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, None);

        bridge.route_note_on(69, 100);
        let mut out = vec![0.0; 64];
        let mut all = Vec::new();
        for _ in 0..40 {
            bridge.process_block(&mut out).unwrap();
            all.extend_from_slice(&out);
        }
        assert!(
            all.iter().any(|&x| x.abs() > 0.1),
            "note_on must produce audio"
        );
        let f = zc_freq(&all, 44100.0);
        assert!((f - 440.0).abs() < 60.0, "expected ~440 Hz, got {f}");
    }

    #[test]
    fn runtime_path_note_off_silences() {
        let sample = make_sample(440.0, 0.05, 44100.0);
        let (graph, plan, voice_nodes) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, None);

        bridge.route_note_on(69, 100);
        let mut out = vec![0.0; 64];
        for _ in 0..10 {
            bridge.process_block(&mut out).unwrap();
        }
        bridge.route_note_off(69);
        // one-shot sample is short; run past it
        for _ in 0..30 {
            bridge.process_block(&mut out).unwrap();
        }
        assert!(
            out.iter().all(|&x| x.abs() < 1e-3),
            "note_off must silence the voice"
        );
    }

    #[test]
    fn runtime_path_polyphony() {
        let sample = make_sample(440.0, 1.0, 44100.0);
        let (graph, plan, voice_nodes) = build_rompler_graph(8, sample, 44100.0, 69);
        let (handle, control) = RuntimeCore::new_with_channels(plan, &graph, 44100.0);
        let mut bridge = MidiToAudioBridge::new_without_device(MidiBridgeConfig::default())
            .with_runtime(handle, control, voice_nodes, None);

        for note in [60u8, 64, 67, 69, 72, 76, 79, 81] {
            bridge.route_note_on(note, 100);
        }
        assert_eq!(bridge.active_voice_count(), 8, "8 voices active");
        // A 9th voice steals the oldest (VoiceAllocator behavior).
        bridge.route_note_on(84, 100);
        assert_eq!(bridge.active_voice_count(), 8, "still 8 (steal)");
    }
}
