# Architecture

## Signal path

```text
MIDI / automation
      |
91 continuously rotating tonewheels
      |
61 keys x 9 physical contacts
      |
drawbar buses + foldback + leakage
      |
shared matching transformer
      |
expression
      |
integrated Leslie (crossover, horn, drum, inertia, Doppler, stereo radiation)
      |
stereo output
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

## Version 0.1.0 limitations

- Upper manual only; lower manual and pedalboard are reserved for the next
  structural milestone.
- Scanner vibrato and percussion are not yet implemented.
- Contact bounce, matching-transformer and Leslie constants are provisional.
- Leslie cabinet reflections and angle-dependent measured filters are not yet
  included.
