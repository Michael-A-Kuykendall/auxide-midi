#!/bin/bash
# Quick start script for Auxide MIDI Synthesizer demo

set -e

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║        Auxide MIDI Synthesizer - Quick Start                  ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || ! grep -q "auxide-midi" Cargo.toml; then
    echo "❌ Please run this script from the auxide-midi directory"
    echo "   cd auxide-midi && ./run-demo.sh"
    exit 1
fi

echo "Step 1: Checking available MIDI devices..."
echo

cargo run --quiet --example list_devices

echo
echo "Step 2: Building synthesizer..."
cargo build --quiet --example poly_synth

echo "✓ Build complete"
echo

echo "Step 3: Starting synthesizer..."
echo "   (If no MIDI device auto-detected, you'll be prompted to select one)"
echo

cargo run --example poly_synth
