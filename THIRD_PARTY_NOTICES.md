# Third-party notices

RF-Organ is an independent Rust implementation. It does not compile or embed
source code from the projects below. Their publications, data tables and source
code are used as technical references and are credited here as implementation
work progresses.

## Trademarks

HAMMOND, B-3 and LESLIE are trademarks of Hammond Suzuki. RF-Organ is not
affiliated with, endorsed by or sponsored by Hammond Suzuki or any of its
subsidiaries. Those names appear in this repository only where they are needed
to say factually which published instrument or document a model was derived
from, which is how the rest of these notices work. They are not used as the
name of this product, of any of its modules, of any of its controls or of any
of its presets: the module that models a rotating-baffle cabinet is called the
rotary speaker throughout, and no logo, typeface or other branding of those
marks is reproduced anywhere.

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
ladder is linear and, with its states interleaved section by section,
tridiagonal, so RF-Organ applies the trapezoidal rule and solves the resulting
system directly each sample rather than building a wave-digital tree. No WDF
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

## Rotary cabinet documentation

Version 0.21.0 takes the mechanical description Hammond publishes for a
digital rotating cabinet: the horn turns counter-clockwise and the drum
clockwise; slow speeds run from 20 to 120 rpm and fast speeds from 200 to
500 rpm; a rise, a fall and a brake time are separate per rotor, floored at
0.8 s for the horn and 1.0 s for the drum and ceilinged at 12.5 s; a mode
switch can be delayed by up to a second before the rotor responds; and a time
means the time to cross the whole speed range, so a shorter move takes
proportionally less and the rate is what stays constant.

Version 0.32.0 adds the last setting on that page: the step past the widest
pair in the width control, which the manual calls "Side" and describes as
placing the microphones "on each side of the cabinet" rather than in front of
it. RF-Organ makes it a switch of its own instead of the last step of the
width, so that the width beside it stays a width all the way along; the
setting is the documented one either way. What it needs and the manual does
not give on that page is how wide the cabinet is, since that is what a
microphone put beside it has to clear, so the 36 cm half-footprint is
provisional.

Version 0.31.0 gives them a stand each as well. That manual's width, centre
and distance are all listed per rotor, and RF-Organ had one of each shared
between the two pairs, with the drum's centre wired to the negative of the
horn's. The negation came from a note in the manual - that a positive value on
the horn and a negative one on the drum emphasise the different directions the
baffles approach from - which is advice to whoever is placing the microphones,
not something the cabinet does on its own. Both pairs stand where they are put
now, and that advice is available rather than enforced.

Version 0.30.0 gives the horn and the drum a volume each, which is how that
manual has them: three microphone volumes in decibels, from unity down through
-76 to silence, one for each of the horn's pair, the drum's pair and the
woofer. Before this the first two shared one bipolar balance between them,
which could only ever turn one of them down.

Version 0.29.0 adds the woofer and the capsules, both from the microphone page
of the same manual. The woofer's sound is described there as dry and
unmodulated, as not leaving the cabinet directly, and as being picked up by the
drum's microphone primarily and the horn's slightly; its level is one of three
the manual gives in decibels from silence to unity. How much less the horn's
pair hears is not a number it publishes, so the quarter RF-Organ uses is
provisional, as is where the level sits by default. The two capsules are given
only as characters - a dynamic one that "enhances the sense of perspective" and
a condenser one that is "natural" - with no response and no model named. The
part of that which is not taste is in the geometry: a pressure-gradient capsule
lifts the bass as it nears a source, by an amount the distance and the pattern
decide, so RF-Organ derives it rather than choosing it, and bounds it because a
real capsule rolls off underneath. The presence and the top that separate the
two characters are provisional, and the laboratory reports what they come to.

Version 0.28.0 adds the supply the motors run from. This one is not from a
Hammond document: the speeds are, and the rest is how an alternating-current
motor works, which is that its shaft turns at the supply's frequency over its
pole pairs. So the speeds a cabinet is quoted at belong to the supply it was
built for, and the same cabinet on another supply turns in proportion - five
sixths of everything on fifty cycles against sixty. What RF-Organ models is
one cabinet on either supply, which is a real thing that happens to a cabinet
that travels; it is not the two cabinets a manufacturer would sell into the
two markets, because an exported one was re-belted or re-motored to reach its
rated speeds there. The console's own generator is left at concert pitch and
does not follow this switch.

Version 0.26.0 adds the stop angle from the same description: each rotor has
one, documented as a whole degree from 0 to 359 or "Rnd" for a random angle,
and the brake time is documented specifically as the time to stop from the
fast speed. Both are modelled, and where they conflict the engine keeps the
documented brake time and lets the stop from a slower speed run long; see
`docs/CALIBRATION.md`.

RF-Organ runs its rotors at 400 and 40 rpm for the horn and 340 and 40 for the
drum, the figures usually quoted for a 122 and the ones Hammond's own worked
examples use. The six transition times are provisional, inside the documented
floors, and the laboratory measures what they produce.

## Rotating source geometry

Version 0.24.0 places the microphones by the same geometry Smith, Serafin, Abel
and Berners set out in "Doppler Simulation and the Leslie" (Proceedings of the
5th International Conference on Digital Audio Effects, Hamburg, 2002): the
radiating mouth is treated as an omnidirectional point on a circle, the
listener as a point off that circle, and the pitch deviation as what the
changing distance between them does to the arrival time, whose far-field form
is the tangential speed of the radiating point over the speed of sound. The
paper also gives the reason the omnidirectional assumption is fair, which is
the diffuser fitted into the mouth of the horn.

Nothing was taken from that paper but its description of the physics, which is
not a copyrightable element and carries no licence obligation; no code, figure
or table of it is reproduced here, and the implementation in
`crates/rf-organ-dsp/src/leslie.rs` is our own.

The microphone placement follows the ranges Hammond publishes for a digital
cabinet: 0 to 170 cm in front, 0 to 40 cm between the pair, and an offset of
the pair from the rotor's pivot. RF-Organ exposes those as distances in metres,
the way RackForge's Concert Grand exposes its own microphone placement, and
adds a pattern control from omnidirectional through cardioid to a figure of
eight, which on a rotating source decides how much of the sweep arrives as
level and how much as tone.

What is not documented anywhere we can cite is the radius each mouth actually
turns at. So the horn's 18 cm and the drum's 12 cm are not constants: they are
the defaults of two controls on a model page, in metres, where a measurement
can replace them without a rebuild. `artifacts/measurements.csv` reports the
pitch deviation they produce against the deviation the same geometry
predicts.

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
scanner-vibrato and rotating-cabinet papers that define the wider research
baseline.

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
