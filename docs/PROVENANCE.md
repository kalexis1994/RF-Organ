# Provenance

Every number the engine is built from, what it rests on and the span it
could plausibly take. Generated from `crates/rf-organ-dsp/src/provenance.rs`
by `cargo run -p rf-organ-lab -- provenance docs/PROVENANCE.md`; a test
holds this page to that table, so edit the table and regenerate.

| Basis | Meaning |
|---|---|
| documented | Stated in a manufacturer's document: a service manual, a schematic, an owner's manual or a patent. |
| measured | Measured on real instruments by someone else, and published. |
| derived | Follows by physics or arithmetic from documented or measured figures. |
| borrowed | Taken from another model rather than from an instrument. |
| provisional | Chosen here, pending a measurement. |

A span is *sourced* when a document or a measurement gives it and *assumed*
when nothing published bounds it; an assumed span says where a sensitivity
survey should look, not where the truth is.

## Summary

| Basis | Constants |
|---|---:|
| documented | 16 |
| measured | 4 |
| derived | 5 |
| borrowed | 4 |
| provisional | 73 |
| **total** | **102** |

## `tonewheel`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `SHAFT_RPM_SIXTY` | 1200 | rpm | documented | exact | Service manual: the synchronous run motor turns at 1200 rpm on 60 Hz. |
| `SHAFT_RPM_FIFTY` | 1500 | rpm | documented | exact | Service manual: the 4-pole armature turns at 1500 rpm on 50 Hz. |
| `ECCENTRICITY_DEPTH` | 0.015 | fraction | provisional | 0 to 0.05 (assumed) | Hammond documents a once-per-revolution level change; its depth varies from organ to organ and is not published. |
| `LOW_OCTAVE_THIRD` | 0.12 | fraction | provisional | 0 to 0.35 (assumed) | Low wheels are visibly not sinusoidal (Muenster and Pfeifle, ISMA 2019); the profile is not fitted to any wheel. |
| `SECOND_OCTAVE_THIRD` | 0.045 | fraction | provisional | 0 to 0.15 (assumed) | As the bottom octave's, smaller. |
| `LOW_OCTAVE_FIFTH` | 0.035 | fraction | provisional | 0 to 0.15 (assumed) | As the bottom octave's third. |
| `WOW_RATE_HZ` | 2.7 | Hz | provisional | 0.5 to 6 (assumed) | The service manual describes a resiliently coupled drive; how fast it swings is not published. |
| `WOW_DEPTH` | 0.0006 | fraction | provisional | 0 to 0.002 (assumed) | As the wow rate. |
| `FLUTTER_DEPTH` | 0.0004 | fraction | provisional | 0 to 0.002 (assumed) | As the wow rate. |
| `COUPLING_HZ` | 34 | Hz | provisional | 10 to 80 (assumed) | The service manual describes coil-spring couplings; their resonance is not published. |
| `COUPLING_DAMPING` | 0.6 | ratio | provisional | 0.2 to 1 (assumed) | As the coupling's corner. |

## `lib`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `LEAKAGE_CEILING` | 0.12 | gain | provisional | 0.01 to 0.3 (assumed) | Hammond describes leakage and compartments, not its level. |

## `manual`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `CONTACT_DELAY_CEILING_S` | 0.7256 | s | documented | exact | Hammond's contact page: a contact may be delayed by up to 725.6 ms. |
| `PRESS_SPREAD_BRISK_S` | 0.0004 | s | provisional | 0.0001 to 0.002 (assumed) | The keyboard-action study that measured contact spread (Moro et al., JASA 2017) is behind a paywall. |
| `PRESS_SPREAD_GENTLE_S` | 0.008 | s | provisional | 0.002 to 0.02 (assumed) | As the brisk spread. |
| `BOUNCE_GENTLE_S` | 0.0015 | s | provisional | 0.0005 to 0.005 (assumed) | As the brisk spread. |
| `BOUNCE_HARD_S` | 0.004 | s | provisional | 0.001 to 0.01 (assumed) | As the brisk spread. |
| `BOUNCE_REDRAW_HZ` | 2600 | Hz | borrowed | 1300 to 5200 (assumed) | setBfree's 1302 Hz bounce, written as a half period. |
| `BOUNCE_RESTITUTION` | 0.5 | ratio | borrowed | 0.2 to 0.8 (assumed) | setBfree's dynamic-envelopes branch; that a bounce loses energy is established, by how much is not. |
| `RELEASE_SPREAD_S` | 0.0018 | s | borrowed | 0.00036 to 0.00181 (sourced) | The top of setBfree's click-burst range, which it gives a release and an attack alike. |
| `RELEASE_CLICK_SHARE` | 0.5 | ratio | borrowed | 0.25 to 1 (assumed) | setBfree's release click at half the attack's, transplanted from a level to a rate. |
| `KEY_CLICK_SOFT_S` | 0.01 | s | provisional | 0.002 to 0.02 (assumed) | The XK-7 manual documents both ends of the click control, not how long the gentle one is. |
| `LEAKAGE_PER_KEY` | 0.35 | ratio | provisional | 0 to 1 (assumed) | Hammond documents that leakage grows with the keys held, not by how much. |
| `LEAKAGE_BOOST_DEFAULT` | 0.5 | control | provisional | 0 to 1 (assumed) | Where that growth control starts. |

