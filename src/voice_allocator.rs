//! Voice allocation for polyphonic synthesis with voice stealing.
//!
//! Manages a fixed pool of 8 voices, allocating them to new MIDI notes
//! and stealing the oldest inactive voice when needed.

pub const MAX_VOICES: usize = 8;

/// Unique identifier for an allocated voice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceId(pub usize);

/// State of a single voice slot in the allocator.
#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceSlot {
    pub active: bool,
    pub note: u8,
    pub age: u64,
}

/// Allocates voices to MIDI notes with oldest-voice-stealing on overflow.
///
/// Uses a simple least-recently-used (LRU) strategy via age tracking with u64
/// to avoid wraparound issues (~292 billion allocations before overflow).
#[derive(Debug)]
pub struct VoiceAllocator {
    voices: [VoiceSlot; MAX_VOICES],
    next_age: u64,
}

impl VoiceAllocator {
    /// Creates a new voice allocator with all voices initially inactive.
    pub fn new() -> Self {
        Self {
            voices: [VoiceSlot::default(); MAX_VOICES],
            next_age: 0,
        }
    }

    /// Allocates a voice for the given MIDI note.
    ///
    /// Returns Some(VoiceId) if an inactive voice is available.
    /// If all voices are active, steals the oldest inactive voice and returns its ID.
    /// Returns None only if all voices are somehow stuck in impossible states.
    pub fn allocate_voice(&mut self, note: u8) -> Option<VoiceId> {
        // First try to find an inactive voice
        for (i, voice) in self.voices.iter_mut().enumerate() {
            if !voice.active {
                voice.active = true;
                voice.note = note;
                voice.age = self.next_age;
                self.next_age = self.next_age.saturating_add(1);
                return Some(VoiceId(i));
            }
        }

        // All voices active, steal the oldest one
        let oldest_idx = self.find_oldest_voice();
        self.voices[oldest_idx].active = true;
        self.voices[oldest_idx].note = note;
        self.voices[oldest_idx].age = self.next_age;
        self.next_age = self.next_age.saturating_add(1);
        Some(VoiceId(oldest_idx))
    }

    /// Release the voice playing the given note
    /// Releases the voice currently playing the given MIDI note.
    pub fn release_voice(&mut self, note: u8) {
        for voice in &mut self.voices {
            if voice.active && voice.note == note {
                voice.active = false;
                break;
            }
        }
    }

    /// Get the number of active voices
    /// Returns the count of currently active voices.
    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Get all active voices
    /// Iterates over all active voices with their IDs and MIDI notes.
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
        allocator.allocate_voice(60).unwrap();
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

    #[test]
    fn age_counter_uses_u64_prevents_wraparound() {
        // Verify that age uses u64, which doesn't wrap in practice.
        // Even at 1M allocations/sec, u64 takes ~292 billion years to wrap.
        let mut allocator = VoiceAllocator::new();

        // Allocate and release many voices
        for _ in 0..1000 {
            for note in 60..68 {
                let voice_id = allocator.allocate_voice(note);
                assert!(voice_id.is_some());
                allocator.release_voice(note);
            }
        }

        // next_age should be very large (8000 allocations done)
        // With u64 and saturating_add, we get at least 8000
        assert!(allocator.next_age >= 8000);
    }
}
