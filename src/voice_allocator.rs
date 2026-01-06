//! Voice allocation for polyphonic synthesis

pub const MAX_VOICES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceId(pub usize);

#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceSlot {
    pub active: bool,
    pub note: u8,
    pub age: u32,
}

#[derive(Debug)]
pub struct VoiceAllocator {
    voices: [VoiceSlot; MAX_VOICES],
    next_age: u32,
}

impl VoiceAllocator {
    pub fn new() -> Self {
        Self {
            voices: [VoiceSlot::default(); MAX_VOICES],
            next_age: 0,
        }
    }

    /// Allocate a voice for the given note
    /// Returns Some(VoiceId) if successful, None if all voices busy
    pub fn allocate_voice(&mut self, note: u8) -> Option<VoiceId> {
        // First try to find an inactive voice
        for (i, voice) in self.voices.iter_mut().enumerate() {
            if !voice.active {
                voice.active = true;
                voice.note = note;
                voice.age = self.next_age;
                self.next_age = self.next_age.wrapping_add(1);
                return Some(VoiceId(i));
            }
        }

        // All voices active, steal the oldest one
        let oldest_idx = self.find_oldest_voice();
        self.voices[oldest_idx].active = true;
        self.voices[oldest_idx].note = note;
        self.voices[oldest_idx].age = self.next_age;
        self.next_age = self.next_age.wrapping_add(1);
        Some(VoiceId(oldest_idx))
    }

    /// Release the voice playing the given note
    pub fn release_voice(&mut self, note: u8) {
        for voice in &mut self.voices {
            if voice.active && voice.note == note {
                voice.active = false;
                break;
            }
        }
    }

    /// Get the number of active voices
    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Get all active voices
    pub fn active_voices(&self) -> impl Iterator<Item = (VoiceId, u8)> + '_ {
        self.voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.active)
            .map(|(i, v)| (VoiceId(i), v.note))
    }

    fn find_oldest_voice(&self) -> usize {
        let mut oldest_idx = 0;
        let mut oldest_age = self.voices[0].age;

        for (i, voice) in self.voices.iter().enumerate() {
            if voice.age < oldest_age {
                oldest_age = voice.age;
                oldest_idx = i;
            }
        }

        oldest_idx
    }
}

impl Default for VoiceAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_becomes_available() {
        let mut allocator = VoiceAllocator::new();

        // Allocate a voice
        let voice_id = allocator.allocate_voice(60).unwrap();
        assert_eq!(allocator.active_voice_count(), 1);

        // Release it
        allocator.release_voice(60);
        assert_eq!(allocator.active_voice_count(), 0);
    }

    #[test]
    fn all_voices_busy_steals_oldest() {
        let mut allocator = VoiceAllocator::new();

        // Fill all voices
        for i in 0..MAX_VOICES {
            let voice_id = allocator.allocate_voice(60 + i as u8).unwrap();
            assert_eq!(voice_id.0, i);
        }
        assert_eq!(allocator.active_voice_count(), MAX_VOICES);

        // Try to allocate one more - should steal oldest (voice 0)
        let stolen_voice = allocator.allocate_voice(100).unwrap();
        assert_eq!(stolen_voice.0, 0); // Should steal voice 0
        assert_eq!(allocator.active_voice_count(), MAX_VOICES);
    }

    #[test]
    fn note_retriggering() {
        let mut allocator = VoiceAllocator::new();

        // Play same note twice
        let voice1 = allocator.allocate_voice(60).unwrap();
        let voice2 = allocator.allocate_voice(60).unwrap();

        // Should get different voices
        assert_ne!(voice1.0, voice2.0);
        assert_eq!(allocator.active_voice_count(), 2);
    }

    #[test]
    fn active_voices_iteration() {
        let mut allocator = VoiceAllocator::new();

        allocator.allocate_voice(60).unwrap();
        allocator.allocate_voice(64).unwrap();
        allocator.allocate_voice(67).unwrap();

        let active: Vec<_> = allocator.active_voices().map(|(_, note)| note).collect();
        assert_eq!(active.len(), 3);
        assert!(active.contains(&60));
        assert!(active.contains(&64));
        assert!(active.contains(&67));
    }
}
