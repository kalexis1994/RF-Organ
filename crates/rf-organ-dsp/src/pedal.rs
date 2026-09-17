// SPDX-License-Identifier: GPL-2.0-or-later

use core::f32::consts::TAU;

use crate::tonewheel::TONEWHEEL_COUNT;

pub const PEDAL_DRAWBAR_COUNT: usize = 2;
pub const PEDAL_FIRST_NOTE: u8 = 24;
pub const PEDAL_KEY_COUNT: usize = 25;
const PEDAL_LAST_NOTE: u8 = PEDAL_FIRST_NOTE + PEDAL_KEY_COUNT as u8 - 1;
const PEDAL_CONTACT_COUNT: usize = 8;
const PEDAL_BUS_COUNT: usize = 4;
const DRAWBAR_LEVELS: [f32; 9] = [0.0, 0.089, 0.126, 0.178, 0.251, 0.355, 0.501, 0.708, 1.0];

// Physical spring order from the late 25-note console pedal switch: 10th,
// 12th, 6th, 8th, 2nd, 4th, fundamental and 3rd harmonic. Adjacent springs
// feed one of the four pedal busbars.
const CONTACT_OFFSETS: [usize; PEDAL_CONTACT_COUNT] = [40, 43, 31, 36, 12, 24, 0, 19];

// Reduced conductance ratios from the B-3/C-3 resistor panel. The 16' path
// uses 470, 47 and 10 ohm branches plus the filtered low bus; the 8' path
// uses 20, 5 and 5 ohm branches and deliberately omits the low bus.
const SIXTEEN_BUS_GAINS: [f32; PEDAL_BUS_COUNT] = [10.0 / 470.0, 10.0 / 47.0, 1.0, 1.0];
const EIGHT_BUS_GAINS: [f32; PEDAL_BUS_COUNT] = [0.25, 1.0, 1.0, 0.0];

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

    /// See the manual's contact: nothing left to count down, nothing left to
    /// bounce, gate already where the key put it.
    const fn settled(&self) -> bool {
        self.delay == 0 && self.bounce_left == 0 && self.gate == if self.target { 1.0 } else { 0.0 }
    }

    fn tick(&mut self) {
        if self.delay > 0 {
            self.delay -= 1;
            return;
        }
        if self.bounce_left == 0 {
            self.gate = f32::from(self.target);
            return;
        }
        self.bounce_left -= 1;
        if self.period_left == 0 {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            let settled = self.noise & 3 == 0;
            self.gate = f32::from(if settled { self.target } else { !self.target });
            self.period_left = self.period;
        } else {
            self.period_left -= 1;
        }
        if self.bounce_left == 0 {
            self.gate = f32::from(self.target);
        }
    }

    fn reset(&mut self) {
        self.gate = 0.0;
        self.target = false;
        self.delay = 0;
        self.bounce_left = 0;
        self.period_left = 0;
    }
}

#[derive(Clone, Copy)]
struct PedalKey {
    active: bool,
    /// Whether any of this pedal's contacts still has work to do.
    settling: bool,
    contacts: [Contact; PEDAL_CONTACT_COUNT],
}

impl PedalKey {
    const EMPTY: Self = Self {
        active: false,
        settling: false,
        contacts: [Contact::EMPTY; PEDAL_CONTACT_COUNT],
    };

    const fn sounding(&self) -> bool {
        self.active || self.settling
    }
}

/// Twenty-five-note console pedal clavier. Each key closes eight harmonic
/// contacts onto four shared busbars. The two drawbars receive different
/// resistor-panel mixtures of those buses, as in the late B-3/C-3 circuit.
pub struct Pedalboard {
    keys: [PedalKey; PEDAL_KEY_COUNT],
    drawbars: [u8; PEDAL_DRAWBAR_COUNT],
    sample_rate: f32,
    contact_spread: f32,
    contact_bounce: f32,
    event_counter: u32,
    low_bus_state: f32,
    low_bus_step: f32,
}

impl Pedalboard {
    pub fn new(sample_rate: f32) -> Self {
        let mut pedals = Self {
            keys: [PedalKey::EMPTY; PEDAL_KEY_COUNT],
            drawbars: [8, 0],
            sample_rate,
            contact_spread: 0.55,
            contact_bounce: 0.45,
            event_counter: 0,
            low_bus_state: 0.0,
            // L20 removes high-frequency switching energy from the low pedal
            // bus. This bounded one-pole is provisional until L20 is measured.
            low_bus_step: (TAU * 700.0 / sample_rate).min(1.0),
        };
        for key in 0..PEDAL_KEY_COUNT {
            for contact in 0..PEDAL_CONTACT_COUNT {
                pedals.keys[key].contacts[contact].wheel =
                    pedal_contact_wheel(key, contact).expect("bounded pedal contact") as u8;
            }
        }
        pedals
    }

