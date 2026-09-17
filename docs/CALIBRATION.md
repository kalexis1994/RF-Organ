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
- isolated low-C pedal captures for 16', 8' and both drawbars;
- the integrated Leslie cabinet impulse response;
- an end-to-end A4 frequency, RMS and peak probe;
- expression gain at 80 Hz, 1 kHz and 8 kHz for five pedal positions;
- AO-28 tone-control response at 100 Hz, 1 kHz and 10 kHz across ±9 dB;
- direct two-tone transformer difference-product levels across five magnetic
  settings;
- fast and slow percussion envelope curves measured in 10 ms RMS windows;
- 1 kHz scanner carrier and ±6.9 Hz sideband levels for V1–V3 and C1–C3;
- horn and drum acceleration/braking curves sampled every 10 ms, including
  measured 63% and 90% transition times;
- pedal harmonic levels for the eight physical contacts, plus the complete
  key-off response through the pedal filter and console electronics;
- a manifest recording version, sample rate and normalization policy.

`measurements.csv` is the compact comparison surface. The detailed
tables retain the underlying curves:

- `percussion-envelope.csv` records both decay registrations;
- `scanner-sidebands.csv` records carrier level in dBFS and sidebands in dBc;
- `leslie-rotor-response.csv` records actual and target rotor speeds during
  Tremolo acceleration and Brake deceleration.
- `pedal-spectrum.csv` records the eight contact frequencies and levels for
  isolated 16' and 8' registrations;
- `pedal-release.csv` records the first 100 ms after releasing the 16' low C.
  This end-to-end response includes the provisional L20 branch, matching
  transformer and console coupling network; it deliberately does not pretend
  that the measured tail belongs to one component in isolation.
- `expression-response.csv` records the swell-section capacitance, provisional
  low corner and gain at each probe frequency without drive or tone trim,
  making the passive-network coefficients independently fit-able.
- `tone-control-response.csv` records gain relative to neutral at three
  frequencies for five control positions, isolating the post-V4B shelf.
- `transformer-intermodulation.csv` drives the reduced magnetic model with
  523.3 Hz and 698.5 Hz and reports their 175.2 Hz difference product.

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

## Comparing reference captures

Reference WAV files use the same names as the generated scenarios. A directory
may contain only the scenarios available from a session; at least one must be
present. Copy `docs/REFERENCE_CAPTURE_TEMPLATE.txt` to
`reference-manifest.txt` in that directory and complete every required field.
Private measurements may say so in `rights`; they do not need to be committed
or redistributed.

Accepted captures are mono or stereo 48 kHz WAV files using PCM 16/24/32-bit
or IEEE float32 samples. The comparator intentionally rejects other sample
rates instead of silently resampling them, and rejects non-finite float data.

```text
cargo run --release -p rf-organ-lab -- compare \
  artifacts/calibration \
  path/to/reference-captures \
  artifacts/comparison
```

`capture-comparison.csv` aligns each pair at its first transient while keeping
the original amplitudes. It reports:

- reference-minus-model onset offset;
- absolute model and reference RMS levels;
- model-minus-reference level, peak and crest-factor deltas;
- correlation between 10 ms RMS envelopes;
- model and reference mid/side stereo width and their delta.

When isolated pedal captures are present, comparison also writes:

- `pedal-spectrum-comparison.csv`, with model/reference dBc and error for all
  eight physical harmonics;
- `pedal-fit-candidates.csv`, with relative gain multipliers for each resistor
  bus and a provisional L20 effective-cutoff starting point.

When either transformer dyad is present, its complete single-C, single-F and
C+F triplet is required. `transformer-intermodulation-comparison.csv` subtracts
the two single-note noise/leakage powers from the dyad difference product and
reports model/reference dBFS, dBc and error. Upper captures measure T2+T3;
lower captures measure T1+T3. A separate T3 coefficient cannot be inferred
from these two combined paths alone.

Bus candidates are normalized to an unaffected anchor inside the same
registration, so recording gain cancels out. They are fitting aids rather than
replacement resistor values. The L20 candidate uses the noise-corrected
end-to-end key-off response and must be reviewed jointly with the matching
transformer and console coupling network.

Every comparison writes `reference-quality.csv`. It checks capture duration,
clipped samples, pre-onset noise and steady-state SNR, note-on/note-off timing,
and early/late tuning drift for the isolated pedal registrations. A failing
pedal capture remains visible in the comparison reports but is barred from
`pedal-fit-candidates.csv`; warnings remain eligible and are tagged in each
candidate row.

The current gates fail captures below 3.2 seconds, with any sample at or above
0.9999 full scale, SNR below 20 dB, event timing more than 100 ms from the
protocol, tuning more than 50 cents away, or pedal drift above 5 cents.
Warnings begin at 40 dB SNR, 20 ms event error, 10 cents absolute tuning or
1 cent drift.

The three pedal captures use low C (MIDI 24), begin at 250 ms, release at
3 seconds and leave all other drawbars closed. Files `07`, `08` and `09` select
16' only, 8' only and both drawbars respectively. Match that protocol when
recording a reference console.

## Transformer reference protocol

Files `10` through `15` use only the 8' drawbar and MIDI C5/F5 (72/77). Record
single C, single F and the C+F dyad first on the upper manual, then repeat the
triplet on the lower manual. Begin at 250 ms, release at 3 seconds, capture four
seconds total, use full expression, Normal volume, maximum-bandwidth AO-28 tone,
and disable percussion, vibrato and Leslie rotation. Preserve recording gain
across all six files and do not normalize them.

For an electrical reference, the documented G-G output must be recorded only
through a correctly rated isolated differential interface by a qualified tube-
equipment technician. A powered AO-28 contains hazardous voltages. Do not open
the console, defeat its grounding, or attach ordinary audio equipment directly
to internal terminals. Microphone captures remain useful for end-to-end
comparison but cannot isolate T1/T2/T3 from the cabinet and room.

An RF-Organ-generated calibration directory may be used as the reference for a
self-check without an external manifest. Comparing a directory with itself
must yield zero deltas and envelope correlation 1.0 for all sixteen WAV files.

The generated `artifacts/` directory is intentionally ignored by Git. Curated
measurement data should only enter the repository with clear redistribution
rights and provenance in `THIRD_PARTY_NOTICES.md`.
