#!/usr/bin/env bash
# Build the factorio-playground WASM package into docs/public/playground/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/docs/public/playground"

command -v wasm-pack >/dev/null || {
  echo "wasm-pack is required (cargo install wasm-pack)" >&2
  exit 1
}

mkdir -p "$OUT"
rm -rf "$OUT"/*

CARGO_PROFILE_RELEASE_LTO=true \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
CARGO_PROFILE_RELEASE_PANIC=abort \
wasm-pack build "$ROOT/crates/factorio-playground" \
  --target web \
  --release \
  --out-dir "$OUT"

# wasm-pack writes a catch-all .gitignore; keep artifacts for local docs preview
# but do not commit them (see root .gitignore).
rm -f "$OUT/.gitignore" "$OUT/README.md" "$OUT/package.json" \
  "$OUT"/*.d.ts

echo "Playground WASM ready at $OUT"
