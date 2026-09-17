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

## Version 0.16.0 limitations

- The classic B-3 pedal contact and resistor-panel topology is present. The
  L20 pedal-filter coefficient and console matching-network constants remain
  provisional; musical pedal sustain is intentionally absent because it is a
  feature of later digital Hammond instruments, not the electromechanical B-3.
- Percussion routing follows the AO-28 schematic, but its electrical
  constants are not yet calibrated against a reference console.
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
- The engine is measured as sample-rate invariant from 32 to 192 kHz, but a
  fully engaged console leaves little headroom at 192 kHz: roughly 1.7x real
  time on the development machine, against 2.1x before the vibrato line landed.
  The ladder is the largest single cost and an exact banded solve would be
  cheaper than the banded transition it uses today.