## `pedal`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `RELEASE_VELOCITY_FLOOR` | 0.25 | velocity | provisional | 0 to 1 (assumed) | A floor for controllers that send no release velocity; the manuals no longer use one. |

## `percussion`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `NORMAL_DRAWBAR_ATTENUATION` | 0.5011872 | gain | documented | exact | Hammond: the vintage console takes about 6 dB out of the upper drawbars at Normal volume. |
| `ATTACK_SECONDS` | 0.0015 | s | provisional | 0.0002 to 0.005 (assumed) | The percussion capacitor charges through the key contact; no figure is published. |
| `RECOVERY_SECONDS` | 0.12 | s | provisional | 0.03 to 0.5 (assumed) | Hammond describes the recovery, not its time. |
| `SOFT_LEVEL` | 0.34 | gain | provisional | 0.1 to 0.6 (assumed) | Hammond names the settings, not the percussion's own level. |
| `NORMAL_LEVEL` | 0.72 | gain | provisional | 0.4 to 1 (assumed) | As the Soft level. |
| `SLOW_T60_S` | 1.25 | s | provisional | 0.8 to 2.5 (assumed) | Hammond publishes no decay in seconds. |
| `FAST_T60_S` | 0.28 | s | provisional | 0.15 to 0.6 (assumed) | As the slow decay. |

## `transformer`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `ASYMMETRY_DEFAULT` | 0.1 | control | provisional | 0 to 0.5 (assumed) | Neither core material nor history is published; puts the second-order product about 11 dB under the third. |
| `MEMORY_HZ` | 900 | Hz | provisional | 100 to 3000 (assumed) | A reduced core; the laboratory fits drive and memory from intermodulation captures. |
| `DRIVE_GAIN` | 5 | gain | provisional | 1 to 10 (assumed) | As the memory corner. |
| `MEMORY_BIAS` | 0.8 | gain | provisional | 0 to 2 (assumed) | As the memory corner. |
| `BEND_FLOOR` | 0.55 | ratio | provisional | 0.3 to 0.9 (assumed) | As the memory corner. |
| `BEND_SPAN` | 0.35 | ratio | provisional | 0 to 0.6 (assumed) | As the memory corner. |

## `vibrato_line`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `INDUCTANCE_H` | 0.5 | H | documented | exact | Component value of the line's series inductors. |
| `TERMINATION_OHM` | 15000 | ohm | documented | exact | Component value of the line's termination. |
| `SOURCE_OHM` | 22000 | ohm | documented | exact | Rc, in circuit for chorus and shorted for vibrato. |
| `CUTOFF_HZ` | 7117.6259 | Hz | derived | exact | One over pi root LC of a uniform section. |
| `IMPEDANCE_OHM` | 11180.339887498949 | ohm | derived | exact | Root L over C of a uniform section; a change of variable. |
| `ROTOR_HZ` | 6.9 | Hz | provisional | 6 to 7.2 (assumed) | Driven from the synchronous motor; DAFx-16 quotes about 6 Hz, and 6.9 is not traced to a document here. |

