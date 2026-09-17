# Architecture

## Signal path

```text
MIDI / automation
      |
91 continuously rotating tonewheels
      |
upper + lower manuals and pedal contacts
      |
independent drawbar buses + foldback + leakage
      |
upper -> T2 matching transformer ----+
                                     +-> vibrato tablets -> scanner/direct --+
lower + pedal -> T1 transformer -----+                                     |
                                                                           +-> V4A sum
percussion amplifier ------------------------------------------------------+      |
                                                                                  v
                                                 preamp + expression + tone + V3B + T3
                                                                                  |
                                                            integrated Leslie + output
```

## Boundaries

`rf-organ-dsp` owns the instrument and contains no RackForge ABI knowledge.
`rf-organ-plugin` translates sample-accurate host events, programs and state
into calls on the DSP engine.

The Leslie is not a separately installed effect. It is a self-contained Rust
module inside `rf-organ-dsp`, which gives the user one instrument while keeping
the rotating-speaker model independently testable and replaceable.

## Research and production models

The production engine uses bounded recurrences, fixed-size arrays and reduced
models. Higher-cost electromagnetic, circuit and measurement work will live in
future analysis/laboratory crates and produce calibrated coefficients for the
production engine.

The two magnetic states are independent, matching the documented T1 and T2
paths. Percussion does not pass through either matching transformer or the
scanner; it joins the non-vibrato and scanner-return channels at the V4A sum.

The console electronics place a reduced V4A stage before the capacitive swell
network and a reduced V4B stage after it. The expression network exposes its
section capacitance and low-frequency corner to the laboratory so future
reference captures can replace provisional response coefficients.

After V4B, the AO-28 tone control applies a broad shelf above 200 Hz. A reduced
V3B/12BH7 stage drives T3, whose magnetic state is independent from the T1/T2
input matching transformers. Plugin output level remains outside this physical
console path as a host-facing master control.

## Transformer calibration

One musical `Transformer Drive` / `Transformer Memory` pair sets the character
of all three transformers. Each unit then adds its own bipolar trim, so a
measurement of T1, T2 or T3 can be applied without moving the others; the sum
is clamped to the model range. All trims default to zero, which reproduces the
shared coefficients earlier versions used.

The laboratory renders the upper and lower C/F/dyad triplets at three drawbar
levels, plus an optional T3 injection grid at three injected levels. The manual
triplets measure the combined T2+T3 and T1+T3 paths. T3 is identified only from
the injection captures; the comparator fits T3 first and only then resolves T2
and T1, and reports the manual paths as underdetermined when no injection
triplet exists.

## Vibrato and chorus

The vibrato/chorus is the documented circuit rather than a modulated delay: an
eighteen-section LC ladder, terminated and tapped at nineteen nodes, scanned by
a sixteen-stack capacitive rotor that crossfades between adjacent terminals as
it travels out to the ninth terminal and back. The depth tablet selects which
ladder nodes the nine terminals reach; the vibrato/chorus tablet shorts or
restores the 22 kΩ source resistor, and the chorus character follows from that
rather than from mixing a dry copy.

The ladder is linear and its coefficients never change while audio runs, so it
is discretised once per switch position with the trapezoidal rule. Each sample
is one banded matrix-vector product: neighbouring sections occupy neighbouring
state indices, and entries below a documented threshold are dropped, which a
test checks against the dense transition over a full second.

## Percussion and keying

Percussion is single-trigger, taken from the upper manual's 4' bus for the
second harmonic and its 2 2/3' bus for the third, and it cancels the 1'
drawbar while it is on. At Normal volume it also takes about 6 dB out of the
upper drawbars, which is Hammond's documented figure for the vintage console;
Soft leaves them alone.

The envelope is not a gate. It rises through a short attack instead of
stepping, and the supply it draws from only recovers once every key is up, so
a console played fast and detached delivers less percussion on each note than
one played with gaps. Both time constants are provisional and the laboratory
measures them: the attack in quarter-millisecond windows, the recovery as peak
level against the gap since the last release.

Keying reaches the generator through nine contacts per key, which close over a
few milliseconds with the highest first, and the laboratory times them one
drawbar at a time so the same protocol can be run on a console.

## Cost