    pub fn set_drawbar(&mut self, index: usize, position: u8) -> bool {
        if index >= PEDAL_DRAWBAR_COUNT || position > 8 {
            return false;
        }
        self.drawbars[index] = position;
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
        for contact in 0..PEDAL_CONTACT_COUNT {
            let order = PEDAL_CONTACT_COUNT - 1 - contact;
            let nominal = spread_samples * order as u32 / (PEDAL_CONTACT_COUNT as u32 - 1);
            let seed = hash(self.event_counter ^ ((key as u32) << 8) ^ contact as u32);
            let jitter = if spread_samples == 0 {
                0
            } else {
                seed % (spread_samples / 8 + 1)
            };
            self.keys[key].contacts[contact].schedule(
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
            for contact in &mut key.contacts {
                contact.tick();
                settling |= !contact.settled();
            }
            key.settling = settling;
        }
    }

    pub fn sample(&mut self, wheels: &[f32; TONEWHEEL_COUNT]) -> f32 {
        let mut buses = [0.0; PEDAL_BUS_COUNT];
        for key in self.keys.iter().filter(|key| key.sounding()) {
            for (contact_index, contact) in key.contacts.iter().enumerate() {
                buses[contact_index / 2] += contact.gate * wheels[contact.wheel as usize];
            }
        }

        self.low_bus_state += self.low_bus_step * (buses[3] - self.low_bus_state);
        buses[3] = self.low_bus_state;
        let sixteen_level = DRAWBAR_LEVELS[self.drawbars[0] as usize];
        let eight_level = DRAWBAR_LEVELS[self.drawbars[1] as usize];
        let mut output = 0.0;
        for bus in 0..PEDAL_BUS_COUNT {
            output += buses[bus]
                * (sixteen_level * SIXTEEN_BUS_GAINS[bus] + eight_level * EIGHT_BUS_GAINS[bus]);
        }
        output * 0.028
    }

    pub fn reset(&mut self) {
        self.low_bus_state = 0.0;
        for key in &mut self.keys {
            key.active = false;
            key.settling = false;
            for contact in &mut key.contacts {
                contact.reset();
            }
        }
    }
}

fn pedal_contact_wheel(key: usize, contact: usize) -> Option<usize> {
    if key >= PEDAL_KEY_COUNT || contact >= PEDAL_CONTACT_COUNT {
        return None;
    }
    Some(key + CONTACT_OFFSETS[contact])
}

fn key_index(note: u8) -> Option<usize> {
    (PEDAL_FIRST_NOTE..=PEDAL_LAST_NOTE)
        .contains(&note)
        .then(|| usize::from(note - PEDAL_FIRST_NOTE))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Same as the manuals: a settled pedal drops out of the scan and stops
    /// contributing, which is exact because its gates are then zero.
    #[test]
    fn a_pedal_stops_being_scanned_once_its_contacts_settle() {
        let mut pedals = Pedalboard::new(48_000.0);
        assert!(pedals.keys.iter().all(|key| !key.settling));
        assert!(pedals.note_on(24, 1.0));
        assert!(pedals.keys[0].settling);
        for _ in 0..4_800 {
            pedals.tick_contacts();
        }
        assert!(!pedals.keys[0].settling && pedals.keys[0].sounding());
        assert!(pedals.note_off(24, 1.0));
        for _ in 0..4_800 {
            pedals.tick_contacts();
        }
        assert!(pedals.keys.iter().all(|key| !key.sounding()));
        assert!(
            pedals.keys[0]
                .contacts
                .iter()
                .all(|contact| contact.gate == 0.0)
        );
    }

    #[test]
    fn pedal_range_and_drawbars_are_bounded() {
        let mut pedals = Pedalboard::new(48_000.0);
        assert!(!pedals.note_on(23, 1.0));
        assert!(pedals.note_on(24, 1.0));
        assert!(pedals.note_on(48, 1.0));
        assert!(!pedals.note_on(49, 1.0));
        assert!(pedals.set_drawbar(1, 8));
        assert!(!pedals.set_drawbar(2, 8));
        assert!(!pedals.set_drawbar(0, 9));
    }

    #[test]
    fn late_console_pedal_uses_eight_harmonic_contacts() {
        let wheels = core::array::from_fn(|contact| pedal_contact_wheel(0, contact).unwrap());
        assert_eq!(wheels, CONTACT_OFFSETS);
        assert_eq!(pedal_contact_wheel(24, 7), Some(43));
        assert_eq!(pedal_contact_wheel(25, 0), None);
        assert_eq!(pedal_contact_wheel(0, 8), None);
    }

    #[test]
    fn contact_spread_applies_to_the_pedal_stack() {
        let mut pedals = Pedalboard::new(48_000.0);
        assert!(pedals.set_contact_spread(1.0));
        assert!(pedals.set_contact_bounce(0.0));
        assert!(pedals.note_on(PEDAL_FIRST_NOTE, 0.25));
        let mut observed_partial_stack = false;
        for _ in 0..512 {
            pedals.tick_contacts();
            let closed = pedals.keys[0]
                .contacts
                .iter()
                .filter(|contact| contact.gate > 0.0)
                .count();
            observed_partial_stack |= (1..PEDAL_CONTACT_COUNT).contains(&closed);
        }
        assert!(observed_partial_stack);
    }

    #[test]
    fn key_release_has_no_musical_sustain_stage() {
        let mut pedals = Pedalboard::new(48_000.0);
        assert!(pedals.set_contact_spread(0.0));
        assert!(pedals.set_contact_bounce(0.0));
        assert!(pedals.note_on(PEDAL_FIRST_NOTE, 1.0));
        pedals.tick_contacts();
        assert!(
            pedals.keys[0]
                .contacts
                .iter()
                .all(|contact| contact.gate == 1.0)
        );
        assert!(pedals.note_off(PEDAL_FIRST_NOTE, 1.0));
        pedals.tick_contacts();
        assert!(
            pedals.keys[0]
                .contacts
                .iter()
                .all(|contact| contact.gate == 0.0)
        );
    }
}
