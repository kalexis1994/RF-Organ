# Calibration workflow

`rf-organ-lab` generates deterministic, unnormalised references from the exact
DSP used by the RackForge plugin. It is deliberately written in Rust and does
not embed third-party analysis code.

```text
cargo run --release -p rf-organ-lab -- render artifacts/calibration
```

The suite contains:

- the 91 physical generator frequencies as CSV;
- the generator's taper and eccentricity, measured key by key;
- leakage against the number of notes held;
- direct 888, percussion, scanner C3, Chorale and Tremolo phrases;
- a complete two-manual and pedal-console phrase;
- isolated low-C pedal captures for 16', 8' and both drawbars;
- an eighteen-file two-manual transformer grid: C, F and the C+F dyad at three
  documented drawbar levels, for the combined T2+T3 and T1+T3 paths;
- a nine-file T3 injection grid at three documented injection levels;
- the integrated rotary cabinet impulse response;
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
- horn and drum rise, fall and brake curves sampled every 10 ms, with the
  measured transition time and the speed range each one crossed;
- pedal harmonic levels for the eight physical contacts, plus the complete
  key-off response through the pedal filter and console electronics;
- a manifest recording version, sample rate and normalization policy.

`measurements.csv` is the compact comparison surface. The detailed
tables retain the underlying curves:

- `generator-taper.csv` plays each manual key alone through its 8' contact,
  which reaches exactly one wheel, and reports that wheel's level against the
  median of the generator together with the sideband a revolution away from
  the tone. The taper column is what a measured console replaces; the sideband
  column is the eccentricity, and at the current provisional depth it sits
  about 42 dB under the tone.
- `generator-leakage.csv` reports leakage against the number of keys held,
  measured at the frequencies of the compartment companions of the wheels
  being keyed rather than as broadband noise. The model's leakage holds a
  constant level against what is played; Hammond describes a console's as
  rising with the number of notes, so this table is where that difference will
  show against a reference capture.
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
- `rotary-rotor-response.csv` records actual and target rotor speeds, in both
  hertz and rpm, through all three transitions: slow to fast, fast to slow and
  fast to a stop. Each one starts from a settled rotor, so the measured time is
  comparable with the figure a cabinet is specified by. At the middle of the
  acceleration control the horn crosses its 360 rpm range in 1.04 s and the
  drum its 300 rpm range in 2.64 s, the extra 40 ms being the delay before a
  switched mode reaches the motor.
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
maximum-bandwidth AO-28 tone, and disable percussion, vibrato and rotary
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

## Rotary geometry protocol

`rotary-doppler.csv` and the `doppler-` and `arrival-` rows of
`measurements.csv` come from a probe that feeds one steady tone into the
cabinet, demodulates each output channel against that tone, and reads the
pitch deviation off the drift of the resulting phase. It reports, for each
rotor, the deviation it measured and the deviation the geometry predicts: the
exact one, from differentiating the path length around a full turn, and the
far-field one, which is the tangential speed of the mouth over the speed of
sound. The difference between the two channels' phases, at a known carrier, is
how much longer the sound took to reach one microphone than the other.

The drum is the clean test, because it reaches the microphones through a gain
and nothing else. At the default placement - a cardioid pair 35 cm out and
30 cm apart - it measures 21.06 cents up and 20.96 down against a predicted
21.70, and an arrival span of 539 microseconds against a predicted 551: a
shortfall of under a cent, which is the tenth of a rotation the deviation is
averaged over plus the delay line's linear interpolation. The prediction
itself was checked against an independent calculation of the same integral.

The horn measures 39.9 up and 53.0 down against the same predicted 38.5. The
excess and the lopsidedness are not the path; they are the tone shelf that
turns with the horn. Weighting a signal's low and high halves differently is a
shelving filter, and moving those weights as the rotor turns moves its phase,
which reads as pitch. It is not wrong for a horn to do this - a real one is
brighter on axis, and that colour has to be minimum-phase - but the weights
themselves are invented, so the size of the effect means nothing yet. It grows
as the microphones come closer, because the angle they see the mouth through
swings harder. What the
probe establishes is that the underlying deviation is the geometry's, and that
anything on top of it is the shaping's.

