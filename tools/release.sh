#!/usr/bin/env bash
# Builds a release of RF-Organ and packs it into an installable .rfplugin.
#
# Everything the package contains is rebuilt from the repository first, and the
# same validation CI runs is repeated here, so the archive that comes out has
# been through the same gates as the branch. The archive itself is
# deterministic: packing twice from an unchanged tree gives the same bytes and
# the same digest, which is what makes a version reproducible rather than
# merely labelled.
#
#     bash tools/release.sh            # validate, build, pack
#     bash tools/release.sh --quick    # skip the test suite and the sweep
#
# Needs the sibling rackforge checkout, wasm-bindgen, and the wasm32 target.
set -euo pipefail

cd "$(dirname "$0")/.."
export CARGO_INCREMENTAL=0

quick=0
if [ "${1:-}" = "--quick" ]; then
  quick=1
elif [ "$#" -gt 0 ]; then
  echo "usage: release.sh [--quick]" >&2
  exit 2
fi

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
if [ -z "$version" ]; then
  echo "could not read the workspace version" >&2
  exit 1
fi
for file in package/rackforge-plugin.toml package/metadata/runtime.json; do
  if ! grep -q "\"\?$version\"\?" "$file"; then
    echo "$file does not carry version $version" >&2
    exit 1
  fi
done
echo "RELEASE_VERSION $version"

core_target=target/rackforge-core
rackforge=../rackforge/Cargo.toml

echo "== formatting and lints =="
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings

if [ "$quick" -eq 0 ]; then
  echo "== tests =="
  cargo test --locked --workspace
fi

# The package carries the licence and the notices because the archive is
# conveyed on its own: someone downloads it, and the host compiles it into its
# binary. Refreshed here so the copies cannot drift from the originals.
cp LICENSE package/LICENSE
cp THIRD_PARTY_NOTICES.md package/NOTICE.md

echo "== wasm =="
cargo build --locked --release -p rf-organ-plugin --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/rf_organ_plugin.wasm package/component.wasm
cargo build --locked --release -p rf-organ-ui --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/rf_organ_ui.wasm \
  --target web --out-dir package/web --out-name app --no-typescript

echo "== package =="
cargo run --locked --manifest-path "$rackforge" --target-dir "$core_target" \
  -p rackforge-core -- inspect package
"$core_target"/debug/rackforge-core smoke package --preset gospel-full
bash tools/check-package.sh "$core_target"/debug/rackforge-core \
  target/wasm32-unknown-unknown/release/rf_organ_plugin.wasm

if [ "$quick" -eq 0 ]; then
  echo "== sample rates and cost =="
  cargo run --locked --release -p rf-organ-lab -- sweep artifacts/sweep
fi

echo "== pack =="
mkdir -p dist
archive="dist/RF-Organ-$version.rfplugin"
rm -f "$archive"
cargo run --locked --manifest-path "$rackforge" --target-dir "$core_target" --release \
  -p rackforge-store -- pack-wasm package package/component.wasm "$archive"

# Pack a second copy and compare: a version that cannot be rebuilt to the same
# bytes is not reproducible, whatever the changelog says.
check="dist/RF-Organ-$version.repeat.rfplugin"
rm -f "$check"
"$core_target"/release/rackforge-store pack-wasm package package/component.wasm "$check" \
  > /dev/null
if ! cmp -s "$archive" "$check"; then
  rm -f "$check"
  echo "packing twice produced different archives" >&2
  exit 1
fi
rm -f "$check"

echo "RELEASE_READY version=$version path=$archive bytes=$(wc -c < "$archive")"
echo "Install it on another machine with:"
echo "  rackforge-store install-local $(basename "$archive") <STORE_ROOT>"
