// SPDX-License-Identifier: GPL-2.0-or-later
use crate::preset::PRESET_COUNT;
use crate::tonewheel::TONEWHEEL_COUNT;

pub const DRAWBAR_COUNT: usize = 9;
/// Adjustable drawbar groups per manual. The service manual's own count:
/// "9 PRESET KEYS AND 2 SETS OF 9 ADJUSTABLE HARMONIC DRAWBARS FOR EACH
/// MANUAL".
pub const DRAWBAR_SET_COUNT: usize = 2;

/// Which of the twelve reverse-colour keys at the left of a manual is down.
///
/// The service manual makes this a circuit rather than a preference: "As
/// there are no wires connected to these busbars, a preset or adjust key must
/// be depressed before any circuit can be completed." The key at the extreme
/// left is the cancel key, "with no contacts, which releases any preset or
/// adjust key that happens to be depressed" - so cancel is silence, not a
/// neutral position.
///
/// The two at the extreme right are the adjust keys: "The adjust keys, A# and
/// B, are connected by flexible wires ... to the corresponding nine
/// drawbars", and "In each case the A# adjust key controls the left hand
/// group of drawbars for that manual."
///
/// The nine between them, C# to A, are wired to the preset panel, where each
/// harmonic is screwed to one of nine bars and "this is equivalent to setting
/// a harmonic drawbar to the corresponding number". They are therefore nine
/// more registrations and nothing else, and [`crate::preset`] holds them.
///
/// The numbering below is the order these were modelled in and not the order
/// they sit on the keyboard, so that the three that existed before the panel
/// did keep the values already recorded against them. [`Self::KEYBOARD`] is
/// the left-to-right order, which is what a player sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Registration {
    Cancel = 0,
    AdjustA = 1,
    AdjustB = 2,
    PresetCSharp = 3,
    PresetD = 4,
    PresetDSharp = 5,
    PresetE = 6,
    PresetF = 7,
    PresetFSharp = 8,
    PresetG = 9,
    PresetGSharp = 10,
    PresetA = 11,
}

impl Registration {
    /// The twelve keys in the order they are laid out, from the cancel key at
    /// the extreme left to the B adjust key at the extreme right.
    pub const KEYBOARD: [Self; 12] = [
        Self::Cancel,
        Self::PresetCSharp,
        Self::PresetD,
        Self::PresetDSharp,
        Self::PresetE,
        Self::PresetF,
        Self::PresetFSharp,
        Self::PresetG,
        Self::PresetGSharp,
        Self::PresetA,
        Self::AdjustA,
        Self::AdjustB,
    ];

    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Cancel),
            1 => Some(Self::AdjustA),
            2 => Some(Self::AdjustB),
            3 => Some(Self::PresetCSharp),
            4 => Some(Self::PresetD),
            5 => Some(Self::PresetDSharp),
            6 => Some(Self::PresetE),
            7 => Some(Self::PresetF),
            8 => Some(Self::PresetFSharp),
            9 => Some(Self::PresetG),
            10 => Some(Self::PresetGSharp),
            11 => Some(Self::PresetA),
            _ => None,
        }
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    /// The key's own name, which is the note it sits on.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::AdjustA => "adjust-a-sharp",
            Self::AdjustB => "adjust-b",
            Self::PresetCSharp => "preset-c-sharp",
            Self::PresetD => "preset-d",
            Self::PresetDSharp => "preset-d-sharp",
            Self::PresetE => "preset-e",
            Self::PresetF => "preset-f",
            Self::PresetFSharp => "preset-f-sharp",
            Self::PresetG => "preset-g",
            Self::PresetGSharp => "preset-g-sharp",
            Self::PresetA => "preset-a",
        }
    }

    /// Which group of drawbars this key is wired to, if it is wired to one.
    /// The cancel key has no contacts at all, and a preset key is wired to
    /// the panel rather than to a drawbar.
    pub const fn drawbars(self) -> Option<usize> {
        match self {
            Self::AdjustA => Some(0),
            Self::AdjustB => Some(1),
            _ => None,
        }
    }

    /// Which of the panel's nine registrations this key carries, if it
    /// carries one.
    pub const fn preset(self) -> Option<usize> {
        match self {
            Self::PresetCSharp => Some(0),
            Self::PresetD => Some(1),
            Self::PresetDSharp => Some(2),
            Self::PresetE => Some(3),
            Self::PresetF => Some(4),
            Self::PresetFSharp => Some(5),
            Self::PresetG => Some(6),
            Self::PresetGSharp => Some(7),
            Self::PresetA => Some(8),
            _ => None,
        }
    }
}

/// How much louder the leakage gets for each extra key held, at the top of
/// the rate control. Provisional: Hammond publishes that the rate exists and
/// is adjustable, not what it is.
const LEAKAGE_PER_KEY: f32 = 0.35;

