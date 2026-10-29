// SPDX-License-Identifier: GPL-2.0-or-later
//! RackForge adapter for the RF-Organ physical-model instrument.

mod pedal;
mod settings;

use pedal::{HOLD_TO_BRAKE_S, SpeedPedal, Switch};
use rackforge_plugin_sdk::{
    MIDI_FAMILY_CONTROL, MIDI_FAMILY_NOTE, MIDI2_FLAG_ORIGIN_7BIT, MIDI2_KIND_CONTROL_CHANGE,
    MIDI2_KIND_NOTE_OFF, MIDI2_KIND_NOTE_ON, MidiEvent, MidiEvent2, ParameterEvent, Processor,
    export_processor,
};
use rf_organ_dsp::{
    LEVEL_SILENT_DB, MIC_DISTANCE_DEFAULT_M, MIC_DISTANCE_RANGE_M, MIC_SPACING_DEFAULT_M,
    MIC_SPACING_MAX_M, OrganEngine, OrganPart, Registration, RotaryMode,
};
use settings::{LOWER_REGISTRATION, REGISTRATION, ROTARY_MODE};
pub use settings::{PARAMETER_COUNT, Settings, presets};

pub const MAX_FRAMES: u32 = 4096;
pub const MAX_EVENTS: usize = 256;
pub const STATE_VERSION: u32 = 24;
pub const STATE_BYTES: usize = state_bytes(PARAMETER_COUNT);

/// Every state this plugin has written, as its version and how many
/// parameters it carried. Each version only ever appended parameters, so an
/// older state is the first that many of today's, converted where a meaning
/// changed. A new version is one more row here.
const STATE_LAYOUTS: [(u32, usize); STATE_VERSION as usize] = [
    (1, 19),
    (2, 24),
    (3, 37),
    (4, 41),
    (5, 45),
    (6, 51),
    (7, 55),
    (8, 57),
    (9, 58),
    (10, 60),
    (11, 61),
    (12, 64),
    (13, 66),
    (14, 67),
    (15, 68),
    (16, 69),
    (17, 72),
    (18, 73),
    (19, 74),
    (20, 75),
    (21, 95),
    (22, 96),
    (23, 97),
    (STATE_VERSION, PARAMETER_COUNT),
];

/// The size of a state carrying `fields` parameters: a magic, a version and
/// one `f64` each.
const fn state_bytes(fields: usize) -> usize {
    8 + fields * 8
}

/// A gain as decibels, with nothing at all reported as the silence the
/// volumes bottom out at.
fn decibels_of(gain: f64) -> f64 {
    if gain <= 0.0 {
        return f64::from(LEVEL_SILENT_DB);
    }
    (20.0 * gain.log10()).max(f64::from(LEVEL_SILENT_DB))
}

#[derive(Default)]
pub struct RfOrganProcessor {
    engine: Option<Box<OrganEngine>>,
    settings: Settings,
    /// The speed the rotors go back to when CC 4 lets them go: the program's,
    /// until CC 1 throws the switch to another one.
    rotary_speed: RotaryMode,
    /// The mode the cabinet was last put in, by a program, a control or
    /// MIDI: the sustain pedal needs to know whether the rotors are stopped.
    rotary_engaged: RotaryMode,
    pedal: SpeedPedal,
    /// How many frames an alternating press has to be held to stop the
    /// rotors, at the rate the engine was prepared for.
    hold_frames: u32,
    maximum_frames: u32,
    channels: u32,
}

impl RfOrganProcessor {
    fn apply_settings(&mut self, settings: Settings) -> bool {
        if !settings.valid() {
            return false;
        }
        self.settings = settings;
        self.rotary_speed = settings.rotary_mode;
        self.rotary_engaged = settings.rotary_mode;
        self.pedal.cancel_hold();
        if let Some(engine) = &mut self.engine {
            settings.apply(engine);
        }
        true
    }

    /// Puts the cabinet in `mode`, remembering it for the sustain pedal.
    fn engage_rotary(&mut self, mode: RotaryMode) {
        self.rotary_engaged = mode;
        if let Some(engine) = &mut self.engine {
            engine.set_rotary_mode(mode);
        }
    }

    /// The speed switch on CC 1: below the middle is Chorale.
    fn select_rotary_speed(&mut self, fast: bool) {
        self.rotary_speed = if fast {
            RotaryMode::Tremolo
        } else {
            RotaryMode::Chorale
        };
        self.engage_rotary(self.rotary_speed);
    }

    /// The rotor stop on CC 4: releasing it returns to the selected speed.
    fn stop_rotors(&mut self, stop: bool) {
        self.engage_rotary(if stop {
            RotaryMode::Brake
        } else {
            self.rotary_speed
        });
    }

    /// The sustain pedal on CC 64, down at or past the middle.
    fn sustain_pedal(&mut self, down: bool) {
        let change = self.pedal.set(
            down,
            self.settings.sustain_pedal,
            self.rotary_speed,
            self.rotary_engaged,
        );
        self.switch_rotary(change);
    }

    /// Presses one of the twelve reverse-colour keys from the keyboard.
    ///
    /// This is the drop-down, reached from MIDI rather than from the screen:
    /// the same parameter, set through the same door, so the engine changes
    /// in this block, the surface reads the new key back on its next poll,
    /// and a saved project keeps it. Nothing else is applied, so whatever CC 1
    /// or CC 11 last set is left where it was.
    fn press_preset_key(&mut self, (part, key): (OrganPart, Registration)) {
        let index = match part {
            OrganPart::Upper => REGISTRATION,
            OrganPart::Lower => LOWER_REGISTRATION,
            OrganPart::Pedal => return,
        };
        let _ = self.set_parameter(index, f64::from(key.index()));
    }

    fn switch_rotary(&mut self, change: Option<Switch>) {
        if let Some(Switch { selected, engaged }) = change {
            self.rotary_speed = selected;
            self.engage_rotary(engaged);
        }
    }

