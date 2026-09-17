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
- an eighteen-file two-manual transformer grid: C, F and the C+F dyad at three
  documented drawbar levels, for the combined T2+T3 and T1+T3 paths;
- a nine-file T3 injection grid at three documented injection levels;
- the integrated Leslie cabinet impulse response;
- an end-to-end A4 frequency, RMS and peak probe;
- expression gain at 80 Hz, 1 kHz and 8 kHz for five pedal positions;
- AO-28 tone-control response at 100 Hz, 1 kHz and 10 kHz across ±9 dB;
- console harmonic structure against drive at two signal levels, reported per
  harmonic and as total distortion;
- five expression captures with the pedal parked at documented positions;
- direct two-tone transformer difference and third-order product levels across
  five magnetic settings;
- per-unit transformer calibration: the effective drive and memory coefficients
  T1, T2 and T3 receive from the shared character control plus their own trim,
  and the product levels each one produces at three signal levels;
- percussion envelope curves for all four tablet combinations, the attack in
  quarter-millisecond windows, the level the strike takes out of the drawbars,
  and the recovery curve after a release;
- the nine key contacts timed one drawbar at a time, at three velocities;
- 1 kHz scanner carrier and the first three sideband pairs for V1–V3 and
  C1–C3, plus the level a steady tone returns at across the audio band;
- horn and drum acceleration/braking curves sampled every 10 ms, including
  measured 63% and 90% transition times;
- pedal harmonic levels for the eight physical contacts, plus the complete
  key-off response through the pedal filter and console electronics;
- a manifest recording version, sample rate and normalization policy.

`measurements.csv` is the compact comparison surface. The detailed
tables retain the underlying curves:

- `percussion-envelope.csv` records all four tablet combinations;
- `percussion-recovery.csv` records the peak of a strike against the gap since
  the last release, which is how fast detached playing loses the effect;
- `keying-contacts.csv` records, for each drawbar contact and three
  velocities, when it closes relative to the key, the energy in the first
  10 ms against the steady tone, and that steady level. Opening one drawbar at
  a time is what makes the same measurement possible on a console;
- `scanner-sidebands.csv` records carrier level in dBFS and the first three
  sideband pairs in dBc. One scanner revolution travels out to the ninth
  terminal and back, so the modulation is not a sinusoid and the pairs do not
  fall off like a single-tone vibrato. At V3 the carrier is close to its first
  Bessel null — the taps span the whole 0.85 ms ladder, which is about 2.7
  radians of phase at 1 kHz — so the sidebands stand above what remains of it.
- `scanner-line-response.csv` records the level a steady tone returns at, in
  five bands and over a whole number of rotor revolutions, for each of the six
  positions. It shows the ladder's own lowpass corner near 7 kHz and the
  residual level difference between the vibrato and chorus positions.
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
- `console-distortion.csv` records the first four harmonics and total
  distortion of a 1 kHz tone through the console stages, for five drive
  settings at a quiet and a loud level. The second harmonic leading the third,
  and growing with the first power of level while the third grows with its
  square, is the signature of the single-ended stages; a reference console
  measured the same way is what would replace their provisional constants.
- `transformer-intermodulation.csv` drives the reduced magnetic model with
  523.3 Hz and 698.5 Hz and reports both their 175.2 Hz difference product and
  their 348.1 Hz third-order product.
- `transformer-calibration.csv` records, for each of T1, T2 and T3 and for
  three drive trims, the effective coefficients that unit receives, the
  coefficients the other two keep, and the products measured at the three
  documented levels.

## What the two intermodulation products mean

The reduced magnetic model is an odd nonlinearity: a linear input stage, a
symmetric saturation and a linear mix. An odd system produces no genuine
second-order product, so the model's own 175.2 Hz difference product is a
floor, not a signal. At C5/F5 the reading in that bin is dominated by window
leakage from the real fifth-order product at 3f_C - 2f_F = 172.9 Hz: doubling
the analysis window drops the 175.2 Hz reading by about 20 dB while 172.9 Hz
and 348.1 Hz stay put.

Fitting therefore uses the third-order product at 2f_C - f_F = 348.1 Hz, which
the model does produce (about -33 dBc at the baseline character) and which
grows monotonically with drive. The difference product is still reported for
both model and reference, because a real transformer with asymmetric hysteresis
does produce one: a reference capture whose difference product sits far above
the model's floor is evidence that the reduced model is missing the asymmetry,
not evidence of a mis-set coefficient.

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

