#!/bin/bash
set -e
echo "Building frontend..."
cd "$(dirname "$0")" && npm run build && cd ..
echo "Building Rust binary..."
cargo build --release
echo "Done! Run: ./target/release/polez gui"
