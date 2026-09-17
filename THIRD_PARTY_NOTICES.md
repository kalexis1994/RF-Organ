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

## Branding artwork

The icon, banner and splash in `package/branding/` are drawn by
`tools/make-artwork.py` from geometry and from the play surface's own palette.
They contain no lettering and no third-party image, so no font or artwork
licence applies to them. Their subject is the drawbar layout every tonewheel
console shares - two brown, four white and three black - which Hammond's
manual refers to when it calls the 2 2/3' the first black drawbar.

## Leslie cabinet documentation

Version 0.21.0 takes the mechanical description Hammond publishes for a
digital Leslie: the horn turns counter-clockwise and the drum clockwise; slow
speeds run from 20 to 120 rpm and fast speeds from 200 to 500 rpm; a rise, a
fall and a brake time are separate per rotor, floored at 0.8 s for the horn and
1.0 s for the drum and ceilinged at 12.5 s; a mode switch can be delayed by up
to a second before the rotor responds; and a time means the time to cross the
whole speed range, so a shorter move takes proportionally less and the rate is
what stays constant.

RF-Organ runs its rotors at 400 and 40 rpm for the horn and 340 and 40 for the
drum, the figures usually quoted for a 122 and the ones Hammond's own worked
examples use. The six transition times are provisional, inside the documented
floors, and the laboratory measures what they produce. The stop angle and the
microphone geometry Hammond specifies in centimetres are not modelled yet.

## Generator compartments and wheel motion

Version 0.20.0 takes three documented things about the generator.

The B-3/C-3 service manual describes the generator as divided into
compartments, each holding four tone wheels driven by one gear and shielded
magnetically from the rest, and lays the compartments out by tooth count: one
holds 2, 32, 8 and 128, the next holds 4, 64, 16 and 192, and a speed with no
192-tooth wheel leaves that position blank. RF-Organ derives its compartment
table from that layout, so leakage now reaches a wheel from up to three
companions instead of the single four-octave partner it used before, and every
wheel belongs to a compartment where five previously belonged to none.

Hammond's XK-7/XK-7D manual describes what differs between individual
generators — level, wow and flutter, and eccentricity — and defines them: wow
is a once-per-revolution change of pitch or phase caused by gear backlash, and
eccentricity is a wheel stamped off-centre whose high spots pass nearer to and
further from the pickup once per revolution, so the tone becomes slightly
louder and softer. RF-Organ models the eccentricity as that once-per-revolution
level change, at a depth that is provisional; wow and flutter are not modelled
yet.

Münster and Pfeifle's ISMA-2019 measurements of a Model A report the induced
voltage as approximately sine-like with strong amplitude fluctuation caused by
the unsteady motion of the wheels, which is the same effect from the other
side. Their paper gives no per-wheel table, and the service manual's own
generator output voltages are in a scan that does not survive character
recognition, so the per-wheel taper stays flat and exposed rather than
invented.

## AO-28 stage shape

Version 0.19.0 records what could not be sourced. The B-3/C-3 preamplifier
schematic in the service manual available to this project is a scan whose
optical character recognition returns unusable fragments for the component
labels, and the parts list covers assemblies rather than resistor and
capacitor values. The AO-28's stage constants therefore cannot be ported the
way the vibrato ladder's were, and they stay provisional.

What the stages did get is their shape. A single-ended triode's plate current
follows roughly a three-halves power of grid voltage, so its transfer curve is
asymmetric and its distortion is led by the second harmonic, with the third
falling away as the square of level. That is textbook tube behaviour rather
than anything specific to Hammond, and it is stated here as such. The stages
previously used a symmetric soft clipper, which put the third harmonic 29 dB
above the second; they now put the second ahead by 19 to 42 dB across the
normal range. The asymmetry of each stage remains a provisional number, and
the laboratory reports the harmonic structure it produces.

## Hammond percussion documentation

Version 0.17.0 takes the percussion behaviour Hammond documents for the
vintage console in the XK-7/XK-7D owner's manual: the 1' drawbar is cancelled
while percussion is on, the second harmonic is the 4' bus and the third is the
2 2/3' bus, the envelope is single-trigger so that re-keying is required, and
at Normal volume the upper drawbars are reduced "by a small amount (about
6 dB)" while Soft leaves them alone. The 6 dB figure is the manufacturer's,
and RF-Organ applies it as such.

Hammond documents no decay times, attack time or recovery behaviour in
seconds, so those constants stay provisional and are exposed as measurements
instead: the laboratory reports the four decay curves, the attack, the
recovery after release and the nine contact closure times, and the comparator
turns a reference capture into candidate values for them directly.

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
