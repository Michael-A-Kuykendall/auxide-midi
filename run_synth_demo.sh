#!/bin/bash
# Run the Auxide synth demo

cd "$(dirname "$0")"
cd ..

echo "Building Auxide Synth Demo..."
cargo build --example synth_demo --quiet

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo "Starting Synth Demo..."
echo ""
cargo run --example synth_demo --quiet