To measure a real cabinet against this, put a spaced pair at a measured
distance in front of it, play a steady tone well above 800 Hz for the horn and
well below it for the drum, and report peak deviation in cents. The radii are
the quantity to solve for: a horn whose mouth turns at radius r sweeps
1200*log2(1 + r*omega/c) cents at angular speed omega.

## Rotor stop protocol

`rotary-stop-angle.csv` and the `stop-` rows of `measurements.csv` come from
stopping the cabinet at six aimed angles, from both running speeds, and
reading where each rotor came to rest against where it was sent. The worst
error is 0.03 degrees, and a stop from the fast speed takes 2.44 seconds,
which is the drum's documented brake time plus the documented relay delay.

From the slow speed the same stops take between 0.32 and 1.99 seconds, and
that spread is the point rather than a defect. The deceleration is shaped, not
rated: the speed still falls from where it was to nothing, but along a curve
whose area is the angle that lands the mouth where it was asked to stop. The
shape can cover between a third and two thirds of what a rotor at that speed
would cover in that time, which from the fast speed is always enough to
absorb up to half a turn of adjustment. From the slow speed the rotor is only
covering a thirtieth of a turn while it stops, so an angle most of a turn away
cannot fit, and the only honest choice left is to take longer. Hammond
documents the brake time as the time to stop from the fast speed, so that is
where it is held exactly.

To measure a real cabinet against this, stop it repeatedly from each speed
with the angle set to the same value and photograph or mark where the horn
ends up. What matters is the spread across repetitions, not the absolute
angle, since zero here means the mouth facing the microphones.

## Woofer and capsule protocol

The `sweep-depth-`, `proximity-` and `capsule-` rows of `measurements.csv`
separate what the woofer does from what the capsules do.

A 200 Hz tone through a cabinet at tremolo sweeps 13.7 dB with the woofer
silent and 5.1 dB with it fully open. That direction is the whole point: the
woofer's bass never enters a rotor, so adding it adds a steady sound to a
moving one and the sweep gets shallower. A model that deepened it would have
the woofer inside the drum.

Moving the pair from 150 cm to 15 cm raises a 50 Hz tone by 13.3 dB with an
omnidirectional capsule and 2.5 dB more than that with a figure of eight. The
first figure is geometry - a stopped mouth ends up almost against the capsule -
and the second is the proximity effect, which is why it is derived from the
distance and the pattern rather than dialled in. It is bounded, and that bound
is provisional: a real capsule rolls off below its proximity rise and this one
simply stops rising.

A dynamic capsule reads 2.0 dB below a condenser at 10 kHz and 0.8 dB above it
at 4 kHz. Both numbers are provisional and describe no particular microphone.
To measure this properly you would need to know which two Hammond modelled.

## Console stage protocol

`console-injection-*.wav` are three bench signals: a 1 kHz tone at a quarter
of full scale into the preamplifier's own input, with the tone control flat
and the pedal wide open, at no drive, at the baseline drive and hard. Play the
same tone into a real AO-28 at its input, record its output, and
`rf-organ-lab compare` writes `console-stage-comparison.csv` and
`console-stage-candidates.csv`.

The fit searches for the stage character whose second harmonic matches the
reference, by rendering the injection at each candidate and comparing - the
model is the only thing that knows what a character does, so the fit asks it
rather than inverting a formula that could drift away from the engine. The
model against itself returns 1.000000 with 0.000000 dB of error, and a
character the model is not using comes back out of the fit to within 0.05,
which is the check that matters: an identity alone would also pass for a fit
that always answered with the model's own value.

The clean injection is in the comparison and out of the fit. A stage with no
drive has no curve to be lopsided about, so it says nothing about asymmetry
and would only pull the fit toward noise.

What the fit will not do is tell V4A, V4B and V3B apart. The tone passes
through all three and comes out once. Separating them needs a probe at each
stage, and until there is one the three keep the ratios they have and only
their common size moves.

## Taper protocol

`generator-taper-sweep.wav` holds every key of the upper manual in turn on the
eight foot alone, 200 ms each with 50 ms of silence after it, with everything
that could colour one key differently from another switched off. Record a
console playing the same thing and `rf-organ-lab compare` writes
`taper-comparison.csv` and `taper-fit-candidates.csv`: one line per wheel, the
level to put in `crates/rf-organ-dsp/src/taper.rs`, and a status saying
whether that wheel was readable at all.

