#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "Compiling SCSS..."
mkdir -p static/css
grass static/scss/main.scss static/css/main.css --style compressed

echo "Building Rust..."
cargo build --release

echo "Done."
