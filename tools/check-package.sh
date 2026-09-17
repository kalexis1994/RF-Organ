#!/usr/bin/env bash
# Compares the committed component.wasm with a freshly built one through the
# RackForge host. A cosmetic source change produces a different binary but must
# not change the parameter schema, the state size or the rendered output; a DSP
# or parameter change that was never packaged does change them, and that is
# exactly the staleness this catches. Byte comparison would not work: two
# toolchains and two platforms do not emit identical wasm.
set -euo pipefail

core=${1:?usage: check-package.sh RACKFORGE_CORE FRESH_COMPONENT_WASM [PRESET]}
fresh_component=${2:?usage: check-package.sh RACKFORGE_CORE FRESH_COMPONENT_WASM [PRESET]}
preset=${3:-gospel-full}

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cp -r package "$workdir/package"
cp "$fresh_component" "$workdir/package/component.wasm"

"$core" smoke package --preset "$preset" > "$workdir/committed.txt"
"$core" smoke "$workdir/package" --preset "$preset" > "$workdir/fresh.txt"

value() {
  sed -n "s/.*$2=\([^ ]*\).*/\1/p" "$1" | head -1
}

status=0
for key in parameters pages presets state_bytes; do
  committed=$(value "$workdir/committed.txt" "$key")
  fresh=$(value "$workdir/fresh.txt" "$key")
  if [ -z "$committed" ] || [ "$committed" != "$fresh" ]; then
    echo "package/component.wasm is stale: $key reports '$committed', a fresh build reports '$fresh'"
    status=1
  fi
done

committed_peak=$(value "$workdir/committed.txt" peak)
fresh_peak=$(value "$workdir/fresh.txt" peak)
if ! awk -v a="$committed_peak" -v b="$fresh_peak" \
  'BEGIN { exit (a - b < 1e-6 && b - a < 1e-6) ? 0 : 1 }'; then
  echo "package/component.wasm is stale: it renders peak $committed_peak where a fresh build renders $fresh_peak"
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo "PACKAGE_FRESH parameters=$(value "$workdir/committed.txt" parameters) state_bytes=$(value "$workdir/committed.txt" state_bytes) peak=$committed_peak"
fi
exit "$status"
