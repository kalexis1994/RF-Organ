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

## Version 0.12.0 limitations

- The classic B-3 pedal contact and resistor-panel topology is present. The
  L20 pedal-filter coefficient and console matching-network constants remain
  provisional; musical pedal sustain is intentionally absent because it is a
  feature of later digital Hammond instruments, not the electromechanical B-3.
- Scanner and percussion routing follows the AO-28 schematic, but their
  electrical constants are not yet calibrated against a reference console.
- Contact bounce, matching-transformer, console tube-stage, expression-network
  and Leslie constants are provisional.
- Leslie cabinet reflections and angle-dependent filters are present, but
  their coefficients are not yet fitted to multi-angle cabinet measurements.
