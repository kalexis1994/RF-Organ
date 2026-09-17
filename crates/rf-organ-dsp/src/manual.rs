// SPDX-License-Identifier: GPL-2.0-or-later
use crate::tonewheel::TONEWHEEL_COUNT;

pub const DRAWBAR_COUNT: usize = 9;
pub const MANUAL_FIRST_NOTE: u8 = 36;
pub const MANUAL_KEY_COUNT: usize = 61;
const MANUAL_LAST_NOTE: u8 = MANUAL_FIRST_NOTE + MANUAL_KEY_COUNT as u8 - 1;
const OFFSETS: [i16; DRAWBAR_COUNT] = [-12, 7, 0, 12, 19, 24, 28, 31, 36];
const DRAWBAR_LEVELS: [f32; 9] = [0.0, 0.089, 0.126, 0.178, 0.251, 0.355, 0.501, 0.708, 1.0];

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
        target: false,
        delay: 0,
        bounce_left: 0,
        period: 1,
        period_left: 0,
        noise: 1,
    };

    fn schedule(&mut self, target: bool, delay: u32, bounce: u32, period: u16, seed: u32) {
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
    const fn settled(&self) -> bool {
        self.delay == 0 && self.bounce_left == 0 && self.gate == if self.target { 1.0 } else { 0.0 }
    }

    fn tick(&mut self) -> bool {
        if self.delay > 0 {
            self.delay -= 1;
            return false;
        }
        let before = self.gate;
        if self.bounce_left == 0 {
            self.gate = if self.target { 1.0 } else { 0.0 };
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
    /// Whether any of this key's contacts still has work to do. A console has
    /// 549 manual contacts and almost all of them are idle at any moment.
    settling: bool,
    contacts: [Contact; DRAWBAR_COUNT],
}

impl Key {
    const EMPTY: Self = Self {
        active: false,
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
    drawbars: [u8; DRAWBAR_COUNT],
    wheel_gains: [f32; TONEWHEEL_COUNT],
    sample_rate: f32,
    contact_spread: f32,
    contact_bounce: f32,
    event_counter: u32,
}

impl Manual {
    pub fn new(sample_rate: f32) -> Self {
        let mut manual = Self {
            keys: [Key::EMPTY; MANUAL_KEY_COUNT],
            drawbars: [8, 8, 8, 0, 0, 0, 0, 0, 0],
            wheel_gains: [0.0; TONEWHEEL_COUNT],
            sample_rate,
            contact_spread: 0.55,
            contact_bounce: 0.45,
            event_counter: 0,
        };
        for key in 0..MANUAL_KEY_COUNT {
            for bus in 0..DRAWBAR_COUNT {
                manual.keys[key].contacts[bus].wheel =
                    drawbar_wheel(key, bus).expect("bounded contact") as u8;
            }
        }
        manual
    }

    pub fn set_drawbar(&mut self, index: usize, position: u8) -> bool {
        if index >= DRAWBAR_COUNT || position > 8 {
            return false;
        }
        self.drawbars[index] = position;
        self.rebuild_gains();
        true
    }

    pub fn drawbar(&self, index: usize) -> Option<u8> {
        self.drawbars.get(index).copied()
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
        self.schedule_key(key, false, velocity.max(0.25));
        true
    }

    fn schedule_key(&mut self, key: usize, target: bool, velocity: f32) {
        self.event_counter = self.event_counter.wrapping_add(1);
        self.keys[key].settling = true;
        let spread_seconds = self.contact_spread * (0.0004 + 0.008 * (1.0 - velocity));
        let spread_samples = (spread_seconds * self.sample_rate) as u32;
        let bounce_samples =
            (self.contact_bounce * (0.0015 + 0.004 * velocity) * self.sample_rate) as u32;
        let period = (self.sample_rate / 2_600.0).max(1.0) as u16;
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
            );
        }
    }

    pub fn tick_contacts(&mut self) {
        for key in &mut self.keys {
            if !key.settling {
                continue;
            }
            let mut settling = false;
            for (bus, contact) in key.contacts.iter_mut().enumerate() {
                let before = contact.gate;
                if contact.tick() {
                    let level = DRAWBAR_LEVELS[self.drawbars[bus] as usize];
                    self.wheel_gains[contact.wheel as usize] += (contact.gate - before) * level;
                }
                settling |= !contact.settled();
            }
            key.settling = settling;
        }
    }

    pub fn sample(
        &self,
        wheels: &[f32; TONEWHEEL_COUNT],
        leakage: f32,
        suppress_ninth_drawbar: bool,
    ) -> f32 {
        let mut output = 0.0;
        for (index, gain) in self.wheel_gains.iter().copied().enumerate() {
            if gain == 0.0 {
                continue;
            }
            let pair = compartment_pair(index);
            let leak = pair.map_or(0.0, |pair| wheels[pair]);
            output += gain * (wheels[index] + leakage * leak);
        }
        if suppress_ninth_drawbar {
            let level = DRAWBAR_LEVELS[self.drawbars[8] as usize];
            for key in self.keys.iter().filter(|key| key.sounding()) {
                let contact = key.contacts[8];
                let index = contact.wheel as usize;
                let leak = compartment_pair(index).map_or(0.0, |pair| wheels[pair]);
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

    fn rebuild_gains(&mut self) {
        self.wheel_gains.fill(0.0);
        for key in &self.keys {
            for (bus, contact) in key.contacts.iter().enumerate() {
                self.wheel_gains[contact.wheel as usize] +=
                    contact.gate * DRAWBAR_LEVELS[self.drawbars[bus] as usize];
            }
        }
    }
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

fn compartment_pair(index: usize) -> Option<usize> {
    match index {
        0..=35 => Some(index + 48),
        41..=47 => Some(index + 43),
        48..=83 => Some(index - 48),
        84..=90 => Some(index - 43),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut manual = Manual::new(48_000.0);
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
    fn compartment_pairs_are_reciprocal() {
        for wheel in 0..TONEWHEEL_COUNT {
            if let Some(pair) = compartment_pair(wheel) {
                assert_eq!(compartment_pair(pair), Some(wheel));
            }
        }
    }
}