    fn midi1(&mut self, event: &MidiEvent) {
        let [status, index, value] = event.data;
        let part = part_from_channel(status & 0x0f);
        if matches!(status & 0xf0, 0x80 | 0x90)
            && let Some(key) = part.and_then(|part| preset_key(part, index))
        {
            // A note-on with no velocity is a release, and a preset key
            // ignores its release: it stays down until another is pressed.
            if status & 0xf0 == 0x90 && value != 0 {
                self.press_preset_key(key);
            }
            return;
        }
        let Some(engine) = &mut self.engine else {
            return;
        };
        match status & 0xf0 {
            0x90 if value != 0 => {
                if let Some(part) = part {
                    let _ = engine.note_on_part(part, index, f32::from(value) / 127.0);
                }
            }
            0x80 | 0x90 => {
                if let Some(part) = part {
                    let _ = engine.note_off_part(part, index, f32::from(value) / 127.0);
                }
            }
            0xb0 => match index {
                1 => self.select_rotary_speed(value >= 64),
                // Rotor stop, the third position a cabinet's switch has.
                // Releasing it returns the rotors to the selected speed.
                4 => self.stop_rotors(value < 64),
                64 => self.sustain_pedal(value >= 64),
                11 => {
                    let _ = engine.set_expression(f32::from(value) / 127.0);
                }
                120 | 123 => {
                    if let Some(part) = part {
                        engine.all_notes_off_part(part);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn midi2(&mut self, event: &MidiEvent2) {
        let part = part_from_channel(event.channel);
        if matches!(event.kind, MIDI2_KIND_NOTE_ON | MIDI2_KIND_NOTE_OFF)
            && let Some(key) = part.and_then(|part| preset_key(part, event.index))
        {
            if event.kind == MIDI2_KIND_NOTE_ON {
                self.press_preset_key(key);
            }
            return;
        }
        let Some(engine) = &mut self.engine else {
            return;
        };
        match event.kind {
            MIDI2_KIND_NOTE_ON => {
                let velocity = if event.flags & MIDI2_FLAG_ORIGIN_7BIT != 0 {
                    (event.value >> 9) as f32 / 127.0
                } else {
                    event.value.max(1) as f32 / 65_535.0
                };
                if let Some(part) = part {
                    let _ = engine.note_on_part(part, event.index, velocity);
                }
            }
            MIDI2_KIND_NOTE_OFF => {
                if let Some(part) = part {
                    let _ = engine.note_off_part(
                        part,
                        event.index,
                        event.value.min(65_535) as f32 / 65_535.0,
                    );
                }
            }
            MIDI2_KIND_CONTROL_CHANGE => {
                let value = if event.flags & MIDI2_FLAG_ORIGIN_7BIT != 0 {
                    (event.value >> 25) as f32 / 127.0
                } else {
                    event.value as f32 / u32::MAX as f32
                };
                match event.index {
                    1 => self.select_rotary_speed(value >= 0.5),
                    4 => self.stop_rotors(value < 0.5),
                    64 => self.sustain_pedal(value >= 0.5),
                    11 => {
                        let _ = engine.set_expression(value);
                    }
                    120 | 123 => {
                        if let Some(part) = part {
                            engine.all_notes_off_part(part);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Processor for RfOrganProcessor {
    fn prepare(&mut self, rate: f64, frames: u32, inputs: u32, outputs: u32) -> bool {
        if frames == 0 || frames > MAX_FRAMES || inputs != 0 || !(1..=2).contains(&outputs) {
            return false;
        }
        let Ok(mut engine) = OrganEngine::new(rate as f32) else {
            return false;
        };
        self.settings.apply(&mut engine);
        self.engine = Some(Box::new(engine));
        self.hold_frames = (rate * HOLD_TO_BRAKE_S).round().max(1.0) as u32;
        self.maximum_frames = frames;
        self.channels = outputs;
        true
    }

    fn set_parameter(&mut self, index: u32, value: f64) -> bool {
        let Some(settings) = self.settings.with_parameter(index, value) else {
            return false;
        };
        self.settings = settings;
        if index == ROTARY_MODE {
            self.rotary_speed = settings.rotary_mode;
            self.rotary_engaged = settings.rotary_mode;
            self.pedal.cancel_hold();
        }
        if let Some(engine) = &mut self.engine {
            settings.apply_parameter(index, engine);
        }
        true
    }

    fn get_parameter(&self, index: u32) -> Option<f64> {
        self.settings.parameter(index)
    }

    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.reset();
            self.settings.apply(engine);
        }
        self.rotary_speed = self.settings.rotary_mode;
        self.rotary_engaged = self.settings.rotary_mode;
        self.pedal = SpeedPedal::default();
    }

    fn load_preset(&mut self, id: &str) -> bool {
        let Some((_, _, _, settings)) = presets().into_iter().find(|preset| preset.0 == id) else {
            return false;
        };
        self.apply_settings(settings)
    }

    fn save_state(&self, destination: &mut [u8]) -> Option<usize> {
        let destination = destination.get_mut(..STATE_BYTES)?;
        destination[..4].copy_from_slice(b"RFOR");
        destination[4..8].copy_from_slice(&STATE_VERSION.to_le_bytes());
        for index in 0..PARAMETER_COUNT as u32 {
            let value = self
                .settings
                .parameter(index)
                .expect("fixed parameter schema");
            let offset = 8 + index as usize * 8;
            destination[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        Some(STATE_BYTES)
    }

    fn load_state(&mut self, state: &[u8]) -> bool {
        if state.get(..4) != Some(b"RFOR".as_slice()) {
            return false;
        }
        let Some(version) = state
            .get(4..8)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four bytes")))
        else {
            return false;
        };
        let Some(&(_, fields)) = STATE_LAYOUTS
            .iter()
            .find(|(written, fields)| *written == version && state_bytes(*fields) == state.len())
        else {
            return false;
        };
        let mut settings = Settings::default();
        // Up to version twenty-two the defaults stood a spaced pair in front
        // of each rotor. A state is read over the placement it was saved
        // against, so that what it is too old to carry does not move to the
        // studio placement that is the default now.
        if version <= 22 {
            for (index, earlier) in [
                (41, f64::from(MIC_DISTANCE_DEFAULT_M)),
                (62, f64::from(MIC_SPACING_DEFAULT_M)),
                (64, 0.0),
            ] {
                let Some(updated) = settings.with_parameter(index, earlier) else {
                    return false;
                };
                settings = updated;
            }
        }
        // Up to version ten the horn and the drum shared one bipolar balance
        // between them. They have a volume each now, as Hammond documents
        // them, so the old control has to be split rather than read.
        let mut balance = None;
        // Up to version eleven the two pairs of microphones shared one
        // distance, one spacing and one offset, and the drum's offset was the
        // horn's negated. They stand separately now, as Hammond places them,
        // so an older state has to put the drum's pair where that shared
        // setting had it.
        let mut shared_stand = [None; 3];
        for index in 0..fields as u32 {
            let offset = 8 + index as usize * 8;
            let value = f64::from_le_bytes(
                state[offset..offset + 8]
                    .try_into()
                    .expect("validated state"),
            );
            // Before 0.24.0 the microphone placement was two numbers from
            // zero to one. It is a distance and a spacing in metres now, so an
            // older state has to be converted and not read straight through.
            let value = match (version, index) {
                (..=6, 41) => {
                    let (near, far) = MIC_DISTANCE_RANGE_M;
                    f64::from(near) + value * f64::from(far - near)
                }
                (..=6, 42) => value * f64::from(MIC_SPACING_MAX_M),
                (..=10, 44) => {
                    balance = Some(value);
                    decibels_of(1.0 + value.min(0.0))
                }
                _ => value,
            };
            // Whatever that conversion arrived at is what the drum's pair
            // should inherit, so it is recorded after the conversion and not
            // before it.
            if version <= 11 {
                match index {
                    41 => shared_stand[0] = Some(value),
                    42 => shared_stand[1] = Some(value),
                    51 => shared_stand[2] = Some(-value),
                    _ => {}
                }
            }
            let Some(updated) = settings.with_parameter(index, value) else {
                return false;
            };
            settings = updated;
        }
        for (index, value) in [61, 62, 63].into_iter().zip(shared_stand) {
            let Some(value) = value else {
                continue;
            };
            let Some(updated) = settings.with_parameter(index, value) else {
                return false;
            };
            settings = updated;
        }
        // Up to version twenty-two every microphone was one capsule, so the
        // drum's pair takes the capsule the state had.
        if version <= 22 {
            let shared_capsule = settings.parameter(59).expect("fixed parameter schema");
            let Some(updated) =
                settings.with_parameter(settings::DRUM_MICROPHONE_TYPE, shared_capsule)
            else {
                return false;
            };
            settings = updated;
        }
        if let Some(balance) = balance {
            let Some(updated) = settings.with_parameter(60, decibels_of(1.0 - balance.max(0.0)))
            else {
                return false;
            };
            settings = updated;
        }
        self.apply_settings(settings)
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        midi: &[MidiEvent],
        parameters: &[ParameterEvent],
        frames: u32,
        inputs: u32,
        outputs: u32,
    ) {
        self.process_wide(
            input,
            output,
            midi,
            &[],
            parameters,
            frames,
            inputs,
            outputs,
        );
    }

    fn process_wide(
        &mut self,
        _input: &[f32],
        output: &mut [f32],
        midi: &[MidiEvent],
        midi2: &[MidiEvent2],
        parameters: &[ParameterEvent],
        frames: u32,
        inputs: u32,
        outputs: u32,
    ) {
        output.fill(0.0);
        let samples = (frames as usize).checked_mul(outputs as usize);
        if self.engine.is_none()
            || frames > self.maximum_frames
            || inputs != 0
            || outputs != self.channels
            || samples.is_none_or(|count| count > output.len())
            || !ordered(midi.iter().map(|event| event.frame), frames)
            || !ordered(midi2.iter().map(|event| event.frame), frames)
            || !ordered(parameters.iter().map(|event| event.frame), frames)
            || midi.iter().any(|event| !valid_midi1(event))
            || midi2.iter().any(|event| {
                event.channel >= 16
                    || event.index >= 128
                    || (matches!(event.kind, MIDI2_KIND_NOTE_ON | MIDI2_KIND_NOTE_OFF)
                        && event.value > 65_535)
            })
            || parameters.iter().any(|event| {
                self.settings
                    .with_parameter(event.index, event.value)
                    .is_none()
            })
        {
            return;
        }

        let (mut parameter_index, mut midi_index, mut midi2_index) = (0, 0, 0);
        for frame in 0..frames {
            while parameter_index < parameters.len() && parameters[parameter_index].frame == frame {
                let event = parameters[parameter_index];
                let _ = self.set_parameter(event.index, event.value);
                parameter_index += 1;
            }
            while midi_index < midi.len() && midi[midi_index].frame == frame {
                self.midi1(&midi[midi_index]);
                midi_index += 1;
            }
            while midi2_index < midi2.len() && midi2[midi2_index].frame == frame {
                self.midi2(&midi2[midi2_index]);
                midi2_index += 1;
            }
            let held = self.pedal.tick(self.hold_frames);
            self.switch_rotary(held);
            let [left, right] = self.engine.as_mut().expect("prepared engine").next_sample();
            let offset = frame as usize * outputs as usize;
            output[offset] = if outputs == 1 {
                (left + right) * 0.5
            } else {
                left
            };
            if outputs == 2 {
                output[offset + 1] = right;
            }
        }
    }
}

fn ordered(frames: impl Iterator<Item = u32>, block_frames: u32) -> bool {
    let mut previous = 0;
    let mut count = 0;
    for frame in frames {
        count += 1;
        if count > MAX_EVENTS || frame >= block_frames || frame < previous {
            return false;
        }
        previous = frame;
    }
    true
}

fn valid_midi1(event: &MidiEvent) -> bool {
    if !(1..=3).contains(&event.length) || event.data[0] < 128 {
        return false;
    }
    if event.data[1..event.length as usize]
        .iter()
        .any(|value| *value >= 128)
    {
        return false;
    }
    !matches!(event.data[0] & 0xf0, 0x80 | 0x90 | 0xb0) || event.length == 3
}

/// The lowest of the twelve reverse-colour keys, as MIDI sees it.
///
/// On the console they sit at the left end of each manual, the octave below
/// its sixty-one playing keys. The manuals here start at MIDI 36, so the
/// preset octave is 24 to 35 - the bottom full octave of an eighty-eight-key
/// controller - and it runs in the order the keys are laid out: the cancel
/// key at C, the nine presets from C sharp to A, the two adjust keys at A
/// sharp and B. The pedal clavier has no preset keys and its notes start at
/// 24 too, on its own channel, so it is not affected.
const PRESET_KEY_FIRST: u8 = 24;

/// Which preset key a note on a manual's channel presses, if it presses one.
fn preset_key(part: OrganPart, note: u8) -> Option<(OrganPart, Registration)> {
    if part == OrganPart::Pedal {
        return None;
    }
    let position = note.checked_sub(PRESET_KEY_FIRST)?;
    Registration::KEYBOARD
        .get(usize::from(position))
        .map(|key| (part, *key))
}

fn part_from_channel(channel: u8) -> Option<OrganPart> {
    match channel {
        0 => Some(OrganPart::Upper),
        1 => Some(OrganPart::Lower),
        2 => Some(OrganPart::Pedal),
        _ => None,
    }
}

export_processor!(RfOrganProcessor,
    max_frames = 4096, max_input_channels = 0, max_output_channels = 2,
    max_midi_events = 256, max_parameter_events = 256, max_transfer_bytes = 1024,
    midi2 = { max_events = 256, families = MIDI_FAMILY_NOTE | MIDI_FAMILY_CONTROL }
);

#[cfg(test)]
mod tests {
    use super::*;
    use rf_organ_dsp::MicrophoneType;

    /// The size of the state `version` wrote, as a constant a test can size
    /// an array with.
    const fn written_bytes(version: u32) -> usize {
        let mut row = 0;
        while row < STATE_LAYOUTS.len() {
            if STATE_LAYOUTS[row].0 == version {
                return state_bytes(STATE_LAYOUTS[row].1);
            }
            row += 1;
        }
        panic!("no state was written at that version");
    }

    /// Reading an old state relies on every version having appended to the
    /// one before it, and on the last row being what `save_state` writes.
    #[test]
    fn every_state_version_appends_to_the_one_before() {
        let mut previous = (0, 0);
        for (version, fields) in STATE_LAYOUTS {
            assert_eq!(version, previous.0 + 1, "version {version} is out of turn");
            assert!(fields > previous.1, "version {version} carries no more");
            previous = (version, fields);
        }
        assert_eq!(previous, (STATE_VERSION, PARAMETER_COUNT));
        assert_eq!(written_bytes(STATE_VERSION), STATE_BYTES);
    }

    /// A state that names one version and has the size of another, or is
    /// cut short, or is not ours, is refused rather than half read.
    #[test]
    fn a_state_that_does_not_match_its_version_is_refused() {
        let source = RfOrganProcessor::default();
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));

        let mut mislabelled = current;
        mislabelled[4..8].copy_from_slice(&20_u32.to_le_bytes());
        let mut restored = RfOrganProcessor::default();
        assert!(!restored.load_state(&mislabelled));
        assert!(!restored.load_state(&current[..written_bytes(20)]));
        assert!(!restored.load_state(&current[..6]));
        assert!(!restored.load_state(&[]));

        let mut foreign = current;
        foreign[..4].copy_from_slice(b"XXXX");
        assert!(!restored.load_state(&foreign));
        assert!(restored.load_state(&current));
    }

    #[test]
    fn state_round_trip_and_factory_presets() {
        let mut source = RfOrganProcessor::default();
        assert!(source.load_preset("gospel-full"));
        let mut bytes = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut bytes), Some(STATE_BYTES));
        let mut restored = RfOrganProcessor::default();
        assert!(restored.load_state(&bytes));
        assert_eq!(restored.settings, source.settings);
    }

    /// The one balance that used to sit between the horn and the drum has to
    /// come back as the two volumes it stood for: turning it toward the drum
    /// meant turning the horn down, and the other way round.
    #[test]
    fn the_old_balance_becomes_two_volumes() {
        let source = RfOrganProcessor {
            settings: Settings::default(),
            ..RfOrganProcessor::default()
        };
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));

        for (balance, horn, drum) in [
            (0.0_f64, 0.0_f64, 0.0_f64),
            (-0.5, -6.020_6, 0.0),
            (0.5, 0.0, -6.020_6),
            (-1.0, f64::from(LEVEL_SILENT_DB), 0.0),
            (1.0, 0.0, f64::from(LEVEL_SILENT_DB)),
        ] {
            let mut legacy = [0_u8; written_bytes(10)];
            legacy.copy_from_slice(&current[..written_bytes(10)]);
            legacy[4..8].copy_from_slice(&10_u32.to_le_bytes());
            let offset = 8 + 44 * 8;
            legacy[offset..offset + 8].copy_from_slice(&balance.to_le_bytes());

            let mut restored = RfOrganProcessor::default();
            assert!(
                restored.load_state(&legacy),
                "balance {balance} was refused"
            );
            assert!(
                (restored.settings.rotary_horn_level - horn).abs() < 1.0e-3,
                "balance {balance} gave the horn {}",
                restored.settings.rotary_horn_level
            );
            assert!(
                (restored.settings.rotary_drum_level - drum).abs() < 1.0e-3,
                "balance {balance} gave the drum {}",
                restored.settings.rotary_drum_level
            );
        }
    }

    #[test]
    fn legacy_states_migrate_with_new_controls_at_defaults() {
        let source = RfOrganProcessor::default();
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));
        let saved_against = Settings {
            rotary_horn_mic_distance: f64::from(MIC_DISTANCE_DEFAULT_M),
            rotary_horn_mic_sides: false,
            rotary_drum_mic_spacing: f64::from(MIC_SPACING_DEFAULT_M),
            rotary_drum_microphone_type: Settings::default().rotary_microphone_type,
            ..Settings::default()
        };
        let mut legacy = [0_u8; written_bytes(1)];
        legacy.copy_from_slice(&current[..written_bytes(1)]);
        legacy[4..8].copy_from_slice(&1_u32.to_le_bytes());
        let mut restored = RfOrganProcessor::default();
        assert!(restored.load_state(&legacy));
        assert_eq!(restored.settings, saved_against);

        let mut version_two = [0_u8; written_bytes(2)];
        version_two.copy_from_slice(&current[..written_bytes(2)]);
        version_two[4..8].copy_from_slice(&2_u32.to_le_bytes());
        assert!(restored.load_state(&version_two));
        assert_eq!(restored.settings, saved_against);

        let mut version_three = [0_u8; written_bytes(3)];
        version_three.copy_from_slice(&current[..written_bytes(3)]);
        version_three[4..8].copy_from_slice(&3_u32.to_le_bytes());
        assert!(restored.load_state(&version_three));
        assert_eq!(restored.settings, saved_against);

        let mut version_four = [0_u8; written_bytes(4)];
        version_four.copy_from_slice(&current[..written_bytes(4)]);
        version_four[4..8].copy_from_slice(&4_u32.to_le_bytes());
        assert!(restored.load_state(&version_four));
        assert_eq!(restored.settings, saved_against);

        // Every version up to six placed the microphones with two numbers
        // from zero to one, which this version reads as metres, so those
        // states have to arrive converted rather than merely accepted.
        let place_in_units = |state: &mut [u8]| {
            for (index, unit) in [(41_usize, 0.35_f64), (42, 0.75)] {
                let offset = 8 + index * 8;
                state[offset..offset + 8].copy_from_slice(&unit.to_le_bytes());
            }
        };
        let (near, far) = MIC_DISTANCE_RANGE_M;
        // Both pairs get what the one shared stand had.
        let distance = f64::from(near) + 0.35 * f64::from(far - near);
        let spacing = 0.75 * f64::from(MIC_SPACING_MAX_M);
        let migrated = Settings {
            rotary_horn_mic_distance: distance,
            rotary_horn_mic_spacing: spacing,
            rotary_drum_mic_distance: distance,
            rotary_drum_mic_spacing: spacing,
            ..saved_against
        };

        let mut version_five = [0_u8; written_bytes(5)];
        version_five.copy_from_slice(&current[..written_bytes(5)]);
        version_five[4..8].copy_from_slice(&5_u32.to_le_bytes());
        place_in_units(&mut version_five);
        assert!(restored.load_state(&version_five));
        assert_eq!(restored.settings, migrated);
        assert_eq!(restored.settings.transformer_trims, [[0.0; 2]; 3]);

        let mut version_six = [0_u8; written_bytes(6)];
        version_six.copy_from_slice(&current[..written_bytes(6)]);
        version_six[4..8].copy_from_slice(&6_u32.to_le_bytes());
        place_in_units(&mut version_six);
        assert!(restored.load_state(&version_six));
        assert_eq!(restored.settings, migrated);
        assert_eq!(restored.settings.rotary_horn_mic_offset, 0.0);
        assert_eq!(restored.settings.rotary_drum_mic_offset, 0.0);
    }

    #[test]
    fn note_events_produce_audio_at_their_frame() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.prepare(48_000.0, 128, 0, 2));
        let midi = [MidiEvent {
            frame: 16,
            data: [0x90, 60, 110],
            length: 3,
        }];
        let mut output = [0.0_f32; 256];
        processor.process(&[], &mut output, &midi, &[], 128, 0, 2);
        assert!(output[..32].iter().all(|sample| *sample == 0.0));
        assert!(output[32..].iter().any(|sample| sample.abs() > 1.0e-6));
    }

    #[test]
    fn invalid_block_contract_returns_silence() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.prepare(48_000.0, 64, 0, 2));
        let mut output = [1.0_f32; 128];
        processor.process(&[], &mut output, &[], &[], 65, 0, 2);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    /// A cabinet's switch has three positions, and the third one is reachable
    /// from MIDI: stop the rotors, then hand them back to whatever speed the
    /// program selected.
    #[test]
    fn continuous_controller_four_stops_and_releases_the_rotors() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.load_preset("chorale-888"));
        assert!(processor.prepare(48_000.0, 64, 0, 2));
        assert_eq!(processor.settings.rotary_mode, RotaryMode::Chorale);

