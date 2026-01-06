//! Tests for voice allocator

use auxide_midi::{VoiceAllocator, VoiceId};
use proptest::prelude::*;

#[test]
fn allocate_single_voice() {
    let mut allocator = VoiceAllocator::new();
    let voice_id = allocator.allocate_voice(60).unwrap();
    assert_eq!(voice_id.0, 0);
    assert_eq!(allocator.active_voice_count(), 1);
}

#[test]
fn allocate_multiple_voices() {
    let mut allocator = VoiceAllocator::new();

    for i in 0..8 {
        let voice_id = allocator.allocate_voice(60 + i as u8).unwrap();
        assert_eq!(voice_id.0, i);
    }

    assert_eq!(allocator.active_voice_count(), 8);
}

#[test]
fn release_voice() {
    let mut allocator = VoiceAllocator::new();
    let voice_id = allocator.allocate_voice(60).unwrap();
    assert_eq!(allocator.active_voice_count(), 1);

    allocator.release_voice(60);
    assert_eq!(allocator.active_voice_count(), 0);
}

#[test]
fn voice_stealing_works() {
    let mut allocator = VoiceAllocator::new();

    // Fill all voices
    for i in 0..8 {
        allocator.allocate_voice(60 + i as u8).unwrap();
    }

    // Try to allocate one more - should steal oldest (voice 0)
    let stolen_voice = allocator.allocate_voice(100).unwrap();
    assert_eq!(stolen_voice.0, 0);
    assert_eq!(allocator.active_voice_count(), 8);
}

#[test]
fn retrigger_same_note() {
    let mut allocator = VoiceAllocator::new();

    let voice1 = allocator.allocate_voice(60).unwrap();
    let voice2 = allocator.allocate_voice(60).unwrap();

    // Should get different voices
    assert_ne!(voice1.0, voice2.0);
    assert_eq!(allocator.active_voice_count(), 2);
}

#[test]
fn oldest_voice_stolen() {
    let mut allocator = VoiceAllocator::new();

    // Allocate voices in order
    for i in 0..8 {
        allocator.allocate_voice(60 + i as u8).unwrap();
    }

    // Allocate one more - should steal voice 0 (oldest)
    let stolen = allocator.allocate_voice(100).unwrap();
    assert_eq!(stolen.0, 0);
}

#[test]
fn active_voices_iteration() {
    let mut allocator = VoiceAllocator::new();

    allocator.allocate_voice(60).unwrap();
    allocator.allocate_voice(64).unwrap();

    let active: Vec<_> = allocator.active_voices().map(|(_, note)| note).collect();
    assert_eq!(active.len(), 2);
    assert!(active.contains(&60));
    assert!(active.contains(&64));
}

#[test]
fn release_nonexistent_voice() {
    let mut allocator = VoiceAllocator::new();
    allocator.allocate_voice(60).unwrap();

    // Release a note that wasn't allocated - should not crash
    allocator.release_voice(70);
    assert_eq!(allocator.active_voice_count(), 1);
}

proptest! {
    #[test]
    fn voice_allocator_no_panic_random_notes(notes in prop::collection::vec(0u8..128, 1..20)) {
        let mut allocator = VoiceAllocator::new();
        let mut allocations = std::collections::HashMap::new();

        for &note in &notes {
            if let Some(voice_id) = allocator.allocate_voice(note) {
                *allocations.entry(note).or_insert(0) += 1;
                // Voice ID should be valid
                prop_assert!(voice_id.0 < 8);
            }
        }

        // Active count should not exceed MAX_VOICES
        prop_assert!(allocator.active_voice_count() <= 8);

        // Release all allocated notes (once for each allocation)
        for (note, count) in allocations {
            for _ in 0..count {
                allocator.release_voice(note);
            }
        }
        prop_assert_eq!(allocator.active_voice_count(), 0);
    }

    #[test]
    fn voice_allocator_age_based_stealing(notes in prop::collection::vec(0u8..128, 9..20)) {
        let mut allocator = VoiceAllocator::new();

        // Allocate 8 voices
        for &note in &notes[..8] {
            allocator.allocate_voice(note).unwrap();
        }
        prop_assert_eq!(allocator.active_voice_count(), 8);

        // Allocate 9th - should steal oldest
        let stolen_voice = allocator.allocate_voice(notes[8]).unwrap();
        prop_assert_eq!(allocator.active_voice_count(), 8);
        // Should have stolen voice 0 (oldest)
        prop_assert_eq!(stolen_voice.0, 0);
    }
}