/// How far apart a key's contacts may be asked to arrive, beyond the spread a
/// press of its own gives them.
///
/// Hammond's own contact page carries this as a control with a published
/// range: a virtual contact may be delayed by up to 725.6 ms after the
/// physical one is made. That is three orders of magnitude past what a briskly
/// played key spreads, and it is the range the console needs in order to do
/// what the same manual describes elsewhere - a key "pressed very slowly"
/// losing its percussion, which at a decay of about a second takes hundreds of
/// milliseconds between the first contact and the bus the tone is heard
/// through.
///
/// It is a separate control here because it is a separate control there. The
/// spread above is what a press does; this is what the instrument is set to
/// do on top of it, and at zero it does nothing at all.
pub const CONTACT_DELAY_CEILING_S: f32 = 0.7256;

/// How long a contact takes to make when it is asked to make gently.
///
/// Hammond's key click control says what the two ends of it are: at zero "the
/// note will sound with no 'click' at the onset of the sound as with a
/// traditional electronic instrument", and "a higher value will create a
/// faster attack as well as introduce Key Click". The click and the attack are
/// the same thing, which is a contact either arriving abruptly or not.
///
/// A console is at the abrupt end and that is where this starts, so nothing
/// already recorded moves; the other end is ten milliseconds, which is well
/// past the point where a step stops being heard as a click and is
/// provisional.
pub(crate) const KEY_CLICK_SOFT_S: f32 = 0.010;
/// How far apart a key's contacts break when it is let go.
///
/// This was the press's own formula run backwards, with a floor of 0.25 on
/// the release velocity because almost no controller sends one. The floor
/// decided everything: it put every release, however the key was actually
/// lifted, at 6.4 ms of stagger, against 0.4 ms for a briskly played press.
/// Sixteen times wider, and not from any measurement -- from a default.
///
/// setBfree's click burst covers the whole transient and runs between
/// 0.36 ms and 1.81 ms, the same range for a release as for an attack; what
/// it varies between the two is the level, not the length. This takes the
/// top of that range, so the shipped spread of 0.55 gives a shade under a
/// millisecond, and the control still reaches the reference's ceiling at
/// its own.
pub(crate) const RELEASE_SPREAD_S: f32 = 0.0018;
/// How much of the attack's arrival a release gets.
///
/// setBfree gives the release a click of its own and sets it at half the
/// attack's: `envReleaseClickLevel` 0.25 against `envAttackClickLevel` 0.50.
/// Its click is a depth of modulation and this one is a rate of arrival, so
/// the ratio is transplanted rather than the quantity -- a release arrives
/// at half the speed the same key would arrive at going down, which is the
/// nearest thing this model has to the same sentence.
pub(crate) const RELEASE_CLICK_SHARE: f32 = 0.5;

/// Where that rate control starts, which is also provisional.
pub const LEAKAGE_BOOST_DEFAULT: f32 = 0.5;
pub const MANUAL_FIRST_NOTE: u8 = 36;
pub const MANUAL_KEY_COUNT: usize = 61;
const MANUAL_LAST_NOTE: u8 = MANUAL_FIRST_NOTE + MANUAL_KEY_COUNT as u8 - 1;
const OFFSETS: [i16; DRAWBAR_COUNT] = [-12, 7, 0, 12, 19, 24, 28, 31, 36];
pub const DRAWBAR_LEVELS: [f32; 9] = [0.0, 0.089, 0.126, 0.178, 0.251, 0.355, 0.501, 0.708, 1.0];

/// B-3-style manual wiring. The returned index addresses the shared 91-wheel
/// bank; unavailable pitches fold back by octaves.
pub fn drawbar_wheel(key: usize, bus: usize) -> Option<usize> {
    if key >= MANUAL_KEY_COUNT || bus >= DRAWBAR_COUNT {
        return None;
    }
    let mut terminal = key as i16 + 13 + OFFSETS[bus];
    while terminal < 13 {
        terminal += 12;
    }
    while terminal > 91 {
        terminal -= 12;
    }
    Some((terminal - 1) as usize)
}

#[derive(Clone, Copy)]
struct Contact {
    wheel: u8,
    gate: f32,
    /// How much of the way the gate travels per sample once the contact is
    /// making. One is the abrupt arrival a console gives and anything less is
    /// the same contact closing more gently.
    rise: f32,
    target: bool,
    delay: u32,
    bounce_left: u32,
    period: u16,
    period_left: u16,
    noise: u32,
}

impl Contact {
    const EMPTY: Self = Self {
        wheel: 0,
        gate: 0.0,
        rise: 1.0,
        target: false,
        delay: 0,
        bounce_left: 0,
        period: 1,
        period_left: 0,
        noise: 1,
    };

