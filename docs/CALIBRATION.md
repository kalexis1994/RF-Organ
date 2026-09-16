# Calibration workflow

`rf-organ-lab` generates deterministic, unnormalised references from the exact
DSP used by the RackForge plugin. It is deliberately written in Rust and does
not embed third-party analysis code.

```text
cargo run --release -p rf-organ-lab -- render artifacts/calibration
```

The suite contains:

- the 91 physical generator frequencies as CSV;
- direct 888, percussion, scanner C3, Chorale and Tremolo phrases;
- a complete two-manual and pedal-console phrase;
- the integrated Leslie cabinet impulse response;
- a manifest recording version, sample rate and normalization policy.

Levels must not be normalized before comparison. Align reference captures by
their first contact transient, preserve the recording gain, and record the
microphone angle and distance. Coefficients derived from a measurement belong
in a reviewed source file together with the measurement identifier, equipment
chain and fitting error.

The generated `artifacts/` directory is intentionally ignored by Git. Curated
measurement data should only enter the repository with clear redistribution
rights and provenance in `THIRD_PARTY_NOTICES.md`.
