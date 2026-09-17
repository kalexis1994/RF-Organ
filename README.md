# RF-Organ

[![CI](https://github.com/kalexis1994/RF-Organ/actions/workflows/ci.yml/badge.svg)](https://github.com/kalexis1994/RF-Organ/actions/workflows/ci.yml)

RF-Organ is a physically informed tonewheel-organ instrument for RackForge.
It is written in Rust and licensed under GPL-2.0-or-later.

The current `0.20.0` baseline establishes the real-time architecture:

- one continuously rotating bank of 91 shared tonewheels;
- 60 Hz gear-ratio tuning instead of ideal equal temperament;
- two 61-key manuals with nine independently timed contacts per key;
- a 25-note pedal clavier with eight contacts per key, four harmonic buses,
  and resistor-weighted 16' and 8' drawbar mixtures;
- B-3-style drawbar wiring and foldback;
- compartment leakage following the service manual's four-wheel compartments,
  and a once-per-revolution level change for wheel eccentricity;
- separate AO-28 matching-transformer paths for upper (T2) and lower+pedal
  (T1), ahead of the vibrato routing;
- one shared transformer character control plus independent calibration trims
  for T1, T2 and T3, so a measurement of one unit does not move the others;
- an AO-28-derived console path with separate pre/post-expression stages
  shaped as single-ended triodes, a calibratable capacitive swell response,
  post-V4B tone shelf, V3B output stage and T3 output transformer;
- the documented vibrato/chorus circuit: an eighteen-section LC ladder with
  its tap dividers and termination, scanned by a sixteen-stack capacitive
  rotor, with the depth tablet selecting tap sets and the vibrato/chorus
  tablet switching the source resistor;
- single-trigger second/third-harmonic percussion with console-style switches
  and a dedicated post-scanner summing path, cancelling the 1' drawbar and
  taking Hammond's documented 6 dB out of the upper drawbars at Normal volume,
  with an attack and a recovery after release rather than a gate;
- an integrated Leslie with independent rotors, spectral directivity,
  microphone geometry and early cabinet reflections;
- a RackForge play surface implemented in Rust/WASM, with console-style
  drawbars, direct access to every automatable parameter, host-driven
  parameter synchronization and day/stage lighting.

The sound constants are intentionally provisional. This baseline is a
testable scaffold for measurement and calibration, not a finished clone.

## MIDI baseline

- Channel 1 notes: upper manual, MIDI notes 36 through 96.
- Channel 2 notes: lower manual, MIDI notes 36 through 96.
- Channel 3 notes: pedal clavier, MIDI notes 24 through 48.
- CC 1: Leslie speed, Chorale below 64 and Tremolo at or above 64.
- CC 11: expression pedal.
- CC 120/123: all notes off for the addressed channel/part.

## Development

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p rf-organ-lab -- sweep artifacts/sweep
cargo build --release -p rf-organ-plugin --target wasm32-unknown-unknown
Copy-Item target/wasm32-unknown-unknown/release/rf_organ_plugin.wasm package/component.wasm
cargo build --release -p rf-organ-ui --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/rf_organ_ui.wasm --target web --out-dir package/web --out-name app --no-typescript
cargo run --manifest-path ../rackforge/Cargo.toml --target-dir target/rackforge-core -p rackforge-core -- inspect package
target/rackforge-core/debug/rackforge-core.exe smoke package --preset chorale-888
```

The RackForge development SDK is consumed from the sibling `rackforge`
checkout. Package metadata lives under `package/`.

`.github/workflows/ci.yml` reproduces that layout on every push and pull
request: it checks out RackForge at the revision `Cargo.lock` records, then
runs formatting, clippy, the test suite, both wasm32 builds, package inspection
and smoke, the sample-rate sweep, and `tools/check-package.sh`, which proves the
committed `component.wasm` still reports the same parameter schema, state size
and rendered output as a build from the current sources. Bump `RACKFORGE_REF`
and the lockfile together.

The custom surface is declared through the package's `web_ui` manifest table.
Its HTML and CSS are static, `bootstrap.js` only initializes the module, and
the generated `app.js` bridge plus `app_bg.wasm` contain the Rust UI runtime.

Deterministic calibration renders and quantitative measurements are produced
by the Rust-only laboratory:

```text
cargo run --release -p rf-organ-lab -- render artifacts/calibration
cargo run --release -p rf-organ-lab -- compare artifacts/calibration path/to/reference-captures artifacts/comparison
```

`render` also accepts `--transformer-drive-trims T1,T2,T3`, which produces a
synthetic reference set with known transformer coefficients. Comparing it with
the untrimmed calibration render recovers those coefficients and is how the
fitting chain is verified.

`sweep` measures one quantity per subsystem at 32, 44.1, 48, 96 and 192 kHz and
fails when any of them moves with the sample rate, then benchmarks five loads at
each rate — from an idle generator up to a fully engaged console — so that the
cost of each subsystem is the difference between two rows.

The output includes scalar measurements, expression, tone-control and
transformer-intermodulation response, percussion envelopes, scanner sidebands,
isolated pedal spectra and key-off response, and Leslie acceleration/braking
curves alongside the audio renders.
The comparator aligns reference captures without resampling or normalizing
them, then reports level, crest, envelope and stereo differences. Isolated
pedal captures additionally produce per-harmonic errors and provisional
resistor-bus/L20 fitting candidates. Transformer captures are recorded at three
documented levels; their intermodulation products yield drive-trim candidates
for T3 from the optional bench injection grid, and then for T2 and T1. Without
an injection measurement the two manual paths are reported as underdetermined
rather than fitted. A capture-quality gate prevents clipped, mistimed, noisy or
unstable recordings from producing coefficients.
See `docs/CALIBRATION.md` for the comparison and provenance protocol.
