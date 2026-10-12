#!/usr/bin/env bash
# Prints what Cargo.lock pins, so the workflows and the release read it from
# the one place it is recorded instead of keeping copies that drift.
#
#     bash tools/pins.sh rackforge      # the RackForge revision the SDK is from
#     bash tools/pins.sh wasm-bindgen   # the wasm-bindgen the surface links
#
# The host tools (inspect, smoke, pack) are built from the same RackForge
# revision as the SDK, and the wasm-bindgen CLI has to match the library
# version exactly or the generated surface will not load.
set -euo pipefail

lock="$(dirname "$0")/../Cargo.lock"

case "${1:-}" in
  rackforge)
    value=$(sed -n 's|^source = "git+https://github.com/kalexis1994/rackforge?rev=[0-9a-f]*#\([0-9a-f]\{40\}\)"$|\1|p' "$lock" | sort -u)
    ;;
  wasm-bindgen)
    value=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$lock")
    ;;
  *)
    echo "usage: pins.sh rackforge|wasm-bindgen" >&2
    exit 2
    ;;
esac

if [ -z "$value" ] || [ "$(printf '%s\n' "$value" | wc -l)" -ne 1 ]; then
  echo "Cargo.lock does not pin exactly one $1: '${value}'" >&2
  exit 1
fi
printf '%s\n' "$value"
