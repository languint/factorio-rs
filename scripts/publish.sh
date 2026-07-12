#!/usr/bin/env bash
# Publish workspace crates to crates.io in dependency order.
#
# Usage:
#   ./scripts/publish.sh           # publish current versions
#   ./scripts/publish.sh --dry-run # package/verify only
#   ./scripts/publish.sh --yes     # skip confirmation prompt
#
# Requires `cargo login` (credentials in ~/.cargo/credentials.toml).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DRY_RUN=0
ASSUME_YES=0
SLEEP_SECS="${PUBLISH_SLEEP_SECS:-45}"

# Bottom-up publish order (path deps must already be on crates.io).
CRATES=(
  factorio-ir
  factorio-api-gen
  factorio-codegen
  factorio-api
  factorio-macros
  factorio-frontend
  factorio-rs
  factorio-rs-cli
)

usage() {
  cat <<'EOF'
Publish workspace crates to crates.io in dependency order.

Usage:
  ./scripts/publish.sh           # publish current versions
  ./scripts/publish.sh --dry-run # package/verify only
  ./scripts/publish.sh --yes     # skip confirmation prompt
  ./scripts/publish.sh --sleep N # seconds to wait between crates (default 45)

Requires `cargo login` (credentials in ~/.cargo/credentials.toml).
EOF
  exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --yes|-y) ASSUME_YES=1 ;;
    --sleep)
      SLEEP_SECS="$2"
      shift
      ;;
    -h|--help) usage 0 ;;
    *)
      echo "unknown argument: $1" >&2
      usage 1
      ;;
  esac
  shift
done

crate_version() {
  local name="$1"
  cargo metadata --format-version 1 --no-deps \
    | python3 -c "
import json, sys
name = sys.argv[1]
data = json.load(sys.stdin)
for pkg in data['packages']:
    if pkg['name'] == name:
        print(pkg['version'])
        raise SystemExit(0)
raise SystemExit(f'package {name} not found in workspace')
" "$name"
}

crates_io_has_version() {
  local name="$1"
  local version="$2"
  local code
  code="$(curl -sS -o /tmp/factorio-rs-crate.json -w '%{http_code}' \
    -A 'factorio-rs-publish-script' \
    "https://crates.io/api/v1/crates/${name}/${version}" || true)"
  [[ "$code" == "200" ]]
}

echo "Publish order (${#CRATES[@]} crates):"
versions=()
for crate in "${CRATES[@]}"; do
  ver="$(crate_version "$crate")"
  versions+=("$ver")
  if crates_io_has_version "$crate" "$ver"; then
    echo "  - ${crate}@${ver}  (already on crates.io — will skip)"
  else
    echo "  - ${crate}@${ver}"
  fi
done

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo
  echo "Dry run: packaging each crate with cargo publish --dry-run"
else
  echo
  if [[ "$ASSUME_YES" -ne 1 ]]; then
    read -r -p "Publish to crates.io? [y/N] " reply
    case "$reply" in
      y|Y|yes|YES) ;;
      *)
        echo "Aborted."
        exit 1
        ;;
    esac
  fi
fi

published=0
skipped=0
for i in "${!CRATES[@]}"; do
  crate="${CRATES[$i]}"
  ver="${versions[$i]}"

  if crates_io_has_version "$crate" "$ver"; then
    echo
    echo "==> Skipping ${crate}@${ver} (already published)"
    skipped=$((skipped + 1))
    continue
  fi

  echo
  echo "==> Publishing ${crate}@${ver}"
  args=(-p "$crate" --locked)
  if [[ "$DRY_RUN" -eq 1 ]]; then
    args+=(--dry-run)
  fi

  cargo publish "${args[@]}"

  if [[ "$DRY_RUN" -eq 0 ]]; then
    published=$((published + 1))
    # Allow the crates.io index to catch up before dependents resolve the new version.
    if [[ "$i" -lt $((${#CRATES[@]} - 1)) ]]; then
      echo "Waiting ${SLEEP_SECS}s for crates.io index..."
      sleep "$SLEEP_SECS"
    fi
  fi
done

echo
if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Dry run finished."
else
  echo "Done. Published ${published}, skipped ${skipped}."
fi
