# RF-Organ

[![CI](https://github.com/kalexis1994/RF-Organ/actions/workflows/ci.yml/badge.svg)](https://github.com/kalexis1994/RF-Organ/actions/workflows/ci.yml)

RF-Organ is a physically informed tonewheel-organ instrument for RackForge.
It is written in Rust and licensed under GPL-2.0-or-later.

The current `0.50.0` baseline establishes the real-time architecture:

- one continuously rotating bank of 91 shared tonewheels;
- 60 Hz gear-ratio tuning instead of ideal equal temperament;
- two 61-key manuals with nine independently timed contacts per key;
- a 25-note pedal clavier with eight contacts per key, four harmonic buses,
  and resistor-weighted 16' and 8' drawbar mixtures;
- a key click that is the contact arrival itself, so turning it down slows
  the attack rather than muting a separate noise;
- the console's twelve reverse-colour keys per manual: cancel, the nine
  preset-panel registrations, and the A# and B adjust keys that reach the two
  sets of nine adjustable drawbars;
- a play surface in four tabs with drawbars drawn as drawbars, and package
  artwork of the instrument itself;
- sixteen programs, half of them tone colours quoted from a published table of
  drawbar registration patterns for the four pipe-organ tone families;
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
- a mains supply switch, which the cabinet's motors follow in proportion and
  the console's drive shaft follows to its market's synchronous speed;
- a drive that is resiliently coupled at every joint, so the shaft is never
  perfectly steady and all 91 wheels stray together;
- leakage that grows faster than what is played, because every contact that
  closes gives the harness another way onto the busbar;
- wheel levels and per-contact key tapering derived from published readings
  of real consoles - Hammond's own tables among them - and checked against
  readings that were not used to derive them;
- the woofer's own unmodulated bass, which reaches the microphones without
  passing through a rotor, and a choice of capsule;
- an integrated rotary speaker with counter-rotating horn and drum at their
  specified speeds, separate rise, fall and brake times per rotor, spectral
  directivity, microphone geometry and early cabinet reflections;
- the cabinet's own valve amplifier and passive crossover, from the published
  schematic of its main amplifier, clipping and blocking the way a push-pull
  pair with no global feedback does;
- a RackForge play surface implemented in Rust/WASM, with console-style
  drawbars, direct access to every automatable parameter, host-driven
  parameter synchronization, day/stage lighting and a plan view of the
  cabinet driven by the engine's own rotor model;
- a manifest at schema 3 with its own artwork.

The sound constants are intentionally provisional. This baseline is a
testable scaffold for measurement and calibration, not a finished clone.

## MIDI baseline

- Channel 1 notes: upper manual, MIDI notes 36 through 96.
- Channel 2 notes: lower manual, MIDI notes 36 through 96.
- Channel 3 notes: pedal clavier, MIDI notes 24 through 48.
- CC 1: rotary speed, Chorale below 64 and Tremolo at or above 64.
- CC 4: rotor stop, braking below 64 and returning at or above 64 to the speed
  last chosen, by CC 1 or by the rotary speed parameter.
- CC 11: expression pedal.

What MIDI sets stays set when a control moves: a parameter change reaches only
the part of the engine that parameter drives, so the pedal and the cabinet
speed hold until the next controller or a change to their own parameters.
- CC 120/123: all notes off for the addressed channel/part.

## Installing a build

`tools/release.sh` rebuilds everything the package contains, runs the same
gates CI does, and packs an installable archive:

```text
bash tools/release.sh
```

It writes `dist/RF-Organ-<version>.rfplugin` and packs it a second time to
prove the archive is reproducible: an unchanged tree gives the same bytes and
the same digest. Install it on another machine with:

```text
rackforge-store install-local RF-Organ-<version>.rfplugin <STORE_ROOT>
```

`--quick` skips the test suite and the sample-rate sweep when iterating.

## Development

```text
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p rf-organ-lab -- sweep artifacts/sweep
cargo build --release -p rf-organ-plugin --target wasm32-unknown-unknown
Copy-Item target/wasm32-unknown-unknown/release/rf_organ_plugin.wasm package/component.wasm
bash tools/build-surface.sh
cargo run --manifest-path ../rackforge/Cargo.toml --target-dir target/rackforge-core -p rackforge-core -- inspect package
target/rackforge-core/debug/rackforge-core.exe smoke package --preset chorale-888
```

The RackForge plugin SDK is a git dependency pinned by revision, and
`Cargo.lock` records it, so building and testing need nothing beside this
checkout. Inspecting, smoking and packing the package run through the host's
own tools, which come from a sibling `rackforge` checkout at the same
revision; `bash tools/pins.sh rackforge` prints it. Package metadata lives
under `package/`.

To build against a local RackForge with changes of its own, override the
dependency in an untracked `.cargo/config.toml`:

```toml
[patch."https://github.com/kalexis1994/rackforge"]
rackforge-plugin-sdk = { path = "../rackforge/crates/rackforge-plugin-sdk" }
```

That rewrites the lockfile's entry to the local path, so it is not something
to commit: CI builds `--locked` and refuses it. Moving to a newer SDK is a
change to the `rev` in `Cargo.toml`, and CI and the release follow the lock.

`.github/workflows/ci.yml` runs on every push and pull request: it checks out
RackForge at the revision `Cargo.lock` records, then runs formatting, clippy,
the test suite, both wasm32 builds, package inspection and smoke, the
sample-rate sweep, and `tools/check-package.sh`, which proves the committed
`component.wasm` still reports the same parameter schema, state size and
rendered output as a build from the current sources.

The manifest is at schema 3, which asks for a short name and a branding
section. `tools/make-artwork.py` draws the icon, banner and splash from the
same palette as the play surface, at the exact sizes the host requires; it
needs Pillow and writes into `package/branding/`.

The custom surface is declared through the package's `web_ui` manifest table.
Its HTML and CSS are static, `bootstrap.js` only initializes the module, and
the generated `app.js` bridge plus `app_bg.wasm` contain the Rust UI runtime.
Those two are not committed: wasm-bindgen output carries the paths of the
machine that built it, so a committed copy could not be checked against the
sources. `tools/build-surface.sh` builds them with the wasm-bindgen version
`Cargo.lock` pins, and CI and the release always do.

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

Every number the engine is built from is registered with what it rests on -
documented, measured, derived, borrowed from another model, or provisional -
and the span it could plausibly take; `docs/PROVENANCE.md` is the table.
`sensitivity` moves each unmeasured constant to both ends of its span, renders
the calibration phrases again - and five of its own that play what they do
not: keys let go one by one under a softened click, the percussion's soft and
slow settings, the swell pedal moving, the cabinet braked with a dynamic
capsule, the pedals released, and the cabinet's amplifier pushed - and ranks
the constants by how far the sound moved, in decibels of level, spectral
shape and envelope, which is where a measurement would be worth the most.
Every constant is reached by at least one phrase:

```text
cargo run --release -p rf-organ-lab -- sensitivity artifacts/sensitivity
```

`wheel-levels` derives the wheel and key tapers the engine ships from Kon
Zissis's tonewheel generator data, reduced by `tools/kon-wheel-levels.py`,
writes the tables for `taper.rs`, and reports how far the engine is from the
readings taken straight off a busbar and from those taken through the keys,
which are not used to derive anything. The same data checks the manuals'
wiring: every contact has to reach the wheel five measured manuals wire it to.

```text
cargo run --release -p rf-organ-lab -- wheel-levels artifacts/wheel-levels
```

The output includes scalar measurements, expression, tone-control and
transformer-intermodulation response, percussion envelopes, scanner sidebands,
isolated pedal spectra and key-off response, and rotary acceleration/braking
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

## Trademarks

HAMMOND, B-3 and LESLIE are trademarks of Hammond Suzuki. RF-Organ is an
independent implementation and is not affiliated with, endorsed by or sponsored
by Hammond Suzuki. Those names are used here only to state factually which
published instrument or document a model was derived from; the module that
models a rotating-baffle cabinet is called the rotary speaker throughout. See
`THIRD_PARTY_NOTICES.md`.
