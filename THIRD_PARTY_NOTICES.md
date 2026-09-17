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

The vibrato/chorus is an independent Rust implementation of the documented
B-3 circuit, informed by the Hammond service documentation, patent US2560568A
and Werner, Dunkel and Germain's DAFx-2016 wave-digital-filter paper.

Version 0.2 used a reduced delay-line stand-in. Version 0.16.0 replaces it with
the circuit itself: eighteen 500 mH sections, seventeen 0.004 µF shunt
capacitors and a final 0.001 µF one, the six tap dividers, the 15 kΩ
termination, the 22 kΩ source resistor that the vibrato/chorus switch shorts,
the nineteen tap nodes, the three depth tap sets and the sixteen-stack scanner
that crossfades between adjacent terminals. Those component values and tap
tables are Hammond service-manual data, tabulated in Table 1 and Table 2 of the
DAFx-2016 paper and used here as data, not as code.

The discretisation is our own and deliberately different from the paper's: the
ladder is linear, so RF-Organ solves it once with the trapezoidal rule into a
fixed state transition rather than building a wave-digital tree. No WDF
adaptor, scattering equation or third-party implementation code is reproduced.
The insertion-loss make-up that keeps the chorus position usable is a
provisional stand-in for the AO-28 vibrato amplifier, derived from the
resistor network rather than fitted.

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

Version 0.11.0 separates reduced pre- and post-expression nonlinear stages and
uses the documented 60 pF-per-section expression control with R34's 15 MOhm
value to expose a calibratable low-frequency corner. Low/mid/high attenuation
remains a reduced model informed by Hammond's published description of
expression as both volume and tonal control; it is not represented as a solved
AO-28 circuit.

Version 0.12.0 places a reduced 200 Hz tone shelf after V4B, followed by a
separate V3B/12BH7 stage and T3 magnetic state. The physical console offered
tone cut only; RF-Organ retains the existing bipolar calibration parameter and
uses Hammond's documented modern ±9 dB extension around neutral. The response
and output-transformer coefficients remain provisional pending measurement.

Version 0.13.0 adds an independently written two-tone transformer probe and
upper/lower reference-capture triplets based on Hammond's published C/F
difference-product listening test. It reports combined T2+T3 and T1+T3 paths;
no individual transformer coefficient is presented as measured or fitted.
