# RF-Organ

![RF-Organ instrument](package/branding/banner.png)

RF-Organ is a tonewheel organ for RackForge with upper and lower manuals, a pedalboard and a rotary cabinet. The drawbars set the mixture of harmonics. Scanner vibrato and chorus add motion before the sound reaches the cabinet.

## Install

RF-Organ is included in the RackForge Standard bundle. To install it separately, download `RF-Organ.rfplugin` and choose **Install plugin** in RackForge's **Plugin Manager**. [Download RF-Organ](https://github.com/kalexis1994/RF-Organ/releases/latest)

## Start playing

Open **RF-Organ** in PLAY and pick one of its 21 factory programs.

- **Straight 888** gives you the sound of the first three drawbars with a direct signal.
- **Chorale 888** adds C3 scanner chorus and slow rotary movement.
- **Jazz Trio** gives you soft third-harmonic percussion and a registration for left-hand bass.

Pull the lower-pitched drawbars for body and bring in the higher ones for bite. Use percussion for an accent at the start of a phrase. With the cabinet engaged, switch from **Chorale** to **Tremolo** during a held chord to hear the rotors speed up.

## Connect a controller

| MIDI | Assignment |
| --- | --- |
| Channel 1 | Upper manual |
| Channel 2 | Lower manual |
| Channel 3 | Pedalboard |
| CC11 | Expression pedal |
| CC1 | Chorale below 64 and Tremolo from 64 |
| CC64 | Switch rotary speed with a pedal press or hold it to stop the rotors by default |

Use the on-screen drawbars to shape each manual independently. The **Sustain Pedal** setting changes what CC64 does.

## License

RF-Organ is distributed under GPL-2.0-or-later. [License](LICENSE) · [Notices](THIRD_PARTY_NOTICES.md)