        let mut output = [0.0_f32; 128];
        let brake = [MidiEvent {
            frame: 0,
            data: [0xb0, 4, 0],
            length: 3,
        }];
        processor.process(&[], &mut output, &brake, &[], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.rotary_mode(), RotaryMode::Brake);

        let release = [MidiEvent {
            frame: 0,
            data: [0xb0, 4, 127],
            length: 3,
        }];
        processor.process(&[], &mut output, &release, &[], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.rotary_mode(), RotaryMode::Chorale);
    }

    /// A state saved before the cabinet amplifier had a drive control comes
    /// back with the drive where the control starts.
    #[test]
    fn a_state_from_before_the_drive_starts_at_its_default() {
        let source = RfOrganProcessor::default();
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));
        let mut older = [0_u8; written_bytes(21)];
        older.copy_from_slice(&current[..written_bytes(21)]);
        older[4..8].copy_from_slice(&21_u32.to_le_bytes());
        let mut restored = RfOrganProcessor::default();
        assert!(restored.load_state(&older));
        assert_eq!(
            restored.settings.rotary_drive,
            f64::from(rf_organ_dsp::AMP_DRIVE_DEFAULT)
        );
    }

    /// Before version twenty-three one capsule stood for every microphone,
    /// so the drum's pair takes whatever that capsule was.
    #[test]
    fn a_shared_capsule_becomes_both_pairs() {
        let mut source = RfOrganProcessor::default();
        assert!(source.set_parameter(59, f64::from(MicrophoneType::Dynamic as u8)));
        assert!(source.set_parameter(
            settings::DRUM_MICROPHONE_TYPE,
            f64::from(MicrophoneType::Condenser as u8)
        ));
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));
        let mut older = [0_u8; written_bytes(22)];
        older.copy_from_slice(&current[..written_bytes(22)]);
        older[4..8].copy_from_slice(&22_u32.to_le_bytes());
        let mut restored = RfOrganProcessor::default();
        assert!(restored.load_state(&older));
        assert_eq!(
            restored.settings.rotary_microphone_type,
            MicrophoneType::Dynamic
        );
        assert_eq!(
            restored.settings.rotary_drum_microphone_type,
            MicrophoneType::Dynamic
        );
    }

    /// A state too old to carry where the microphones stood comes back with
    /// them where they stood when it was saved - a spaced pair in front of
    /// each rotor - and not at the studio placement that is the default now.
    #[test]
    fn an_old_state_keeps_the_placement_it_was_saved_against() {
        let source = RfOrganProcessor::default();
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));
        // Version twelve carried both distances and the drum's spacing, but
        // not whether the horn's pair stood at the sides.
        let mut older = [0_u8; written_bytes(12)];
        older.copy_from_slice(&current[..written_bytes(12)]);
        older[4..8].copy_from_slice(&12_u32.to_le_bytes());
        let mut restored = RfOrganProcessor::default();
        assert!(restored.load_state(&older));
        assert!(!restored.settings.rotary_horn_mic_sides);
        // What the old state did carry is its own.
        assert_eq!(
            restored.settings.rotary_horn_mic_distance,
            source.settings.rotary_horn_mic_distance
        );
        assert_eq!(
            restored.settings.rotary_drum_mic_spacing,
            source.settings.rotary_drum_mic_spacing
        );
    }

    /// The drive reaches the cabinet: a chord through the rotary speaker
    /// with the amplifier driven hard comes out flatter-topped than the
    /// same chord with it clean.
    #[test]
    fn the_drive_reaches_the_cabinet_amplifier() {
        let render = |drive: f64| {
            let mut processor = RfOrganProcessor::default();
            assert!(processor.load_preset("chorale-888"));
            assert!(processor.set_parameter(settings::ROTARY_DRIVE, drive));
            assert!(processor.prepare(48_000.0, 512, 0, 2));
            let chord = [48, 52, 55, 60, 64, 67].map(|note| MidiEvent {
                frame: 0,
                data: [0x90, note, 127],
                length: 3,
            });
            let mut output = [0.0_f32; 1024];
            let (mut peak, mut power) = (0.0_f32, 0.0_f64);
            for block in 0..60 {
                let midi: &[MidiEvent] = if block == 0 { &chord } else { &[] };
                processor.process(&[], &mut output, midi, &[], 512, 0, 2);
                if block >= 20 {
                    for sample in output {
                        peak = peak.max(sample.abs());
                        power += f64::from(sample) * f64::from(sample);
                    }
                }
            }
            f64::from(peak) / (power / (40.0 * 1024.0)).sqrt()
        };
        let clean = render(0.0);
        let driven = render(1.0);
        assert!(
            driven < clean * 0.9,
            "crest {clean:.3} clean, {driven:.3} driven"
        );
    }

    fn controller(frame: u32, index: u8, value: u8) -> MidiEvent {
        MidiEvent {
            frame,
            data: [0xb0, index, value],
            length: 3,
        }
    }

    /// The speed the rotors return to is the one last chosen, and CC 1 is
    /// one way of choosing it: a player who throws the cabinet to fast and
    /// then stops it expects it to come back fast.
    #[test]
    fn releasing_the_stop_returns_to_the_speed_last_chosen() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.load_preset("chorale-888"));
        assert!(processor.prepare(48_000.0, 64, 0, 2));
        let mut output = [0.0_f32; 128];

        let fast_then_stop = [controller(0, 1, 127), controller(8, 4, 0)];
        processor.process(&[], &mut output, &fast_then_stop, &[], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.rotary_mode(), RotaryMode::Brake);

        processor.process(&[], &mut output, &[controller(0, 4, 127)], &[], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.rotary_mode(), RotaryMode::Tremolo);

        // The parameter is the other way of choosing it.
        let chorale = ParameterEvent {
            frame: 0,
            index: ROTARY_MODE,
            value: f64::from(RotaryMode::Chorale as u8),
        };
        let stop_then_release = [controller(8, 4, 0), controller(16, 4, 127)];
        processor.process(&[], &mut output, &stop_then_release, &[chorale], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.rotary_mode(), RotaryMode::Chorale);
    }

    /// The sustain pedal is the cabinet's speed switch: a tap throws it, a
    /// second of holding stops the rotors, and the next tap lets them go at
    /// the speed they had. The hold is counted across blocks, in frames.
    #[test]
    fn the_sustain_pedal_switches_speed_and_a_hold_stops_the_rotors() {
        const BLOCK: u32 = 512;
        let mut processor = RfOrganProcessor::default();
        assert!(processor.load_preset("chorale-888"));
        assert!(processor.prepare(48_000.0, BLOCK, 0, 2));
        let mut output = [0.0_f32; 2 * BLOCK as usize];
        let mut play = |processor: &mut RfOrganProcessor, midi: &[MidiEvent]| {
            processor.process(&[], &mut output, midi, &[], BLOCK, 0, 2);
            processor
                .engine
                .as_ref()
                .expect("prepared engine")
                .rotary_mode()
        };

        let tap = [controller(0, 64, 127), controller(100, 64, 0)];
        assert_eq!(play(&mut processor, &tap), RotaryMode::Tremolo);
        assert_eq!(play(&mut processor, &tap), RotaryMode::Chorale);

        // Held: fast at once, and stopped once a second has gone by.
        assert_eq!(
            play(&mut processor, &[controller(0, 64, 127)]),
            RotaryMode::Tremolo
        );
        let second = processor.hold_frames.div_ceil(BLOCK);
        for _ in 1..second - 1 {
            assert_eq!(play(&mut processor, &[]), RotaryMode::Tremolo);
        }
        assert_eq!(play(&mut processor, &[]), RotaryMode::Brake);
        assert_eq!(
            play(&mut processor, &[controller(0, 64, 0)]),
            RotaryMode::Brake
        );
        // The press that became a stop did not change the speed.
        assert_eq!(play(&mut processor, &tap), RotaryMode::Chorale);
    }

    #[test]
    fn the_sustain_pedal_can_be_momentary_or_ignored() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.load_preset("chorale-888"));
        assert!(processor.prepare(48_000.0, 64, 0, 2));
        let mut output = [0.0_f32; 128];
        let mode = |processor: &RfOrganProcessor| {
            processor
                .engine
                .as_ref()
                .expect("prepared engine")
                .rotary_mode()
        };

        assert!(processor.set_parameter(settings::SUSTAIN_PEDAL, 2.0));
        processor.process(&[], &mut output, &[controller(0, 64, 127)], &[], 64, 0, 2);
        assert_eq!(mode(&processor), RotaryMode::Tremolo);
        processor.process(&[], &mut output, &[controller(0, 64, 0)], &[], 64, 0, 2);
        assert_eq!(mode(&processor), RotaryMode::Chorale);

        assert!(processor.set_parameter(settings::SUSTAIN_PEDAL, 0.0));
        processor.process(&[], &mut output, &[controller(0, 64, 127)], &[], 64, 0, 2);
        assert_eq!(mode(&processor), RotaryMode::Chorale);

        // A program with the cabinet switched out is not switched in by it.
        assert!(processor.load_preset("straight-888"));
        assert!(processor.set_parameter(settings::SUSTAIN_PEDAL, 1.0));
        let tap = [controller(0, 64, 0), controller(8, 64, 127)];
        processor.process(&[], &mut output, &tap, &[], 64, 0, 2);
        assert_eq!(mode(&processor), RotaryMode::Off);
    }

    /// Moving a drawbar is not a request to set the rest of the console
    /// again. Before this, any parameter put the expression and the cabinet
    /// speed back to what the parameters held, so a pedal on CC 11 jumped
    /// open and a speed thrown on CC 1 fell back whenever a control moved.
    #[test]
    fn a_control_moved_leaves_what_midi_set_alone() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.load_preset("chorale-888"));
        assert!(processor.prepare(48_000.0, 64, 0, 2));
        let mut output = [0.0_f32; 128];

        let midi = [controller(0, 11, 0), controller(0, 1, 127)];
        let drawbar = ParameterEvent {
            frame: 16,
            index: settings::DRAWBAR_FIRST,
            value: 4.0,
        };
        processor.process(&[], &mut output, &midi, &[drawbar], 64, 0, 2);

        assert_eq!(processor.settings.drawbars_b[0], 4);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.expression(), 0.0);
        assert_eq!(engine.rotary_mode(), RotaryMode::Tremolo);
    }

    /// The manifest tells the host what state format the plugin writes, and
    /// the plugin is what writes it. Nothing held those two together and they
    /// drifted two versions apart before this test existed, which the host
    /// would have had no way to notice.
    /// The package is conveyed on its own - downloaded, and compiled into the
    /// host's binary - so the licence and the notices have to be inside it
    /// rather than only in the repository it was built from. They are copies,
    /// and a copy nothing checks is a copy that goes stale.
    #[test]
    fn the_package_carries_the_licence_and_the_notices() {
        assert_eq!(
            include_str!("../../../package/LICENSE"),
            include_str!("../../../LICENSE")
        );
        assert_eq!(
            include_str!("../../../package/NOTICE.md"),
            include_str!("../../../THIRD_PARTY_NOTICES.md")
        );
    }

    #[test]
    fn the_manifest_agrees_with_the_state_the_plugin_writes() {
        let manifest = include_str!("../../../package/rackforge-plugin.toml");
        let declared = manifest
            .lines()
            .find_map(|line| line.strip_prefix("state_version = "))
            .expect("the manifest declares a state version")
            .trim()
            .parse::<u32>()
            .expect("a number");
        assert_eq!(declared, STATE_VERSION);
        // The runtime descriptor carries the same number a second time, and
        // the packer refuses a package where the two disagree - which is how
        // this copy was found, at the end of a release rather than here.
        let runtime = include_str!("../../../package/metadata/runtime.json");
        let described = runtime
            .lines()
            .find_map(|line| line.trim().strip_prefix("\"state_version\": "))
            .expect("the descriptor declares a state version")
            .trim_end_matches(',')
            .trim()
            .parse::<u32>()
            .expect("a number");
        assert_eq!(described, STATE_VERSION);
    }

    /// And the version it declares is the one being built.
    #[test]
    fn the_manifest_agrees_with_the_version_being_built() {
        let manifest = include_str!("../../../package/rackforge-plugin.toml");
        let declared = manifest
            .lines()
            .find_map(|line| line.strip_prefix("version = "))
            .expect("the manifest declares a version")
            .trim()
            .trim_matches('"');
        assert_eq!(declared, env!("CARGO_PKG_VERSION"));
    }

    fn note(status: u8, note: u8, velocity: u8) -> MidiEvent {
        MidiEvent {
            frame: 0,
            data: [status, note, velocity],
            length: 3,
        }
    }

    fn play(processor: &mut RfOrganProcessor, midi: &[MidiEvent]) -> f32 {
        let mut output = [0.0_f32; 512];
        processor.process(&[], &mut output, midi, &[], 256, 0, 2);
        output
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
    }

    fn prepared() -> RfOrganProcessor {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.prepare(48_000.0, 256, 0, 2));
        processor
    }

    /// The twelve reverse-colour keys sit in the octave below each manual,
    /// MIDI 24 to 35 in the order they are laid out, and a note there on a
    /// manual's channel presses that manual's key - reported back through the
    /// same parameter the drop-down sets, which is what the surface reads.
    #[test]
    fn the_bottom_octave_presses_the_twelve_preset_keys_of_its_manual() {
        for (offset, key) in Registration::KEYBOARD.into_iter().enumerate() {
            let note_number = PRESET_KEY_FIRST + offset as u8;
            for (status, index) in [(0x90, REGISTRATION), (0x91, LOWER_REGISTRATION)] {
                let mut processor = prepared();
                play(&mut processor, &[note(status, note_number, 100)]);
                assert_eq!(
                    processor.get_parameter(index),
                    Some(f64::from(key.index())),
                    "note {note_number} on status {status:#x} did not press {}",
                    key.label()
                );
            }
        }
    }

    /// Each manual has its own twelve: a key on one leaves the other alone.
    #[test]
    fn a_preset_key_presses_only_its_own_manual() {
        let mut processor = prepared();
        let lower_before = processor.get_parameter(LOWER_REGISTRATION);
        play(&mut processor, &[note(0x90, 25, 100)]);
        assert_eq!(
            processor.get_parameter(REGISTRATION),
            Some(f64::from(Registration::PresetCSharp.index()))
        );
        assert_eq!(processor.get_parameter(LOWER_REGISTRATION), lower_before);
    }

    /// "When a preset key is depressed it locks down and is released only
    /// when another is depressed." Letting go of the key on the controller
    /// does not let go of the registration - neither by a note-off nor by the
    /// note-on with no velocity that many controllers send instead.
    #[test]
    fn a_preset_key_stays_down_when_it_is_released() {
        let mut processor = prepared();
        let pressed = Some(f64::from(Registration::PresetCSharp.index()));
        play(&mut processor, &[note(0x90, 25, 100)]);
        play(&mut processor, &[note(0x80, 25, 0)]);
        assert_eq!(processor.get_parameter(REGISTRATION), pressed);
        play(&mut processor, &[note(0x90, 25, 0)]);
        assert_eq!(processor.get_parameter(REGISTRATION), pressed);
        // The next key is what releases it.
        play(&mut processor, &[note(0x90, 35, 100)]);
        assert_eq!(
            processor.get_parameter(REGISTRATION),
            Some(f64::from(Registration::AdjustB.index()))
        );
    }

    /// A release that was never pressed selects nothing.
    #[test]
    fn a_preset_key_released_without_being_pressed_selects_nothing() {
        let mut processor = prepared();
        let before = processor.get_parameter(REGISTRATION);
        play(&mut processor, &[note(0x80, 25, 0), note(0x90, 26, 0)]);
        assert_eq!(processor.get_parameter(REGISTRATION), before);
    }

    /// The pedal clavier has no preset keys, and its notes start at 24 on its
    /// own channel: a pedal there still sounds, and no registration moves.
    #[test]
    fn the_pedal_clavier_has_no_preset_keys() {
        let mut processor = prepared();
        let upper = processor.get_parameter(REGISTRATION);
        let lower = processor.get_parameter(LOWER_REGISTRATION);
        let mut peak = play(&mut processor, &[note(0x92, 24, 127)]);
        for _ in 0..8 {
            peak = peak.max(play(&mut processor, &[]));
        }
        assert!(peak > 1.0e-6, "the pedal at 24 did not sound");
        assert_eq!(processor.get_parameter(REGISTRATION), upper);
        assert_eq!(processor.get_parameter(LOWER_REGISTRATION), lower);
    }

    /// The playing keys above the preset octave still play.
    #[test]
    fn the_playing_keys_still_play() {
        let mut processor = prepared();
        let mut peak = play(&mut processor, &[note(0x90, 36, 110)]);
        for _ in 0..8 {
            peak = peak.max(play(&mut processor, &[]));
        }
        assert!(peak > 1.0e-6, "the lowest playing key did not sound");
    }

    /// The key chosen from the keyboard is the parameter's value, so a saved
    /// project keeps it.
    #[test]
    fn the_preset_key_chosen_from_the_keyboard_is_saved() {
        let mut processor = prepared();
        play(&mut processor, &[note(0x91, 30, 100)]);
        let mut state = [0_u8; 4096];
        let length = processor.save_state(&mut state).expect("state");
        let mut restored = prepared();
        assert!(restored.load_state(&state[..length]));
        assert_eq!(
            restored.get_parameter(LOWER_REGISTRATION),
            Some(f64::from(Registration::KEYBOARD[6].index()))
        );
    }

    /// The same keys answer a MIDI 2 controller, whose notes arrive on the
    /// other path.
    #[test]
    fn the_preset_keys_answer_midi_2_as_well() {
        let mut processor = prepared();
        let event = MidiEvent2 {
            frame: 0,
            kind: MIDI2_KIND_NOTE_ON,
            channel: 0,
            index: 33,
            flags: 0,
            value: 40_000,
            extra: 0,
        };
        let mut output = [0.0_f32; 512];
        processor.process_wide(&[], &mut output, &[], &[event], &[], 256, 0, 2);
        assert_eq!(
            processor.get_parameter(REGISTRATION),
            Some(f64::from(Registration::PresetA.index()))
        );
    }

    #[test]
    fn midi_channels_route_to_lower_manual_and_pedals() {
        let mut processor = RfOrganProcessor::default();
        assert!(processor.prepare(48_000.0, 256, 0, 2));
        let midi = [
            MidiEvent {
                frame: 0,
                data: [0x91, 60, 100],
                length: 3,
            },
            MidiEvent {
                frame: 0,
                data: [0x92, 24, 127],
                length: 3,
            },
        ];
        let mut output = [0.0_f32; 512];
        processor.process(&[], &mut output, &midi, &[], 256, 0, 2);
        assert!(output.iter().any(|sample| sample.abs() > 1.0e-6));
    }
}
