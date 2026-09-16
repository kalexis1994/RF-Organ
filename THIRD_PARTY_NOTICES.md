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

The scanner implementation is an independent reduced Rust model informed by
the Hammond service documentation, patent US2560568A and Werner, Dunkel and
Germain's DAFx-2016 wave-digital-filter paper. Version 0.2 uses the documented
16-contact scanning topology and console switch semantics, but does not copy
the paper's WDF equations or third-party implementation code.

See `docs/RESEARCH.md` for the Hammond service documentation, tonewheel,
scanner-vibrato and Leslie papers that define the wider research baseline.

## Hammond pedal-switch service documentation

Version 0.9.0 translates the documented late-console 25-note pedal topology
into an independent reduced Rust model: eight harmonic contacts, four busbars,
and the 470/47/10-ohm and 20/5/5-ohm drawbar-mixing branches shown for B-3/C-3
consoles. The L20 filter is presently represented by a provisional bounded
one-pole pending component measurement.

## Hammond AO-28 service documentation

Version 0.10.0 translates the documented B-3/C-3 console routing into an
independent Rust signal graph. Separate magnetic states represent the T2 upper
and T1 lower-plus-pedal matching transformers before the vibrato tablets. The
percussion channel bypasses the matching transformers and scanner and joins
the signal at the V4A summing stage. Component values and tube stages remain
reduced models pending reference-console calibration; no service-manual
artwork or third-party source code is embedded.