When a transformer dyad is present, its complete single-C, single-F and C+F
triplet is required at that level. `transformer-intermodulation-comparison.csv`
subtracts the two single-note powers from the dyad in each measured bin and
reports model/reference dBFS, dBc and error for both the difference and the
third-order product. Upper captures measure T2+T3, lower captures measure
T1+T3, and the injection captures measure T3 alone.

`transformer-fit-candidates.csv` turns those measurements into coefficients.
For each path it searches the drive trim whose model prediction best matches
the reference across the available levels, and reports the candidate trim, the
effective drive it implies, the RMS and worst residual, and a self-check: how
far the short fitting probe lands from the model's own four-second captures at
zero trim, which bounds how much of a residual belongs to the probe rather than
to the reference.

The order is deliberate. T3 is fitted first, and only from injection captures.
T2 and T1 are then fitted from the upper and lower captures with T3 held at
that value. Without an injection triplet the manual paths stay
`underdetermined-requires-t3-injection` and no number is produced: two combined
paths cannot separate three transformers. Only the drive trim is searched; the
memory coefficient barely moves these products and stays where the character
control puts it until a measurement that separates it exists.

A synthetic reference set with known coefficients validates the chain:

```text
cargo run --release -p rf-organ-lab -- render artifacts/synthetic \
  --transformer-drive-trims 0.10,-0.15,0.20
cargo run --release -p rf-organ-lab -- compare \
  artifacts/calibration artifacts/synthetic artifacts/fit-check
```

recovers T1 = 0.100, T2 = -0.150 and T3 = 0.200 with residuals below 0.01 dB.
The flag exists for exactly this check; the calibration render uses no trim,
and the value used is recorded in `manifest.txt`.

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

Files `10` through `27` use only the 8' drawbar and MIDI C5/F5 (72/77). Each
triplet is single C, single F and the C+F dyad. Record the three triplets of
the upper manual first, then the three of the lower manual. The three levels
are 8' drawbar positions 4, 6 and 8, one 6 dB step apart:

| files | manual | level | 8' drawbar |
| --- | --- | --- | --- |
| 10-12 | upper | low | 4 |
| 13-15 | upper | nominal | 6 |
| 16-18 | upper | high | 8 |
| 19-21 | lower | low | 4 |
| 22-24 | lower | nominal | 6 |
| 25-27 | lower | high | 8 |

Begin at 250 ms, hold for at least 2.5 seconds, release at 3 seconds and
capture four seconds total. Use full expression, Normal volume,
maximum-bandwidth AO-28 tone, and disable percussion, vibrato and Leslie
rotation. Preserve one recording gain across all eighteen files and do not
normalize them: the drawbar step is the measured quantity.

Files `28` through `36` are the optional T3 injection grid, three triplets at
injected levels one 6 dB step apart. They are the only captures that separate
T3 from T1 and T2. The model renders them as the two tones through the output
transformer alone at 0.045, 0.09 and 0.18 full scale, gated like a keyed note
with a 5 ms ramp.

**Bench work, qualified technicians only.** Injecting a signal ahead of T3 and
recording the documented G-G output requires opening a powered AO-28, which
contains hazardous voltages. It must be done only by a qualified tube-equipment
technician, only through a correctly rated isolated differential interface, and
the injected level in volts must be recorded in the manifest `notes` field: the
fit maps a level to a coefficient, so an undocumented injection level makes the
candidate meaningless. Do not defeat the console's grounding or attach ordinary
audio equipment to internal terminals. Microphone captures of files `10`
through `27` remain useful for end-to-end comparison but cannot isolate
T1/T2/T3 from the cabinet and room.

An RF-Organ-generated calibration directory may be used as the reference for a
self-check without an external manifest. Comparing a directory with itself must
yield zero deltas and envelope correlation 1.0 for all thirty-seven WAV files,
and every fit candidate must come back at a zero trim.

## Percussion reference protocol

Files `37` through `40` record percussion on the upper manual with drawbars
888 and nothing else out, the third harmonic selected, middle C (MIDI 60),
contact at 250 ms and release at 3 seconds. The four files are Normal/Fast,
Normal/Slow, Soft/Fast and Soft/Slow, and they must share one recording gain:
the level difference between a Normal and a Soft file is itself the
measurement of the documented drawbar cut.