## `electronics`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `EXPRESSION_SECTION_CAPACITANCE_PF` | 60 | pF | documented | exact | AO-28: 60 pF per expression-control section. |
| `EXPRESSION_GRID_RESISTANCE_OHM` | 15000000 | ohm | documented | exact | AO-28: R34, 15 megohm. |
| `EXPRESSION_LOW_HZ` | 176.83882 | Hz | derived | exact | The corner R34 makes with one section; its role as the low band's corner is this model's reduction. |
| `EXPRESSION_HIGH_HZ` | 4000 | Hz | provisional | 2000 to 8000 (assumed) | The C17/C22/C24 high shoulder is a calibration target. |
| `EXPRESSION_LOW_SHARE` | 0.72 | ratio | provisional | 0 to 1 (assumed) | The transfer between moving and fixed plates is a calibration target. |
| `EXPRESSION_HIGH_SHARE` | 0.48 | ratio | provisional | 0 to 1 (assumed) | As the low band's share. |
| `EXPRESSION_SLEW_S` | 0.008 | s | provisional | 0.002 to 0.05 (assumed) | Stands for the pedal's mechanical travel. |
| `TONE_CONTROL_HZ` | 200 | Hz | documented | exact | XK-7 owner's manual: a gently sloped tone control at 200 Hz. |
| `BASS_SHELF_HZ` | 310 | Hz | provisional | 100 to 600 (assumed) | A modern calibration trim, not a console part. |
| `V4A_ASYMMETRY` | 0.45 | ratio | provisional | 0 to 1 (assumed) | AO-28 component values are not legible in the available scan; the comparator fits the shared character from a bench injection. |
| `V4B_ASYMMETRY` | 0.3 | ratio | provisional | 0 to 1 (assumed) | As V4A's. |
| `V3B_ASYMMETRY` | 0.22 | ratio | provisional | 0 to 1 (assumed) | As V4A's. |
| `V4A_DRIVE_SHARE` | 0.45 | ratio | provisional | 0 to 1 (assumed) | From outside the preamplifier only the chain is visible, not how the stages divide it. |
| `V4B_DRIVE_SHARE` | 0.35 | ratio | provisional | 0 to 1 (assumed) | As V4A's. |
| `V3B_DRIVE_SHARE` | 0.2 | ratio | provisional | 0 to 1 (assumed) | As V4A's. |
| `STAGE_CHARACTER_DEFAULT` | 1 | control | provisional | 0 to 3 (assumed) | Where the shared stage character starts; one means the asymmetries as written. |

## `rotary`

| Constant | Value | Unit | Basis | Span | Rests on |
|---|---:|---|---|---|---|
| `HORN_SLOW_RPM` | 48 | rpm | measured | 46 to 50 (sourced) | A 147 at the middle pulley measures 48 and a 122XB 46; every source puts it between 46 and 50. |
| `HORN_FAST_RPM` | 400 | rpm | measured | 400 to 405 (sourced) | A 147 measures 400 and a 122XB 405. |
| `DRUM_SLOW_RPM` | 40 | rpm | measured | 38 to 40 (sourced) | A 147 measures about 40 and a 122XB 38. |
| `DRUM_FAST_RPM` | 340 | rpm | measured | 340 to 395 (sourced) | A 147 measures about 342 and a 122XB 395. |
| `HORN_RISE_SECONDS` | 1 | s | provisional | 0.8 to 12.5 (sourced) | Inside Hammond's documented floor and ceiling for the horn. |
| `HORN_FALL_SECONDS` | 1.2 | s | provisional | 0.8 to 12.5 (sourced) | As the horn's rise. |
| `HORN_BRAKE_SECONDS` | 1 | s | provisional | 0.8 to 12.5 (sourced) | As the horn's rise. |
| `DRUM_RISE_SECONDS` | 2.6 | s | provisional | 1 to 12.5 (sourced) | Inside Hammond's documented floor and ceiling for the drum. |
| `DRUM_FALL_SECONDS` | 3.6 | s | provisional | 1 to 12.5 (sourced) | As the drum's rise. |
| `DRUM_BRAKE_SECONDS` | 2.4 | s | provisional | 1 to 12.5 (sourced) | As the drum's rise. |
| `HORN_TIME_FLOOR` | 0.8 | s | documented | exact | Hammond's floor for a horn transition. |
| `DRUM_TIME_FLOOR` | 1 | s | documented | exact | Hammond's floor for a drum transition. |
| `TIME_CEILING` | 12.5 | s | documented | exact | Hammond's ceiling for a transition. |
| `ACCELERATION_BRISKEST` | 0.55 | ratio | provisional | 0.3 to 1 (assumed) | How the acceleration control maps onto the times; the middle lands near them. |
| `ACCELERATION_SPAN` | 1.8 | ratio | provisional | 0.5 to 4 (assumed) | As the briskest scale. |
| `MODE_DELAY_SECONDS` | 0.04 | s | provisional | 0 to 1 (sourced) | Hammond exposes the relay and clutch delay as 0 to 1 s. |
| `RATED_MAINS_HZ` | 60 | Hz | documented | exact | The supply the quoted speeds belong to. |
| `CROSSOVER_HZ` | 800 | Hz | provisional | 500 to 1200 (assumed) | The figure commonly quoted for the cabinet's crossover; no Leslie document is cited for it here, and the single-pole slope is this model's reduction. |
| `HORN_SHELF_HZ` | 2200 | Hz | provisional | 1000 to 5000 (assumed) | Stands in for the horn's directivity measured at many angles. |
| `HORN_BODY_AWAY` | 0.55 | gain | provisional | 0.2 to 0.9 (assumed) | As the horn's shelf. |
| `HORN_BODY_FACING` | 0.35 | gain | provisional | 0.1 to 0.8 (assumed) | As the horn's shelf. |
| `HORN_TOP_AWAY` | 0.2 | gain | provisional | 0 to 0.6 (assumed) | As the horn's shelf. |
| `HORN_TOP_FACING` | 0.8 | gain | provisional | 0.4 to 1 (assumed) | As the horn's shelf. |
| `DRUM_AWAY` | 0.66 | gain | provisional | 0.3 to 0.95 (assumed) | Stands in for the drum port's directivity. |
| `DRUM_FACING` | 0.34 | gain | provisional | 0.05 to 0.7 (assumed) | As the drum's level away. |
| `REFLECTION_LEVEL` | 0.65 | gain | provisional | 0 to 1 (assumed) | No cabinet's early reflections have been measured for this model. |
| `SUB_LEVEL_DEFAULT_DB` | -9 | dB | provisional | -76 to 0 (sourced) | Hammond documents the woofer's volume range, not where it sits. |
| `SUB_INTO_DRUM` | 1 | gain | derived | exact | The reference the horn pair's share of the woofer is stated against. |
| `SUB_INTO_HORN` | 0.25 | gain | provisional | 0 to 0.6 (assumed) | Hammond says the horn's pair barely hears the woofer, not how little. |
| `DYNAMIC_PRESENCE_HZ` | 3000 | Hz | provisional | 2000 to 6000 (assumed) | No capsule is named anywhere citable. |
| `DYNAMIC_PRESENCE_LIFT` | 0.4 | gain | provisional | 0 to 1 (assumed) | As the presence corner. |
| `DYNAMIC_TOP_HZ` | 11000 | Hz | provisional | 6000 to 16000 (assumed) | As the presence corner. |
| `PROXIMITY_MAX_LIFT` | 0.8 | gain | provisional | 0.3 to 1.5 (assumed) | Stands in for the roll-off a real capsule has under its proximity lift. |
| `HORN_RADIUS_DEFAULT_M` | 0.18 | m | provisional | 0.05 to 0.4 (assumed) | Not published; a control, and the laboratory compares its Doppler with the geometry's. |
| `DRUM_RADIUS_DEFAULT_M` | 0.12 | m | provisional | 0.05 to 0.3 (assumed) | As the horn's radius. |
| `CABINET_HALF_M` | 0.36 | m | provisional | 0.25 to 0.45 (assumed) | Not on the page the microphone placement is documented on. |
| `SOUND_SPEED_M_PER_S` | 343 | m/s | derived | exact | Speed of sound in air at about 20 degrees Celsius. |
| `MIC_SPACING_MAX_M` | 0.4 | m | documented | exact | Hammond: up to 40 cm between a pair. |
| `MIC_OFFSET_MAX_M` | 0.5 | m | documented | exact | Hammond: up to 50 cm between a pair's centre and the rotor's pivot. |
| `MIC_DISTANCE_DEFAULT_M` | 0.35 | m | provisional | 0.12 to 1.7 (sourced) | A default inside Hammond's documented placement range. |
| `MIC_SPACING_DEFAULT_M` | 0.3 | m | provisional | 0 to 0.4 (sourced) | As the distance's default. |
| `MIC_PATTERN_DEFAULT` | 0.5 | control | provisional | 0 to 1 (assumed) | A cardioid, from omnidirectional at zero to figure of eight at one. |