    fn schedule(
        &mut self,
        target: bool,
        delay: u32,
        bounce: u32,
        period: u16,
        seed: u32,
        rise: f32,
    ) {
        self.rise = rise;
        self.target = target;
        self.delay = delay;
        self.bounce_left = bounce;
        self.period = period.max(1);
        self.period_left = 0;
        self.noise = seed.max(1);
    }

    /// A contact with nothing left to do: no delay to count down, no bounce
    /// left, and its gate already where the key put it. Ticking it again
    /// cannot change anything, which is what lets the scan skip whole keys.
    fn settled(&self) -> bool {
        self.delay == 0 && self.bounce_left == 0 && self.gate == if self.target { 1.0 } else { 0.0 }
    }

    fn tick(&mut self) -> bool {
        if self.delay > 0 {
            self.delay -= 1;
            return false;
        }
        let before = self.gate;
        if self.bounce_left == 0 {
            // The click is the arrival, so how fast it arrives is the only
            // thing there is to turn down.
            let target = if self.target { 1.0 } else { 0.0 };
            self.gate = if self.rise >= 1.0 {
                target
            } else if target > self.gate {
                (self.gate + self.rise).min(target)
            } else {
                (self.gate - self.rise).max(target)
            };
        } else {
            self.bounce_left -= 1;
            if self.period_left == 0 {
                self.noise ^= self.noise << 13;
                self.noise ^= self.noise >> 17;
                self.noise ^= self.noise << 5;
                let settled = self.noise & 3 == 0;
                let closed = if settled { self.target } else { !self.target };
                self.gate = if closed { 1.0 } else { 0.0 };
                self.period_left = self.period;
            } else {
                self.period_left -= 1;
            }
            if self.bounce_left == 0 {
                self.gate = if self.target { 1.0 } else { 0.0 };
            }
        }
        self.gate != before
    }
}

#[derive(Clone, Copy)]
struct Key {
    active: bool,
    /// Whether this key has already touched a contact during the press it is
    /// in, so that the percussion is discharged once per key and not once per
    /// contact.
    touched: bool,
    /// Whether any of this key's contacts still has work to do. A console has
    /// 549 manual contacts and almost all of them are idle at any moment.
    settling: bool,
    contacts: [Contact; DRAWBAR_COUNT],
}

impl Key {
    const EMPTY: Self = Self {
        active: false,
        touched: false,
        settling: false,
        contacts: [Contact::EMPTY; DRAWBAR_COUNT],
    };

    /// Whether this key can contribute anything to the output: it is either
    /// held, or still on its way to or from being held.
    const fn sounding(&self) -> bool {
        self.active || self.settling
    }
}

pub struct Manual {
    keys: [Key; MANUAL_KEY_COUNT],
    drawbars: [[u8; DRAWBAR_COUNT]; DRAWBAR_SET_COUNT],
    /// Which of the reverse-colour keys is locked down. Only one can be:
    /// "These keys have a locking and trip mechanism which allows only one
    /// key to be in operation at one time."
    registration: Registration,
    /// This manual's half of the preset panel. The panel is "divided into two
    /// sets of nine bars, each connected to a separate matching transformer",
    /// so the swell and the great do not share one.
    presets: &'static [[u8; DRAWBAR_COUNT]; PRESET_COUNT],
    /// What the busbars actually reach through the key that is down, which is
    /// nothing at all under the cancel key.
    levels: [f32; DRAWBAR_COUNT],
    wheel_gains: [f32; TONEWHEEL_COUNT],
    sample_rate: f32,
    contact_spread: f32,
    contact_bounce: f32,
    event_counter: u32,
    /// Keys down right now, which is what the leakage grows with.
    held: u8,
    contact_delay: f32,
    key_click: f32,
    /// Set for the one sample in which some key's first contact touched its
    /// busbar. The percussion supply is discharged by a contact, not by a
    /// decision, so this is what it waits for.
    took_first_contact: bool,
    leakage_boost: f32,
}

impl Manual {
    pub fn new(sample_rate: f32, presets: &'static [[u8; DRAWBAR_COUNT]; PRESET_COUNT]) -> Self {
        let mut manual = Self {
            keys: [Key::EMPTY; MANUAL_KEY_COUNT],
            drawbars: [[8, 8, 8, 0, 0, 0, 0, 0, 0]; DRAWBAR_SET_COUNT],
            // The B key, because that is the one the percussion needs and so
            // the one a B-3 is played on.
            registration: Registration::AdjustB,
            presets,
            levels: [0.0; DRAWBAR_COUNT],
            wheel_gains: [0.0; TONEWHEEL_COUNT],
            sample_rate,
            contact_spread: 0.55,
            contact_bounce: 0.45,
            contact_delay: 0.0,
            key_click: 1.0,
            event_counter: 0,
            held: 0,
            took_first_contact: false,
            leakage_boost: LEAKAGE_BOOST_DEFAULT,
        };
        for key in 0..MANUAL_KEY_COUNT {
            for bus in 0..DRAWBAR_COUNT {
                manual.keys[key].contacts[bus].wheel =
                    drawbar_wheel(key, bus).expect("bounded contact") as u8;
            }
        }
        manual.rebuild_levels();
        manual
    }

