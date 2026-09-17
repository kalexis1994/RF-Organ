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
                                                            integrated rotary + output
```

## Boundaries

`rf-organ-dsp` owns the instrument and contains no RackForge ABI knowledge.
`rf-organ-plugin` translates sample-accurate host events, programs and state
into calls on the DSP engine.

The rotary speaker is not a separately installed effect. It is a self-contained Rust
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

The ladder is linear and its coefficients never change while audio runs, so
each switch position is discretised once with the trapezoidal rule. Interleaving
the states section by section makes it tridiagonal — a capacitor sees the
currents either side of it, an inductor the voltages either side of it, and
nothing reaches further — so a sample is three multiplications per row for the
explicit half and a forward and back sweep for the implicit one. That solve is
exact rather than truncated, and a test holds it against the dense transition
for a second of impulse response at every supported sample rate.

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

## Rotary cabinet

The horn turns counter-clockwise and the drum clockwise, at 400 and 340 rpm
fast and 40 rpm slow. Each rotor has its own rise, fall and brake time, and a
time means what Hammond says it means: the seconds needed to cross the whole
speed range. The rate is therefore what stays constant, so braking from the
slow speed takes a tenth of the time braking from fast does, and the drum takes
several times longer than the horn either way. A mode switch waits briefly
before the rotor responds, as a relay and a clutch do.

The play surface draws the cabinet from above. It does not animate on a
timer: it runs a second copy of the engine's own rotor model, clocked by the
browser rather than by the audio device, so the speeds, the ramps and the stop
angles it shows are the ones the audio is using. Its frame follows the
microphones, so moving them back zooms out instead of pushing them off the
edge. The parameter table the surface validates against is the engine's, not a
copy of it; a range that moves in the DSP moves there too.

## Version 0.34.0 limitations

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
- Leakage grows faster than what is played, at a rate that is a control of
  its own, which is how Hammond describes it. Measured against the notes that
  caused it, the leakage climbs 7.2 dB between one note and eight; with the
  rate at zero it stays within 1.5 dB, which is where this was before. The
  steepness is provisional.
- The drive is resiliently coupled at every joint, which is what the service
  manual describes, so the shaft does not turn perfectly evenly and the 91
  wheels geared to it stray together. The depth, the rate the motor's coupling
  swings at and where the coil springs resonate are all provisional; what is
  not provisional is that it reaches every wheel at once, which the laboratory
  measures as a correlation of 0.9998 between two notes an octave and a half
  apart. Per-assembly scatter is not modelled: the 48 assemblies are built the
  same way, so they are one filter rather than 48 identical ones.
- The six transition times are provisional, though inside the documented
  range; before 0.21.0 the horn ramped in 0.35 s, which is under the 0.8 s
  floor a cabinet can manage. Placement and stop angle are no longer
  abstractions: both are in the units Hammond specifies them in.
- Each pair of microphones has its own distance, spacing and offset, which is
  how Hammond lists them, and can be moved to the cabinet's flanks instead of
  standing in front of it. Out there the width does nothing, because the
  cabinet is what separates the two, and the distance measures out from the
  flank; how wide the cabinet is, which decides where the flank is, is
  provisional. The bass a capsule gains for being close therefore
  belongs to the pair rather than to the output channel: each pair has its own
  corner, set by its own distance.
- The horn's pair, the drum's pair and the woofer each have their own volume
  in decibels, which is the set Hammond documents. What is still an
  approximation is above them, not among them: the drum's microphones are
  weighted by how squarely the port faces each of them with a pair of
  provisional numbers, and the horn's by a tone shelf with four more.
- The woofer's own bass does not enter a rotor. It reaches the four
  microphones along paths that do not turn, mostly the drum's pair and a
  quarter as much the horn's, so raising it makes the cabinet's sweep
  shallower rather than deeper: 13.7 dB of it becomes 5.1 dB with the woofer
  fully open. How much less the horn's pair hears, and where the level sits by
  default, are provisional.
- The two capsules differ in two ways that are not the same kind of thing. A
  directional capsule lifts the bass as it nears a source, at a corner the
  distance sets and by an amount the pattern scales, which is derived and
  bounded rather than chosen - the bound stands in for the roll-off a real
  capsule has underneath it. The presence and the top that tell a dynamic
  capsule from a condenser are provisional, because no capsule is named
  anywhere we can cite.
- The rotors are belted to alternating-current motors, so the supply sets
  every speed they reach: on 50 Hz the cabinet turns at five sixths of its
  rated speeds and reaches them sooner, because the belt ramps at the rate it
  always did over a shorter range. The console's generator does not follow
  that switch and stays at concert pitch, which is a choice and not a
  consequence: a Hammond sold into a 50 Hz market was geared to play at pitch
  there, and modelling a console running flat on the wrong supply would be a
  separate thing to build.
- A stop has to end at the documented angle and take the documented time, and
  a rotor slowing at a fixed rate cannot do both, because the angle it covers
  is whatever its speed makes it. What the engine holds fixed is the time and
  the endpoints; what it bends is the shape of the deceleration, over a range
  that keeps the speed falling and positive. From the fast speed that window
  always contains the angle, so the brake time is exactly the documented one.
  From a slower speed it sometimes does not, and then the stop takes as long
  as reaching the angle needs - up to about two seconds from Chorale - which
  is a departure from a constant rate that the laboratory reports rather than
  hides.
- The vibrato line uses the documented component values with an unwarped
  bilinear transform, so its cutoff is placed by the components rather than
  fitted; the scanner runs at 6.9 Hz and its plate overlap is modelled as a
  linear crossfade. The AO-28 vibrato amplifier that drives and recovers the
  line is not modelled: a make-up gain derived from the circuit stands in for
  it, which is what keeps the chorus position from arriving 11 dB down.
- Contact bounce, matching-transformer, console tube-stage, expression-network
  and rotary constants are provisional.
- The transformer model is an odd nonlinearity and therefore produces no
  genuine second-order difference product. Its asymmetric magnetic behaviour is
  not modelled yet, so the fit uses the third-order product; see
  `docs/CALIBRATION.md`.
- Cabinet reflections and angle-dependent filters are present, but
  their coefficients are not yet fitted to multi-angle cabinet measurements.
- The microphones are placed geometrically, in centimetres, and the delay,
  level, direction and Doppler all come from the length of the line between a
  rotor's mouth and each microphone. The radius each mouth turns at is not
  documented anywhere we can cite, so 18 cm for the horn and 12 cm for the
  drum are defaults for two controls rather than constants, on their own
  model page, because that is what an unmeasured quantity should be. The
  laboratory measures the deviation they produce against the deviation the
  geometry predicts, and the drum lands within a cent of it. The horn does
  not, by about 15 cents at the default placement, because its rotating tone
  shelf modulates phase as well as level; that is a property of the shaping,
  not of the path, and the shaping is still provisional.
- The engine is measured as sample-rate invariant from 32 to 192 kHz. A fully
  engaged console runs about 15.2x real time at 48 kHz and 3.8x at 192 kHz on
  the development machine. What remains is mostly the generator itself, which is
  inherent: 91 wheels turn whether or not anyone is playing.
