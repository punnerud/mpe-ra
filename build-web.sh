#!/usr/bin/env bash
# Bygger WASM-versjonen og kopierer den inn i web/ for servering i nettleser.
set -euo pipefail

cd "$(dirname "$0")"

echo "==> Bygger for wasm32-unknown-unknown (release) ..."
cargo build --release --target wasm32-unknown-unknown

echo "==> Kopierer .wasm til web/ ..."
cp target/wasm32-unknown-unknown/release/openrarust.wasm web/openrarust.wasm

echo ""
echo "Ferdig. Start en lokal webserver og apne nettleseren:"
echo "    cd web && python3 -m http.server 8080"
echo "    -> http://localhost:8080"
