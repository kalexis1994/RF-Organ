#!/usr/bin/env bash
# Builds the play surface's generated half - the `app.js` bridge and
# `app_bg.wasm` - into package/web, beside the static HTML, CSS and fonts.
#
# Those two files are not committed. wasm-bindgen output carries the build
# machine's paths, so a copy built on one machine cannot be compared with a
# copy built on another, and a committed copy nothing can check is a copy that
# goes stale. CI and the release build them from the sources every time
# instead, and so does anyone running the package locally.
#
#     bash tools/build-surface.sh
#
# Needs the wasm32 target and the wasm-bindgen CLI at the version Cargo.lock
# pins (`cargo install wasm-bindgen-cli --version "$(bash tools/pins.sh wasm-bindgen)" --locked`).
set -euo pipefail

cd "$(dirname "$0")/.."

pinned=$(bash tools/pins.sh wasm-bindgen)
installed=$(wasm-bindgen --version 2>/dev/null | sed -n 's/^wasm-bindgen \([^ ]*\).*/\1/p')
if [ "$installed" != "$pinned" ]; then
  echo "wasm-bindgen ${installed:-is missing}; Cargo.lock pins $pinned, and a different CLI emits a surface that will not load" >&2
  echo "install it with: cargo install wasm-bindgen-cli --version $pinned --locked" >&2
  exit 1
fi

target_dir=${CARGO_TARGET_DIR:-target}
cargo build --locked --release -p rf-organ-ui --target wasm32-unknown-unknown
wasm-bindgen "$target_dir"/wasm32-unknown-unknown/release/rf_organ_ui.wasm \
  --target web --out-dir package/web --out-name app --no-typescript
echo "SURFACE_BUILT wasm-bindgen=$pinned"