A console has 1298 key contacts and almost none of them are doing anything at
any given moment, so the engine scans only the keys whose contacts have not
settled. A settled contact has no delay left to count down, no bounce left and
a gate already at its key's position, so skipping it cannot change the output;
the laboratory confirms that by rendering the whole calibration suite byte for
byte identically across the change.

What remains when nothing is keyed is the generator itself, which is
inherent: the 91 wheels turn whether or not anyone is playing.

## Console stages

V4A, V4B and V3B are single-ended stages, and RF-Organ shapes them as such: a
cubic soft clip whose own distortion is third order, followed by a rational
asymmetric curve whose expansion is second order to the first power of level
and third order to its square. Cascaded, they put the second harmonic well
above the third at normal levels and let the third overtake only when the
drive control is pushed into hard clipping, which is what a single-ended
triode does.

The expression network sits between V4A and V4B, so the pedal changes how hard
the later stages are driven rather than scaling the output. Its one fitted
parameter is how much of the pedal's attenuation the middle band takes
compared with the ends; the laboratory fits it from captures with the pedal
parked at documented positions, and reports the bias of that estimator by
fitting the model's own captures first.

## Generator

The 91 wheels turn continuously and each one carries its own output level, so
a measured generator's taper can be applied wheel by wheel; it is flat until
one is measured. Each wheel also carries a once-per-revolution level change for
the eccentricity Hammond documents, at a rate of its tone divided by its tooth
count — about 16 Hz at the bottom of the generator and 31 Hz at the top.

Leakage follows the compartments the service manual describes: four wheels to a
compartment, grouped by tooth count, so a wheel hears up to three companions
rather than one partner. The table is built at compile time because the keyed
path reads it every sample.

## Version 0.20.0 limitations

- The classic B-3 pedal contact and resistor-panel topology is present. The
  L20 pedal-filter coefficient and console matching-network constants remain
  provisional; musical pedal sustain is intentionally absent because it is a
  feature of later digital Hammond instruments, not the electromechanical B-3.
- Percussion routing follows the AO-28 schematic and Hammond's documented
  console behaviour, but its decay times, attack and recovery are provisional:
  the manufacturer publishes no figures in seconds for them.
- Contact timing is a provisional deterministic model. The published keyboard
  action study is behind a paywall, so the laboratory measures the spread and
  the click rather than claiming a fitted one.
- The AO-28 component values are not available: the schematic in the service
  manual reachable from `docs/RESEARCH.md` is a scan that does not survive
  character recognition, and its parts list covers assemblies rather than
  resistors and capacitors. Every console stage constant is therefore
  provisional, with a measurement surface and a reference protocol rather than
  a fitted value.
- The generator taper is flat. The service manual's table of generator output
  voltages is in the same unreadable scan, so no per-wheel levels are claimed;
  the laboratory measures the taper the model produces and the API applies one
  wheel at a time once a console is measured.
- Leakage keeps a constant level against what is played, while Hammond
  describes it as rising with the number of notes held and exposes a control
  for that rate. The laboratory reports leakage against note count, which is
  where that gap will be closed.
- Wow and flutter are not modelled. Eccentricity is, at a provisional depth.
- The vibrato line uses the documented component values with an unwarped
  bilinear transform, so its cutoff is placed by the components rather than
  fitted; the scanner runs at 6.9 Hz and its plate overlap is modelled as a
  linear crossfade. The AO-28 vibrato amplifier that drives and recovers the
  line is not modelled: a make-up gain derived from the circuit stands in for
  it, which is what keeps the chorus position from arriving 11 dB down.
- Contact bounce, matching-transformer, console tube-stage, expression-network
  and Leslie constants are provisional.
- The transformer model is an odd nonlinearity and therefore produces no
  genuine second-order difference product. Its asymmetric magnetic behaviour is
  not modelled yet, so the fit uses the third-order product; see
  `docs/CALIBRATION.md`.
- Leslie cabinet reflections and angle-dependent filters are present, but
  their coefficients are not yet fitted to multi-angle cabinet measurements.
- The engine is measured as sample-rate invariant from 32 to 192 kHz. A fully
  engaged console runs about 19x real time at 48 kHz and 4.3x at 192 kHz on the
  development machine. The largest remaining costs are the generator, which is
  inherent, and the vibrato ladder, for which an exact banded solve would be
  cheaper than the banded transition it uses today.