    /// Moves one drawbar of one adjust key's group. The cancel key has no
    /// drawbars to move, so naming it is an error rather than a silent no-op.
    pub fn set_drawbar(&mut self, key: Registration, index: usize, position: u8) -> bool {
        let Some(set) = key.drawbars() else {
            return false;
        };
        if index >= DRAWBAR_COUNT || position > 8 {
            return false;
        }
        self.drawbars[set][index] = position;
        self.rebuild_levels();
        true
    }

    pub fn drawbar(&self, key: Registration, index: usize) -> Option<u8> {
        self.drawbars.get(key.drawbars()?)?.get(index).copied()
    }

    /// Presses one of the reverse-colour keys, which releases whichever was
    /// down. A held note changes with it: the key is in series with the
    /// busbars, not in front of them.
    pub fn set_registration(&mut self, key: Registration) {
        self.registration = key;
        self.rebuild_levels();
    }

    pub const fn registration(&self) -> Registration {
        self.registration
    }

    pub fn set_contact_spread(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.contact_spread = value;
        true
    }

    pub fn set_contact_bounce(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.contact_bounce = value;
        true
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) -> bool {
        let Some(key) = key_index(note) else {
            return false;
        };
        if !unit(velocity) || velocity == 0.0 {
            return false;
        }
        if self.keys[key].active {
            return true;
        }
        self.keys[key].active = true;
        self.held = self.held.saturating_add(1);
        self.schedule_key(key, true, velocity);
        true
    }

    pub fn note_off(&mut self, note: u8, velocity: f32) -> bool {
        let Some(key) = key_index(note) else {
            return false;
        };
        if !unit(velocity) {
            return false;
        }
        if !self.keys[key].active {
            return true;
        }
        self.keys[key].active = false;
        self.held = self.held.saturating_sub(1);
        // A release reads nothing off this any more: its window is its own
        // and it does not bounce, so there is nothing for a velocity to
        // scale. The floor of 0.25 that used to sit here was the whole
        // reason every release staggered over 6.4 ms.
        self.schedule_key(key, false, velocity);
        true
    }

    fn schedule_key(&mut self, key: usize, target: bool, velocity: f32) {
        self.event_counter = self.event_counter.wrapping_add(1);
        self.keys[key].settling = true;
        // What the press itself spreads, and what the instrument is set to add
        // on top. Hammond's delay is per contact and not scaled by how the key
        // was played; this keeps that, and spends it across the same order the
        // contacts already close in.
        //
        // A key coming up is not a key going down played backwards, and until
        // now this treated it as one. See `RELEASE_SPREAD_S`.
        let spread_seconds = if target {
            self.contact_spread * (0.0004 + 0.008 * (1.0 - velocity))
        } else {
            self.contact_spread * RELEASE_SPREAD_S
        } + self.contact_delay * CONTACT_DELAY_CEILING_S;
        let spread_samples = (spread_seconds * self.sample_rate) as u32;
        // Bounce is the contact arriving and rebounding off the busbar. A
        // contact that is leaving has nothing to rebound from.
        let bounce_samples = if target {
            (self.contact_bounce * (0.0015 + 0.004 * velocity) * self.sample_rate) as u32
        } else {
            0
        };
        let period = (self.sample_rate / 2_600.0).max(1.0) as u16;
        // A console's contacts arrive abruptly. Turning the click down is
        // turning that arrival into a ramp, which is the same control Hammond
        // gives and the same sentence: the attack slows as the click goes.
        // How much of the arrival lands in one sample is the click, so that
        // is what the control sets. It is cubed because the ear is not linear
        // about it: a ramp of two milliseconds already has no click left in
        // it, so a control that spent half its travel getting there would be
        // a switch with a long handle.
        let softest = 1.0 / (KEY_CLICK_SOFT_S * self.sample_rate).max(1.0);
        let click = self.key_click * self.key_click * self.key_click;
        // A release arrives at a share of the speed a press does. See
        // `RELEASE_CLICK_SHARE`.
        let click = if target {
            click
        } else {
            click * RELEASE_CLICK_SHARE
        };
        let rise = click.max(softest).min(1.0);
        for bus in 0..DRAWBAR_COUNT {
            // On a slow physical press the high contacts become audible first.
            let order = DRAWBAR_COUNT - 1 - bus;
            let nominal = spread_samples * order as u32 / (DRAWBAR_COUNT as u32 - 1);
            let seed = hash(self.event_counter ^ ((key as u32) << 8) ^ bus as u32);
            let jitter = if spread_samples == 0 {
                0
            } else {
                seed % (spread_samples / 8 + 1)
            };
            self.keys[key].contacts[bus].schedule(
                target,
                nominal.saturating_add(jitter),
                bounce_samples,
                period,
                seed,
                rise,
            );
        }
    }

