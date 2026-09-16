# Research baseline

## Primary documentation

- Hammond Organ Service Manual, early console models including B-3/C-3:
  https://medias.audiofanzine.com/files/hammond-service-manual-a-a-100-ba-bc-bcv-bv-b2-b3-c-cv-c2-c2-text-470145.pdf
- Hammond B-3/C-3 service manual and AO-28 schematic (T1/T2 matching
  transformers, vibrato channels, percussion input and V4A summing node):
  https://device.report/m/1764f5c997292f510b1451dbaa1e3c2ac48ee76b3628eff16a05a580795ece2b
- Hammond XK-7/XK-7D owner's manual (manufacturer description of the AO-28,
  matching/output transformer hysteresis, expression and tone-control chain):
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
