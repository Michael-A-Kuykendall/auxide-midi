//! Voice state for polyphonic synthesis

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceState {
    pub osc_phase: f32,
    pub filter_z1: f32,
    pub filter_z2: f32,
    pub env_stage: EnvStage,
    pub env_level: f32,
    pub note: u8,
    pub velocity: u8,
    pub active: bool,
}

impl VoiceState {
    pub fn new() -> Self {
        Self {
            osc_phase: 0.0,
            filter_z1: 0.0,
            filter_z2: 0.0,
            env_stage: EnvStage::Idle,
            env_level: 0.0,
            note: 0,
            velocity: 0,
            active: false,
        }
    }

    pub fn reset(&mut self) {
        self.osc_phase = 0.0;
        self.filter_z1 = 0.0;
        self.filter_z2 = 0.0;
        self.env_stage = EnvStage::Idle;
        self.env_level = 0.0;
        self.active = false;
    }

    pub fn trigger(&mut self, note: u8, velocity: u8) {
        self.note = note;
        self.velocity = velocity;
        self.env_stage = EnvStage::Attack;
        self.env_level = 0.0;
        self.active = true;
    }

    pub fn release(&mut self) {
        if self.active {
            self.env_stage = EnvStage::Release;
        }
    }
}

impl Default for VoiceState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct VoicePool {
    voices: [VoiceState; 8],
}

impl VoicePool {
    pub fn new() -> Self {
        Self {
            voices: [VoiceState::new(); 8],
        }
    }

    pub fn get_voice(&self, voice_id: usize) -> &VoiceState {
        &self.voices[voice_id]
    }

    pub fn get_voice_mut(&mut self, voice_id: usize) -> &mut VoiceState {
        &mut self.voices[voice_id]
    }

    pub fn voices(&self) -> &[VoiceState; 8] {
        &self.voices
    }

    pub fn voices_mut(&mut self) -> &mut [VoiceState; 8] {
        &mut self.voices
    }

    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}

impl Default for VoicePool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_of_voice_state_le_64() {
        // VoiceState should fit in a cache line (64 bytes)
        assert!(std::mem::size_of::<VoiceState>() <= 64);
    }

    #[test]
    fn voice_pool_has_8_voices() {
        let pool = VoicePool::new();
        assert_eq!(pool.voices().len(), 8);
    }

    #[test]
    fn voice_trigger_sets_active() {
        let mut voice = VoiceState::new();
        voice.trigger(60, 100);

        assert!(voice.active);
        assert_eq!(voice.note, 60);
        assert_eq!(voice.velocity, 100);
        assert_eq!(voice.env_stage, EnvStage::Attack);
    }

    #[test]
    fn voice_release_sets_release_stage() {
        let mut voice = VoiceState::new();
        voice.trigger(60, 100);
        voice.release();

        assert_eq!(voice.env_stage, EnvStage::Release);
        assert!(voice.active); // Still active until envelope finishes
    }

    #[test]
    fn voice_reset_clears_state() {
        let mut voice = VoiceState::new();
        voice.trigger(60, 100);
        voice.osc_phase = 0.5;
        voice.reset();

        assert!(!voice.active);
        assert_eq!(voice.env_stage, EnvStage::Idle);
        assert_eq!(voice.osc_phase, 0.0);
    }
}