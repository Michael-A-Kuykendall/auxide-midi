//! Tests for CC mapping

use auxide_midi::{CCMap, ParamTarget};

#[test]
fn default_cc1_maps_cutoff() {
    let map = CCMap::new();
    let result = map.map_cc(1, 64);
    assert_eq!(result, Some((ParamTarget::FilterCutoff, 64.0 / 127.0)));
}

#[test]
fn default_cc74_maps_resonance() {
    let map = CCMap::new();
    let result = map.map_cc(74, 100);
    assert_eq!(result, Some((ParamTarget::FilterResonance, 100.0 / 127.0)));
}

#[test]
fn unmapped_cc_returns_none() {
    let map = CCMap::new();
    let result = map.map_cc(42, 100);
    assert_eq!(result, None);
}

#[test]
fn cc_value_normalization() {
    let map = CCMap::new();

    // CC 0 should be 0.0
    let result = map.map_cc(1, 0);
    assert_eq!(result, Some((ParamTarget::FilterCutoff, 0.0)));

    // CC 127 should be 1.0
    let result = map.map_cc(1, 127);
    assert_eq!(result, Some((ParamTarget::FilterCutoff, 1.0)));

    // CC 63 should be approximately 0.5
    let result = map.map_cc(1, 63);
    assert!((result.unwrap().1 - 0.496).abs() < 0.01);
}

#[test]
fn set_mapping_works() {
    let mut map = CCMap::new();
    map.set_mapping(7, ParamTarget::AttackTime);

    let result = map.map_cc(7, 127);
    assert_eq!(result, Some((ParamTarget::AttackTime, 1.0)));
}

#[test]
fn set_mapping_overwrites_existing() {
    let mut map = CCMap::new();

    // Initially CC 1 maps to cutoff
    let result = map.map_cc(1, 100);
    assert_eq!(result, Some((ParamTarget::FilterCutoff, 100.0 / 127.0)));

    // Remap CC 1 to attack time
    map.set_mapping(1, ParamTarget::AttackTime);

    let result = map.map_cc(1, 100);
    assert_eq!(result, Some((ParamTarget::AttackTime, 100.0 / 127.0)));
}

#[test]
fn multiple_mappings_work() {
    let mut map = CCMap::new();

    map.set_mapping(10, ParamTarget::AttackTime);
    map.set_mapping(11, ParamTarget::ReleaseTime);

    let result1 = map.map_cc(10, 64);
    let result2 = map.map_cc(11, 32);

    assert_eq!(result1, Some((ParamTarget::AttackTime, 64.0 / 127.0)));
    assert_eq!(result2, Some((ParamTarget::ReleaseTime, 32.0 / 127.0)));
}

#[test]
fn get_mappings_returns_array() {
    let map = CCMap::new();
    let mappings = map.get_mappings();

    assert_eq!(mappings.len(), 16);
    assert_eq!(mappings[0], (1, ParamTarget::FilterCutoff));
    assert_eq!(mappings[1], (74, ParamTarget::FilterResonance));
}

#[test]
fn unused_mappings_default_to_unused() {
    let map = CCMap::new();
    let mappings = map.get_mappings();

    // Check that unmapped slots are Unused
    assert_eq!(mappings[2], (0, ParamTarget::Unused));
    assert_eq!(mappings[3], (0, ParamTarget::Unused));
}

#[test]
fn cc_values_clamped_to_valid_range() {
    let map = CCMap::new();

    // Test with values outside 0-127 range (though MIDI spec says 0-127)
    let result_low = map.map_cc(1, 0);
    let result_high = map.map_cc(1, 127);

    assert_eq!(result_low, Some((ParamTarget::FilterCutoff, 0.0)));
    assert_eq!(result_high, Some((ParamTarget::FilterCutoff, 1.0)));
}