The fit divides the reference by the model rather than by a flat line. That is
the whole of its correctness: the transformers, the tube stages and the tone
control colour every wheel too, and they are already in the model's reading,
so dividing them out is what leaves the generator. A test renders the sweep,
fits it against itself, and requires every wheel to come back at 1.000000 -
if the fit returned the console's frequency response instead, that test would
fail, and the table would have quietly acquired the whole signal chain.

Wheels the sweep cannot reach are left alone rather than guessed: the eight
foot covers the wheels the manual's keys name and no others, and the candidate
file says so per wheel. Wheels that read more than 12 dB above or 20 dB below
the middle of the sweep are refused, because a generator that far out needs a
service call rather than a table entry.

## Leakage protocol

The `leakage-` rows of `measurements.csv` play one, two, four and eight notes
and report, for each, the companions' level against the played wheels' own:
-28.2, -26.8, -24.5 and -21.0 dBc. What matters is that those four differ.
Leakage that merely followed the notes would keep the same level against them
and the four would be one number, which is what this reported before the rate
existed.

A real console gives this up easily. Play one note, record; play eight,
record; measure each compartment companion against the wheel that keyed it.
The difference between the two chords is the rate, and it is the only quantity
here that is not already provisional - the steepness RF-Organ uses was chosen
to sit in the middle of its control.

## Drive protocol

`drive-wobble.csv` and the `wobble-` rows of `measurements.csv` hold two notes
an octave and a half apart, demodulate each against its own wheel's frequency,
and read the pitch off the drift of the resulting phase. With the drive held
perfectly steady the estimator reports 0.04 cents of movement, which is its
own noise; with the drive as modelled it reports 2.90 cents on the lower note
and 2.86 on the upper. The shaft's own state goes through the same three poles
and the same block average and comes to 2.86, so what reaches the audio is
what the mechanism says it did, to within a percent and a half. Comparing
them any other way is a trap: an estimator narrow enough to be quiet reads
the once-per-revolution part away, and the missing part then looks like a
fault in the model rather than in the ruler.

That those two agree is the measurement that matters. A generator whose wheels
drifted on their own could produce any amount of wobble on each note
independently; one whose wheels are geared to a single shaft has to move them
in the same proportion at the same moment. The two tracks correlate at 0.9998.
The amount is provisional. The togetherness is the documented structure, and
it is what a real generator should be checked against: record two widely
spaced notes at once and correlate their pitch tracks, rather than measuring
either alone.

## Supply protocol

The `fifty-hertz-` and `supply-ratio` rows of `measurements.csv` settle the
cabinet on each supply and read the speeds off it. On 50 Hz the horn runs
333.3 rpm fast and 33.3 slow against its rated 400 and 40, and the drum 283.3
against 340: a ratio of 0.8333 for both, which is 50 over 60 and nothing else.
There is no fitting here and no measurement to compare against, because there
is nothing provisional in it - a synchronous motor's shaft speed is the
supply's frequency over its pole pairs, and the ratio follows.

What is worth measuring on a real cabinet is the part this does not claim:
whether a given cabinet was re-belted for its market, and what its motors
actually reach under load, which is not exactly the synchronous speed.

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

Solving the vibrato ladder directly rather than through a truncated transition
later took it from 359 to 230 ns per sample at 48 kHz, and from 510 to 224 at
192 kHz, where the cost of an exact solve does not grow with the rate. The
scanner's measured sidebands moved by a thousandth of a decibel, which is what
replacing an approximation with the thing it approximated should look like.

Placing the microphones geometrically in 0.24.0 left the cost where it was:
the rotary load adds under 20 ns per sample at every rate, because a path
length is four multiplications and a reciprocal square root with no division
in it.

Those numbers describe one machine and one build; they are a regression signal,
not a specification.

The full survey is also available as
`cargo test --release -p rf-organ-lab -- --ignored`. Ordinary test runs keep the
cheap half of it, which covers the console stages without rendering the
generator.

The generated `artifacts/` directory is intentionally ignored by Git. Curated
measurement data should only enter the repository with clear redistribution
rights and provenance in `THIRD_PARTY_NOTICES.md`.
