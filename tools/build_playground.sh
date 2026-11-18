#!/usr/bin/env bash
# Rebuild the WebAssembly module used by the docs playground.
set -euo pipefail
cd "$(dirname "$0")/.."
wasm-pack build crates/oxymorph-wasm --target web --release --out-dir /tmp/oxymorph-wasm-pkg
cp /tmp/oxymorph-wasm-pkg/oxymorph_wasm.js \
   /tmp/oxymorph-wasm-pkg/oxymorph_wasm_bg.wasm \
   /tmp/oxymorph-wasm-pkg/oxymorph_wasm.d.ts \
   docs/assets/playground/
echo "updated docs/assets/playground/"