    /// Whether a key's first contact touched during the last tick, and
    /// clears the report.
    pub fn took_first_contact(&mut self) -> bool {
        let took = self.took_first_contact;
        self.took_first_contact = false;
        took
    }

    pub fn tick_contacts(&mut self) {
        let mut touched = false;
        for key in &mut self.keys {
            if !key.settling {
                continue;
            }
            let mut settling = false;
            for (bus, contact) in key.contacts.iter_mut().enumerate() {
                let before = contact.gate;
                if contact.tick() {
                    let level = self.levels[bus];
                    self.wheel_gains[contact.wheel as usize] += (contact.gate - before) * level;
                }
                // The first of the nine to touch anything, whichever it is.
                touched |= before == 0.0 && contact.gate > 0.0 && !key.touched;
                settling |= !contact.settled();
            }
            if touched {
                key.touched = true;
            }
            if !key.active && key.contacts.iter().all(|contact| contact.gate == 0.0) {
                key.touched = false;
            }
            key.settling = settling;
        }
        self.took_first_contact = touched;
    }

    /// How fast the leakage grows as more keys go down.
    ///
    /// Hammond gives this as a control of its own, over "the rate at which the
    /// Leakage Tone increases as more notes are played simultaneously". The
    /// reason there is a rate to set is that the leakage arriving at a busbar
    /// comes from the whole generator through the harness, and every contact
    /// that closes gives it another way in - so it grows with the keys held
    /// while the note each key asked for only grows with that key. What is
    /// documented is that it grows and that the rate is adjustable; how
    /// steeply is not, so the step per key is provisional.
    /// How much of the documented contact delay to add to every press, from
    /// none at zero to the published 725.6 ms at one.
    /// How abruptly a contact arrives, from a ramp with no click at zero to
    /// the arrival a console gives at one.
    pub fn set_key_click(&mut self, amount: f32) -> bool {
        if !unit(amount) {
            return false;
        }
        self.key_click = amount;
        true
    }

    pub fn set_contact_delay(&mut self, amount: f32) -> bool {
        if !unit(amount) {
            return false;
        }
        self.contact_delay = amount;
        true
    }

    pub fn set_leakage_boost(&mut self, rate: f32) -> bool {
        if !unit(rate) {
            return false;
        }
        self.leakage_boost = rate;
        true
    }

    /// What the leakage is multiplied by with this many keys down.
    fn leakage_gain(&self) -> f32 {
        1.0 + self.leakage_boost * LEAKAGE_PER_KEY * f32::from(self.held.saturating_sub(1))
    }

    pub fn sample(
        &self,
        wheels: &[f32; TONEWHEEL_COUNT],
        leakage: f32,
        suppress_ninth_drawbar: bool,
    ) -> f32 {
        let leakage = leakage * self.leakage_gain();
        let mut output = 0.0;
        for (index, gain) in self.wheel_gains.iter().copied().enumerate() {
            if gain == 0.0 {
                continue;
            }
            output += gain * (wheels[index] + leakage * compartment_leak(wheels, index));
        }
        if suppress_ninth_drawbar {
            let level = self.levels[8];
            for key in self.keys.iter().filter(|key| key.sounding()) {
                let contact = key.contacts[8];
                let index = contact.wheel as usize;
                let leak = compartment_leak(wheels, index);
                output -= contact.gate * level * (wheels[index] + leakage * leak);
            }
        }
        output * 0.055
    }

    pub fn harmonic_sample(&self, wheels: &[f32; TONEWHEEL_COUNT], bus: usize) -> f32 {
        debug_assert!(bus < DRAWBAR_COUNT);
        let mut output = 0.0;
        for key in self.keys.iter().filter(|key| key.sounding()) {
            let contact = key.contacts[bus];
            output += contact.gate * wheels[contact.wheel as usize];
        }
        output * 0.055
    }

    pub fn is_active(&self, note: u8) -> Option<bool> {
        key_index(note).map(|key| self.keys[key].active)
    }

    pub fn reset(&mut self) {
        self.wheel_gains.fill(0.0);
        for key in &mut self.keys {
            key.active = false;
            key.settling = false;
            for contact in &mut key.contacts {
                contact.gate = 0.0;
                contact.target = false;
                contact.delay = 0;
                contact.bounce_left = 0;
            }
        }
    }

