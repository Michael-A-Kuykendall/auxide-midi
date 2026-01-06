# Changelog

## [0.2.0] - 2026-01-05
- **Comprehensive MIDI integration and testing** - Expanded voice allocator tests with property-based testing
- **Integration tests** - Added tests for MIDI parsing, CC mapping, and cross-crate functionality
- **Voice allocation system** - Enhanced polyphonic synthesis support with configurable voice limits
- **MIDI input handling** - Improved real-time MIDI message parsing and processing
- **CC mapping** - Enhanced control change parameter mapping for dynamic DSP control
- **Voice state management** - Better note on/off, velocity, pitch bend, and modulation tracking
- **Smoothing utilities** - Improved parameter smoothing for glitch-free automation
- **Documentation** - Added governance files and comprehensive API documentation

## [0.1.0] - 2026-01-05
- **Initial release** of auxide-midi, MIDI integration layer for Auxide DSP graphs
- **Voice allocation system** - Polyphonic synthesis support with configurable voice limits
- **MIDI input handling** - Real-time MIDI message parsing and processing
- **CC mapping** - Control change parameter mapping for dynamic DSP control
- **Voice state management** - Note on/off, velocity, pitch bend, and modulation tracking
- **Smoothing utilities** - Parameter smoothing for glitch-free automation
- **Comprehensive testing** - Unit tests for MIDI parsing, voice allocation, CC mapping, and integration
- **RT-safe design** - No allocations or locks in audio processing paths
- **Cross-crate integration** - Seamless integration with auxide-dsp nodes and auxide kernel</content>
<parameter name="filePath">c:\Users\micha\repos\auxide-midi\CHANGELOG.md