## Tables

| Table | Basis | Rests on |
|---|---|---|
| `tonewheel::GEAR_RATIOS_60_HZ` | documented | The generator's gear train, translated from setBfree's 60 Hz table. |
| `taper::TAPER` | provisional | Every wheel at unity until a generator is measured. |
| `preset::UPPER_PRESETS` | documented | The upper preset panel's registrations, quoted digit for digit. |
| `preset::LOWER_PRESETS` | documented | The lower preset panel's registrations, quoted digit for digit. |
| `vibrato_line::CAPACITANCE_F` | documented | Component values of the line's shunt capacitors. |
| `vibrato_line::DIVIDERS` | documented | The six tap dividers' resistances. |
| `vibrato_line::DEPTH_TERMINAL_NODES` | documented | Which ladder nodes each depth position wires to the terminals. |
| `rotary::REFLECTIONS` | provisional | Three early reflections' levels and delays; none measured. |

## Not part of the model

Scalars the engine declares that describe no instrument.

| Constant | Why |
|---|---|
| `lib::SAMPLE_RATE_MIN` | the host contract, measured as rate invariant |
| `lib::SAMPLE_RATE_MAX` | the host contract, measured as rate invariant |
| `rotary::RESTING_HZ` | below this a rotor is treated as stopped; a numerical floor |
| `rotary::NEAREST_SQUARED_M2` | keeps a path length away from zero; never reached |
| `rotary::LEVEL_SILENT_DB` | the floor a volume reads as silence at; a display convention |
| `transformer::LEAN_CEILING` | keeps a leaning core's weak half bending at all; a guard on the reduction |