    /// What the key that is down connects the busbars to, and then the gains
    /// that follow from it.
    fn rebuild_levels(&mut self) {
        let positions = match (self.registration.drawbars(), self.registration.preset()) {
            (Some(set), _) => Some(&self.drawbars[set]),
            (_, Some(preset)) => Some(&self.presets[preset]),
            // The cancel key, which has no contacts and so no circuit.
            _ => None,
        };
        match positions {
            Some(positions) => {
                for bus in 0..DRAWBAR_COUNT {
                    self.levels[bus] = DRAWBAR_LEVELS[positions[bus] as usize];
                }
            }
            None => self.levels.fill(0.0),
        }
        self.rebuild_gains();
    }

    fn rebuild_gains(&mut self) {
        self.wheel_gains.fill(0.0);
        for key in &self.keys {
            for (bus, contact) in key.contacts.iter().enumerate() {
                self.wheel_gains[contact.wheel as usize] += contact.gate * self.levels[bus];
            }
        }
    }
}

/// Sum of what the other wheels in a compartment put into this one.
fn compartment_leak(wheels: &[f32; TONEWHEEL_COUNT], index: usize) -> f32 {
    let mut leak = 0.0;
    for companion in COMPARTMENTS[index].iter().flatten() {
        leak += wheels[usize::from(*companion)];
    }
    leak
}

