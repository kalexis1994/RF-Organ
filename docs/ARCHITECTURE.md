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
shared matching transformer
      |
scanner vibrato / chorus
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

## Version 0.3.0 limitations

- Pedal keying and drawbar topology are present, but pedal sustain and console
  matching-network constants remain provisional.
- Scanner and percussion topology are implemented, but their electrical
  constants are not yet calibrated against a reference console.
- Contact bounce, matching-transformer and Leslie constants are provisional.
- Leslie cabinet reflections and angle-dependent measured filters are not yet
  included.