The 888 registration leaves the 2 2/3' bus empty, so the third-harmonic strike
stands alone in the spectrum at the generator frequency of that bus and its
decay can be read directly. `percussion-comparison.csv` reports, for model and
reference, the steady drawbar level at the fundamental, the strike level
against it in dBc, and the decay time. The decay is fitted between 5 and 25 dB
below the peak and extrapolated to 60 dB, because a real capture rarely has
60 dB of clean decay; what sits in the bin once the strike has gone is
subtracted in power first, so a leaky generator or a noisy room does not stop
the fit.

`percussion-fit-candidates.csv` needs no search: the decay time, the strike
level and the drawbar cut are exactly what the measurement produces, so the
reference value is the candidate. The drawbar cut it reports is the end-to-end
one, measured through the matching transformer and console, and comes out
slightly under the 6 dB the tablet takes at the manual.

## Expression reference protocol

Files `41` through `45` park the swell pedal at five documented positions —
one eighth, one quarter, one half, three quarters and fully open — with every
upper drawbar out and two keys held two octaves apart, C3 and C6. That
registration puts energy on the 16' bus of the low key and on the 8' and 1'
buses of the high one, so one capture can be read at a low, a middle and a
high frequency. All five share a recording gain, and every reading is relative
to the fully open capture, so the gain of the session cancels.

`expression-comparison.csv` reports each band at each position for model and
reference. `console-fit-candidates.csv` then fits the one coefficient the
model exposes for this network: how much of the pedal's attenuation the middle
band takes compared with the ends.

That fit is biased and says so. The estimator evaluates the console
electronics alone, while a capture also carries the matching and output
transformers, whose compression moves with level and therefore with the pedal.
The comparator fits the model's own captures first, where the answer is known,
and reports the offset as `estimator_bias` before taking it back off the
reference fit. Comparing a directory with itself returns the model's own
coefficient exactly, with the bias visible beside it — presently -0.12, which
is how much the transformers move the answer.

## Sample rate and cost

Every other probe in this document runs at 48 kHz. The sweep checks that the
engine means the same thing at the rest of the supported range:

```text
cargo run --release -p rf-organ-lab -- sweep artifacts/sweep
```

`sample-rate-invariance.csv` measures one quantity per subsystem — generator
tuning, chord level, percussion T60, scanner sideband, horn rate and 63%
transition, expression and tone gain, transformer third-order product and pedal
key-off time — at 32, 44.1, 48, 96 and 192 kHz, and reports the worst deviation
from the 48 kHz column against a per-quantity tolerance. A quantity that drifts
with the rate is a coefficient written in samples where it belongs in seconds or
hertz, so the command exits non-zero and names it. At 0.16.0 all eleven hold:
the largest deviations are 10 ms on the percussion T60, which is one analysis
window, and 0.05 dBc on the scanner sideband, which the LC ladder reproduces at
every rate.

`performance.csv` walks five loads at each rate, each adding one subsystem to
the one before it — an idle generator, then thirteen keys held across both
manuals and the pedals, then percussion, then both manuals switched into the
vibrato line, then the rotary cabinet — and reports the real-time factor, the
cost per sample and what each subsystem adds. Subtracting one row from the
previous one is what makes it a guide for where optimisation is worth
spending.

The first breakdown paid for itself immediately. The idle generator cost
2058 ns per sample, three quarters of a fully engaged console, and it cost the
same at every sample rate — the mark of work that does not depend on the audio.
It was the contact scan: 549 manual contacts, 549 more on the lower manual and
200 on the pedals, all ticked every sample whether or not anything was
happening to them. Keys now drop out of the scan once their contacts settle,
which is exact because a settled contact's gate already sits where its key put
it. Idle fell to 612 ns per sample and the fully engaged console went from
7.4x to 19.1x real time at 48 kHz, and from 1.8x to 4.3x at 192 kHz, with the
whole calibration suite rendering byte for byte identically.

Those numbers describe one machine and one build; they are a regression signal,
not a specification.

The full survey is also available as
`cargo test --release -p rf-organ-lab -- --ignored`. Ordinary test runs keep the
cheap half of it, which covers the console stages without rendering the
generator.

The generated `artifacts/` directory is intentionally ignored by Git. Curated
measurement data should only enter the repository with clear redistribution
rights and provenance in `THIRD_PARTY_NOTICES.md`.
