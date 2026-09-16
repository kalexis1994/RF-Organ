# RF-Organ

RF-Organ is a physically informed tonewheel-organ instrument for RackForge.
It is written in Rust and licensed under GPL-2.0-or-later.

The current `0.1.0` baseline establishes the real-time architecture:

- one continuously rotating bank of 91 shared tonewheels;
- 60 Hz gear-ratio tuning instead of ideal equal temperament;
- a 61-key upper manual with nine independently timed contacts per key;
- B-3-style drawbar wiring and foldback;
- topology-derived compartment leakage;
- a shared matching-transformer stage;
- an integrated Leslie module controlled by parameters, programs and MIDI.

The sound constants are intentionally provisional. This first version is a
testable scaffold for measurement and calibration, not a finished clone.

## MIDI baseline

- Channel 1 notes: upper manual, MIDI notes 36 through 96.
- CC 1: Leslie speed, Chorale below 64 and Tremolo at or above 64.
- CC 11: expression pedal.
- CC 120/123: all notes off.

## Development

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p rf-organ-plugin --target wasm32-unknown-unknown
Copy-Item target/wasm32-unknown-unknown/release/rf_organ_plugin.wasm package/component.wasm
../rackforge/target/debug/rackforge-core.exe inspect package
../rackforge/target/debug/rackforge-core.exe smoke package --preset chorale-888
```

The RackForge development SDK is consumed from the sibling `rackforge`
checkout. Package metadata lives under `package/`.
