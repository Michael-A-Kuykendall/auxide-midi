//! MIDI CC parameter mapping

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamTarget {
    FilterCutoff,
    FilterResonance,
    AttackTime,
    ReleaseTime,
    Unused,
}

#[derive(Debug)]
pub struct CCMap {
    mappings: [(u8, ParamTarget); 16], // Fixed size for RT-safety
}

impl CCMap {
    pub fn new() -> Self {
        let mut mappings = [(0, ParamTarget::Unused); 16];

        // Default mappings
        mappings[0] = (1, ParamTarget::FilterCutoff); // Mod wheel -> cutoff
        mappings[1] = (74, ParamTarget::FilterResonance); // Filter Q -> resonance

        Self { mappings }
    }

    /// Map a CC number and value to a parameter target and normalized value
    pub fn map_cc(&self, cc_num: u8, value: u8) -> Option<(ParamTarget, f32)> {
        for (mapped_cc, target) in &self.mappings {
            if *mapped_cc == cc_num && *target != ParamTarget::Unused {
                let normalized = value as f32 / 127.0;
                return Some((*target, normalized));
            }
        }
        None
    }

    /// Set a mapping for a CC number
    pub fn set_mapping(&mut self, cc_num: u8, target: ParamTarget) {
        // Find first unused slot or replace existing
        for mapping in &mut self.mappings {
            if mapping.1 == ParamTarget::Unused || mapping.0 == cc_num {
                *mapping = (cc_num, target);
                break;
            }
        }
    }

    /// Get all current mappings
    pub fn get_mappings(&self) -> &[(u8, ParamTarget); 16] {
        &self.mappings
    }
}

impl Default for CCMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cc1_maps_cutoff() {
        let map = CCMap::new();
        let result = map.map_cc(1, 64);
        assert_eq!(result, Some((ParamTarget::FilterCutoff, 64.0 / 127.0)));
    }

    #[test]
    fn cc74_maps_resonance() {
        let map = CCMap::new();
        let result = map.map_cc(74, 100);
        assert_eq!(result, Some((ParamTarget::FilterResonance, 100.0 / 127.0)));
    }

    #[test]
    fn unused_cc_ignored() {
        let map = CCMap::new();
        let result = map.map_cc(42, 100);
        assert_eq!(result, None);
    }

    #[test]
    fn set_mapping_works() {
        let mut map = CCMap::new();
        map.set_mapping(7, ParamTarget::AttackTime);

        let result = map.map_cc(7, 127);
        assert_eq!(result, Some((ParamTarget::AttackTime, 1.0)));
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
    }
}
