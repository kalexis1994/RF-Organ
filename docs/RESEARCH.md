# Research baseline

## Primary documentation

- Hammond Organ Service Manual, early console models including B-3/C-3:
  https://medias.audiofanzine.com/files/hammond-service-manual-a-a-100-ba-bc-bcv-bv-b2-b3-c-cv-c2-c2-text-470145.pdf
- Hammond B-3/C-3 service manual and AO-28 schematic (T1/T2 matching
  transformers, vibrato channels, percussion input and V4A summing node):
  https://device.report/m/1764f5c997292f510b1451dbaa1e3c2ac48ee76b3628eff16a05a580795ece2b
- Hammond XK-7/XK-7D owner's manual (manufacturer description of the AO-28,
  matching/output transformer hysteresis, expression chain and the gently
  sloped 200 Hz tone control with a modern ±9 dB extension, plus the C/F
  two-note difference-product listening test):
  https://hammondorganco.com/wp-content/uploads/2026/03/XK7DXK7-OEM-MANUAL.pdf
- Hammond XK-5 owner's playing guide (manufacturer expression-response model,
  including independently retained low/high-frequency bands):
  https://hammondorganco.com/wp-content/uploads/2022/09/XK-5-Owners-Playing-Guide-Release-3-1.pdf
- Hammond New B-3/B-3 mk2 owner's documentation (used to distinguish later
  digital pedal-sustain behavior from the electromechanical console):
  https://www.hammond.eu/Support/OwnersManuals
- Laurens Hammond, electrical musical instrument, US1956350A:
  https://patents.google.com/patent/US1956350A/en
- Laurens Hammond and John M. Hanert, vibrato apparatus, US2560568A:
  https://patents.google.com/patent/US2560568A/en
- Donald J. Leslie, rotatable tremulant sound producer, US2489653A:
  https://patents.google.com/patent/US2489653A/en

## Measurements and models

- Malte Münster and Florian Pfeifle, "Non-linearities of the
  mechano-electrical tonegenerator of the Hammond organ," ISMA 2019:
  https://pub.dega-akustik.de/ISMA2019/data/articles/000097.pdf
- Giulio Moro, Andrew P. McPherson and Mark B. Sandler, "Dynamic temporal
  behaviour of the keyboard action on the Hammond organ and its perceptual
  significance," JASA 2017: https://doi.org/10.1121/1.5003796
- Kurt James Werner, W. Ross Dunkel and François G. Germain, "A Computational
  Model of the Hammond Organ Vibrato/Chorus using Wave Digital Filters,"
  DAFx-2016: https://dafx16.vutbr.cz/dafxpapers/38-DAFx-16_paper_54-PN.pdf
- Jussi Pekonen, Tapani Pihlajamäki and Vesa Välimäki, "Computationally
  Efficient Hammond Organ Synthesis," DAFx-2011:
  https://www.dafx.de/paper-archive/2011/Papers/49_e.pdf
- Julius O. Smith, Stefania Serafin, Jonathan Abel and David Berners, "Doppler
  Simulation and the Leslie," DAFx-2002:
  https://dafx.de/papers/DAFX02_Smith_Serafin_Abel_Berners_doppler_leslie.pdf
- Richard Kronland-Martinet and Thierry Voinier, "Real-Time Perceptual
  Simulation of Moving Sources: Application to the Leslie Cabinet," 2008:
  https://link.springer.com/article/10.1155/2008/849696

## Open implementations

- setBfree: https://github.com/pantherb/setBfree
- setBfree dynamic contact-envelope branch:
  https://github.com/giuliomoro/setBfree/tree/dynamic-envelopes
- WebHammond: https://github.com/pieter-v-n/WebHammond
- bfreeOrgan2: https://github.com/fcaspe/bfreeOrgan2
- OpenB3: https://github.com/michele-perrone/OpenB3

Implementation provenance belongs in `THIRD_PARTY_NOTICES.md`. A paper or
project appearing here does not imply that its source code is incorporated.

### What they settled about the release

Listing a reference is not the same as saying what was read out of it, and
until the key-off transient was questioned nothing here said. Both of these
were read, and no code was taken from either.

- Upstream setBfree ships `envAttackModel = ENV_CLICK` against
  `envReleaseModel = ENV_LINEAR`: an attack that clicks and a release that
  does not. A clicking release exists and is not the default, and when it is
  chosen its level is half the attack's, `envReleaseClickLevel` 0.25 against
  `envAttackClickLevel` 0.50. The burst itself runs between 8 and 40 samples
  at 22050, which is 0.36 ms to 1.81 ms, and that range is the same for both.
- The `dynamic-envelopes` branch is Giulio Moro's, the first author of the
  JASA keyboard-action measurements above, so it is the closest thing to an
  implementation of them. Its `BouncingEnvelope` is a restitution model --
  coefficient 0.5, bounce frequency 1302 Hz, amplitude from velocity -- and
  it is built only when an oscillator is added. On the release path it sets
  `osp->be = NULL` and hands the oscillator an ordinary `releaseEnv`.

So both treat a key coming up as a different event from a key going down,
and neither bounces on the way out. `RELEASE_SPREAD_S` and
`RELEASE_CLICK_SHARE` in `manual.rs` are where that lands here; the tests
beside them name it.
