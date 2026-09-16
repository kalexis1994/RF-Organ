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
- an end-to-end A4 frequency, RMS and peak probe;
- fast and slow percussion envelope curves measured in 10 ms RMS windows;
- 1 kHz scanner carrier and ±6.9 Hz sideband levels for V1–V3 and C1–C3;
- horn and drum acceleration/braking curves sampled every 10 ms, including
  measured 63% and 90% transition times;
- a manifest recording version, sample rate and normalization policy.

`measurements.csv` is the compact comparison surface. The three detailed
tables retain the underlying curves:

- `percussion-envelope.csv` records both decay registrations;
- `scanner-sidebands.csv` records carrier level in dBFS and sidebands in dBc;
- `leslie-rotor-response.csv` records actual and target rotor speeds during
  Tremolo acceleration and Brake deceleration.

These values describe the current model; they are regression baselines, not
claims about a particular historical console. A coefficient becomes calibrated
only when its probe is compared with a documented reference capture.

Levels must not be normalized before comparison. Align reference captures by
their first contact transient, preserve the recording gain, and record the
microphone angle and distance. Coefficients derived from a measurement belong
in a reviewed source file together with the measurement identifier, equipment
chain and fitting error.

Keep the sample rate at 48 kHz for baseline comparisons. Frequency error is
reported in cents, percussion decay as T60, scanner sidebands relative to the
measured carrier, and rotor transition times in seconds. When a model change is
intentional, compare both the scalar summary and the associated curve before
accepting a new baseline.

The generated `artifacts/` directory is intentionally ignored by Git. Curated
measurement data should only enter the repository with clear redistribution
rights and provenance in `THIRD_PARTY_NOTICES.md`.
