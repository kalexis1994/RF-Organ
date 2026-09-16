# RF-Organ

RF-Organ is a physically informed tonewheel-organ instrument for RackForge.
It is written in Rust and licensed under GPL-2.0-or-later.

The current `0.5.0` baseline establishes the real-time architecture:

- one continuously rotating bank of 91 shared tonewheels;
- 60 Hz gear-ratio tuning instead of ideal equal temperament;
- two 61-key manuals with nine independently timed contacts per key;
- a 25-note pedal clavier with 16' and 8' drawbars;
- B-3-style drawbar wiring and foldback;
- topology-derived compartment leakage;
- a shared matching-transformer stage;
- a shared console preamplifier with swell-pedal slew and tonal response;
- six scanner vibrato/chorus registrations driven by a 16-contact model;
- single-trigger second/third-harmonic percussion with console-style switches;
- an integrated Leslie with independent rotors, spectral directivity,
  microphone geometry and early cabinet reflections.

The sound constants are intentionally provisional. This first version is a
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
cargo build --release -p rf-organ-plugin --target wasm32-unknown-unknown
Copy-Item target/wasm32-unknown-unknown/release/rf_organ_plugin.wasm package/component.wasm
cargo run --manifest-path ../rackforge/Cargo.toml --target-dir target/rackforge-core -p rackforge-core -- inspect package
target/rackforge-core/debug/rackforge-core.exe smoke package --preset chorale-888
```

The RackForge development SDK is consumed from the sibling `rackforge`
checkout. Package metadata lives under `package/`.