fn key_index(note: u8) -> Option<usize> {
    (MANUAL_FIRST_NOTE..=MANUAL_LAST_NOTE)
        .contains(&note)
        .then(|| usize::from(note - MANUAL_FIRST_NOTE))
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

/// The generator is divided into compartments of four wheels, magnetically
/// shielded from the rest. The service manual lays them out by tooth count:
/// one compartment of a speed holds 2, 32, 8 and 128, the other holds 4, 64,
/// 16 and 192, and where a speed has no 192-tooth wheel that position is
/// marked blank. Wheels of one speed share a driving gear, so a wheel's
/// companions are the rest of its own compartment.
///
/// The table is built at compile time. The hot path looks a wheel up on every
/// sample it is keyed, and working the topology out there cost the engine a
/// third of its headroom.
static COMPARTMENTS: [[Option<u8>; 3]; TONEWHEEL_COUNT] = build_compartments();

pub fn compartment_companions(index: usize) -> [Option<usize>; 3] {
    match COMPARTMENTS.get(index) {
        Some(companions) => [
            companions[0].map(usize::from),
            companions[1].map(usize::from),
            companions[2].map(usize::from),
        ],
        None => [None; 3],
    }
}

const fn build_compartments() -> [[Option<u8>; 3]; TONEWHEEL_COUNT] {
    let mut table = [[None; 3]; TONEWHEEL_COUNT];
    let mut index = 0;
    while index < TONEWHEEL_COUNT {
        table[index] = companions_of(index);
        index += 1;
    }
    table
}

const fn companions_of(index: usize) -> [Option<u8>; 3] {
    let speed = match wheel_speed(index) {
        Some(speed) => speed,
        None => return [None; 3],
    };
    // Even octaves are the 2-32-8-128 compartment; odd octaves and the
    // 192-tooth wheel are the 4-64-16-192 one.
    let even = index < 84 && (index / 12).is_multiple_of(2);
    let mut companions = [None; 3];
    let mut found = 0;
    let mut octave = 0;
    while octave < 7 {
        let candidate = 12 * octave + speed;
        if octave.is_multiple_of(2) == even && candidate < 84 && candidate != index {
            companions[found] = Some(candidate as u8);
            found += 1;
        }
        octave += 1;
    }
    if !even && speed >= 5 {
        let top = 84 + speed - 5;
        if top != index && found < 3 {
            companions[found] = Some(top as u8);
        }
    }
    companions
}

/// Index of the driving gear a wheel runs from. Wheels sharing one are the
/// ones that can share a compartment.
const fn wheel_speed(index: usize) -> Option<usize> {
    if index >= TONEWHEEL_COUNT {
        None
    } else if index >= 84 {
        Some(index % 12 + 5)
    } else {
        Some(index % 12)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// Walks one key's contacts and reports, for the press or the release,
    /// how many samples passed before the last of them stopped moving and
    /// whether any of them ever reversed on its way there.
    fn settle(manual: &mut Manual, key: usize) -> (usize, bool) {
        let mut last_movement = 0;
        let mut reversed = false;
        let mut previous = [0.0_f32; DRAWBAR_COUNT];
        for (slot, contact) in manual.keys[key].contacts.iter().enumerate() {
            previous[slot] = contact.gate;
        }
        let mut direction = [0.0_f32; DRAWBAR_COUNT];
        for sample in 0..9_600 {
            manual.tick_contacts();
            for (slot, contact) in manual.keys[key].contacts.iter().enumerate() {
                let step = contact.gate - previous[slot];
                if step != 0.0 {
                    last_movement = sample;
                    if direction[slot] != 0.0
                        && step.is_sign_negative() != direction[slot].is_sign_negative()
                    {
                        reversed = true;
                    }
                    direction[slot] = step;
                }
                previous[slot] = contact.gate;
            }
        }
        (last_movement, reversed)
    }

    /// A contact that is leaving has nothing to rebound from.
    ///
    /// Bounce is the moving contact striking the busbar and coming back off
    /// it, which is an arrival. Until this was separated, `note_off` ran the
    /// press's whole schedule backwards and gave every release the same
    /// burst of chatter -- nine contacts each slamming between open and
    /// closed on the way out.
    ///
    /// Neither open implementation the project reads does that.
    /// `giuliomoro/setBfree`, the branch written by the author of the JASA
    /// keyboard-action measurements, drives its bouncing envelope from the
    /// attack alone and sets the pointer to null on the release path.
    /// Upstream setBfree ships `envReleaseModel = ENV_LINEAR`: no release
    /// click at all, with a clicking release available and not chosen.
    #[test]
    fn a_key_coming_up_does_not_chatter() {
        let mut manual = Manual::new(48_000.0, &crate::preset::UPPER_PRESETS);
        let key = usize::from(60 - MANUAL_FIRST_NOTE);

        assert!(manual.note_on(60, 1.0));
        let (_, pressing_reversed) = settle(&mut manual, key);
        assert!(
            pressing_reversed,
            "the press should still bounce; that is the click"
        );

        assert!(manual.note_off(60, 1.0));
        let (_, releasing_reversed) = settle(&mut manual, key);
        assert!(!releasing_reversed, "a release must not chatter");
    }

    /// The release breaks its contacts over its own window, not the press's
    /// run backwards.
    ///
    /// What it used to do was run the press's formula with a release
    /// velocity, floored at 0.25 because almost no controller sends one.
    /// The floor decided it: every release, however the key was lifted, got
    /// 6.4 ms of stagger against 0.4 ms for a brisk press. Sixteen times
    /// wider, out of a default rather than a measurement.
    ///
    /// setBfree's click burst covers the whole transient and runs between
    /// 0.36 ms and 1.81 ms, the same range for a release as for an attack.
    /// At the shipped spread this lands a shade under a millisecond, inside
    /// that window, and the bound here is the window's own top.
    #[test]
    fn a_release_breaks_inside_the_window_the_references_use() {
        let mut manual = Manual::new(48_000.0, &crate::preset::UPPER_PRESETS);
        let key = usize::from(60 - MANUAL_FIRST_NOTE);

        assert!(manual.note_on(60, 1.0));
        let _ = settle(&mut manual, key);
        // Zero, which is what almost every controller sends and what used
        // to hit the floor of 0.25 that decided the whole window.
        assert!(manual.note_off(60, 0.0));
        let (last_movement, _) = settle(&mut manual, key);

        let milliseconds = last_movement as f32 * 1000.0 / 48_000.0;
        assert!(
            milliseconds <= 1.81,
            "the release took {milliseconds} ms, past setBfree's longest burst"
        );
        assert!(
            milliseconds > 0.0,
            "the release became instantaneous, which is not the repair"
        );

        // And it no longer follows the press: a key pressed at the very
        // velocity the release used to claim -- the floor of 0.25 -- still
        // staggers several times wider than the release does.
        let mut slow = Manual::new(48_000.0, &crate::preset::UPPER_PRESETS);
        assert!(slow.note_on(60, 0.25));
        let (pressed_slowly, _) = settle(&mut slow, key);
        assert!(
            pressed_slowly as f32 > last_movement as f32 * 3.0,
            "press {pressed_slowly} against release {last_movement}: the two are still tied"
        );
    }

    /// A release arrives at a share of the speed a press does.
    ///
    /// setBfree gives the release a click of its own at half the attack's:
    /// `envReleaseClickLevel` 0.25 against `envAttackClickLevel` 0.50. Its
    /// click is a depth of modulation and this one is a rate of arrival, so
    /// what carries over is the ratio, not the quantity.
    #[test]
    fn a_release_arrives_at_a_share_of_the_speed_a_press_does() {
        let mut manual = Manual::new(48_000.0, &crate::preset::UPPER_PRESETS);
        let key = usize::from(60 - MANUAL_FIRST_NOTE);

        assert!(manual.note_on(60, 1.0));
        let pressing = manual.keys[key].contacts[0].rise;
        let _ = settle(&mut manual, key);
        assert!(manual.note_off(60, 1.0));
        let releasing = manual.keys[key].contacts[0].rise;

        // Spelled out rather than taken from the constant: a test that
        // reads the number it is checking cannot fail when it moves.
        assert!(
            (releasing - pressing * 0.5).abs() <= 1.0e-6,
            "press arrives at {pressing}, release at {releasing}, which is not half"
        );
    }

    /// All-notes-off is not a key coming up, and does not sound like one.
    ///
    /// `Manual::reset` clears the gates and the contact state by hand. That
    /// is right for a panic message, which is not a gesture anybody made,
    /// but it means the two ways a note can stop do not sound alike: this
    /// one has no transient at all. Stated here so that it stays a decision
    /// rather than becoming a surprise.
    #[test]
    fn all_notes_off_cuts_without_touching_a_contact() {
        let mut manual = Manual::new(48_000.0, &crate::preset::UPPER_PRESETS);
        let key = usize::from(60 - MANUAL_FIRST_NOTE);

        assert!(manual.note_on(60, 1.0));
        let _ = settle(&mut manual, key);
        manual.reset();

        assert!(!manual.keys[key].settling);
        assert!(
            manual.keys[key]
                .contacts
                .iter()
                .all(|contact| contact.gate == 0.0),
        );
        let (last_movement, _) = settle(&mut manual, key);
        assert_eq!(last_movement, 0, "reset left something still moving");
    }

    #[test]
    fn foldback_matches_the_b3_manual_rules() {
        assert_eq!(drawbar_wheel(0, 0), Some(12));
        assert_eq!(drawbar_wheel(0, 2), Some(12));
        assert_eq!(drawbar_wheel(60, 8), Some(84));
    }

    /// The scan only visits keys that have something to do. Everything else
    /// on a 61-key manual is 549 contacts that cannot change.
    #[test]
    fn a_key_stops_being_scanned_once_its_contacts_settle() {
        let mut manual = Manual::new(48_000.0, &crate::preset::UPPER_PRESETS);
        assert!(manual.keys.iter().all(|key| !key.settling));

        assert!(manual.note_on(60, 1.0));
        let key = usize::from(60 - MANUAL_FIRST_NOTE);
        assert!(manual.keys[key].settling);
        for _ in 0..4_800 {
            manual.tick_contacts();
        }
        assert!(!manual.keys[key].settling);
        assert!(manual.keys[key].sounding());
        assert!(
            manual.keys[key]
                .contacts
                .iter()
                .all(|contact| contact.gate == 1.0)
        );

        assert!(manual.note_off(60, 1.0));
        assert!(manual.keys[key].settling);
        for _ in 0..4_800 {
            manual.tick_contacts();
        }
        assert!(
            manual
                .keys
                .iter()
                .all(|key| !key.settling && !key.sounding())
        );
        assert!(
            manual.keys[key]
                .contacts
                .iter()
                .all(|contact| contact.gate == 0.0)
        );
    }

    #[test]
    fn compartments_are_reciprocal_and_hold_the_documented_tooth_counts() {
        let teeth = |wheel: usize| crate::tonewheel::gear_teeth(wheel).expect("wheel");
        for wheel in 0..TONEWHEEL_COUNT {
            for companion in compartment_companions(wheel).into_iter().flatten() {
                assert!(
                    compartment_companions(companion).contains(&Some(wheel)),
                    "{wheel} and {companion} disagree"
                );
                assert_ne!(companion, wheel);
            }
        }

        // The lowest wheel shares its compartment with the 32, 8 and 128 tooth
        // wheels of the same speed, and its neighbour compartment holds 4, 64,
        // 16 and a blank where no 192-tooth wheel of that speed exists.
        let mut lowest = compartment_companions(0)
            .into_iter()
            .flatten()
            .map(teeth)
            .collect::<std::vec::Vec<_>>();
        lowest.sort_unstable();
        assert_eq!(lowest, [8, 32, 128]);

        let mut second = compartment_companions(12)
            .into_iter()
            .flatten()
            .map(teeth)
            .collect::<std::vec::Vec<_>>();
        second.sort_unstable();
        assert_eq!(second, [16, 64]);

        // A speed that does have a 192-tooth wheel fills the fourth position.
        let mut filled = compartment_companions(17)
            .into_iter()
            .flatten()
            .map(teeth)
            .collect::<std::vec::Vec<_>>();
        filled.sort_unstable();
        assert_eq!(filled, [16, 64, 192]);

        // Every wheel now belongs to a compartment; none is left out.
        for wheel in 0..TONEWHEEL_COUNT {
            assert!(
                compartment_companions(wheel).iter().any(Option::is_some),
                "wheel {wheel} has no compartment"
            );
        }
    }
}
