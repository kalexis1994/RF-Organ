# Third-party notices

RF-Organ is an independent Rust implementation. It does not compile or embed
source code from the projects below. Their publications, data tables and source
code are used as technical references and are credited here as implementation
work progresses.

## setBfree

- Project: https://github.com/pantherb/setBfree
- License: GPL-2.0-or-later
- Authors include Fredrik Kilander, Robin Gareus and Will Panther.
- Reference use in version 0.1.0: 60 Hz gear-ratio table, 91-wheel generator
  numbering, B-3-style foldback rules and physical tonewheel compartment pairs.
- RF-Organ translates the relevant concepts and tables into safe Rust; it does
  not link the setBfree C implementation.

## Dynamic contact-envelope research

- Paper: Giulio Moro, Andrew P. McPherson and Mark B. Sandler, "Dynamic
  temporal behaviour of the keyboard action on the Hammond organ and its
  perceptual significance," JASA 142(5), 2017.
- Research code: https://github.com/giuliomoro/setBfree/tree/dynamic-envelopes
- License: GPL-2.0-or-later.
- Reference use in version 0.1.0: velocity-dependent contact spread and bounded
  contact chatter. The current implementation is a provisional deterministic
  model, not a reproduction of the published experimental fit.

## Published physical-model references

See `docs/RESEARCH.md` for the Hammond service documentation, tonewheel,
scanner-vibrato and Leslie papers that define the research baseline.

