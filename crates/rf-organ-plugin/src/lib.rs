// SPDX-License-Identifier: GPL-2.0-or-later
//! RackForge adapter for the RF-Organ physical-model instrument.

mod settings;

use rackforge_plugin_sdk::{
    MIDI_FAMILY_CONTROL, MIDI_FAMILY_NOTE, MIDI2_FLAG_ORIGIN_7BIT, MIDI2_KIND_CONTROL_CHANGE,
    MIDI2_KIND_NOTE_OFF, MIDI2_KIND_NOTE_ON, MidiEvent, MidiEvent2, ParameterEvent, Processor,
    export_processor,
};
use rf_organ_dsp::{LeslieMode, MIC_DISTANCE_RANGE_M, MIC_SPACING_MAX_M, OrganEngine, OrganPart};
pub use settings::{PARAMETER_COUNT, Settings, presets};

pub const MAX_FRAMES: u32 = 4096;
pub const MAX_EVENTS: usize = 256;
pub const STATE_VERSION: u32 = 7;
pub const STATE_BYTES_V1: usize = 8 + 19 * 8;
pub const STATE_BYTES_V2: usize = 8 + 24 * 8;
pub const STATE_BYTES_V3: usize = 8 + 37 * 8;
pub const STATE_BYTES_V4: usize = 8 + 41 * 8;
pub const STATE_BYTES_V5: usize = 8 + 45 * 8;
pub const STATE_BYTES_V6: usize = 8 + 51 * 8;
pub const STATE_BYTES: usize = 8 + PARAMETER_COUNT * 8;

#[derive(Default)]
pub struct RfOrganProcessor {
    engine: Option<Box<OrganEngine>>,
    settings: Settings,
    maximum_frames: u32,
    channels: u32,
}

impl RfOrganProcessor {
    fn apply_settings(&mut self, settings: Settings) -> bool {
        if !settings.valid() {
            return false;
        }
        self.settings = settings;
        if let Some(engine) = &mut self.engine {
            settings.apply(engine);
        }
        true
    }

    fn midi1(&mut self, event: &MidiEvent) {
        let Some(engine) = &mut self.engine else {
            return;
        };
        let [status, index, value] = event.data;
        let part = part_from_channel(status & 0x0f);
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
                1 => engine.set_leslie_mode(if value < 64 {
                    LeslieMode::Chorale
                } else {
                    LeslieMode::Tremolo
                }),
                // Rotor stop, the third position a cabinet's switch has.
                // Releasing it returns the rotors to the selected speed.
                4 => engine.set_leslie_mode(if value < 64 {
                    LeslieMode::Brake
                } else {
                    self.settings.leslie_mode
                }),
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
        let Some(engine) = &mut self.engine else {
            return;
        };
        let part = part_from_channel(event.channel);
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
                    1 => engine.set_leslie_mode(if value < 0.5 {
                        LeslieMode::Chorale
                    } else {
                        LeslieMode::Tremolo
                    }),
                    4 => engine.set_leslie_mode(if value < 0.5 {
                        LeslieMode::Brake
                    } else {
                        self.settings.leslie_mode
                    }),
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
        self.maximum_frames = frames;
        self.channels = outputs;
        true
    }

    fn set_parameter(&mut self, index: u32, value: f64) -> bool {
        let Some(settings) = self.settings.with_parameter(index, value) else {
            return false;
        };
        self.apply_settings(settings)
    }

    fn get_parameter(&self, index: u32) -> Option<f64> {
        self.settings.parameter(index)
    }

    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.reset();
            self.settings.apply(engine);
        }
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
        if ![
            STATE_BYTES_V1,
            STATE_BYTES_V2,
            STATE_BYTES_V3,
            STATE_BYTES_V4,
            STATE_BYTES_V5,
            STATE_BYTES_V6,
            STATE_BYTES,
        ]
        .contains(&state.len())
            || &state[..4] != b"RFOR"
        {
            return false;
        }
        let version = u32::from_le_bytes(state[4..8].try_into().expect("validated state"));
        let fields = match (version, state.len()) {
            (1, STATE_BYTES_V1) => 19,
            (2, STATE_BYTES_V2) => 24,
            (3, STATE_BYTES_V3) => 37,
            (4, STATE_BYTES_V4) => 41,
            (5, STATE_BYTES_V5) => 45,
            (6, STATE_BYTES_V6) => 51,
            (STATE_VERSION, STATE_BYTES) => PARAMETER_COUNT,
            _ => return false,
        };
        if fields > PARAMETER_COUNT {
            return false;
        }
        let mut settings = Settings::default();
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
                _ => value,
            };
            let Some(updated) = settings.with_parameter(index, value) else {
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

    #[test]
    fn legacy_states_migrate_with_new_controls_at_defaults() {
        let source = RfOrganProcessor::default();
        let mut current = [0_u8; STATE_BYTES];
        assert_eq!(source.save_state(&mut current), Some(STATE_BYTES));
        let mut legacy = [0_u8; STATE_BYTES_V1];
        legacy.copy_from_slice(&current[..STATE_BYTES_V1]);
        legacy[4..8].copy_from_slice(&1_u32.to_le_bytes());
        let mut restored = RfOrganProcessor::default();
        assert!(restored.load_state(&legacy));
        assert_eq!(restored.settings, Settings::default());

        let mut version_two = [0_u8; STATE_BYTES_V2];
        version_two.copy_from_slice(&current[..STATE_BYTES_V2]);
        version_two[4..8].copy_from_slice(&2_u32.to_le_bytes());
        assert!(restored.load_state(&version_two));
        assert_eq!(restored.settings, Settings::default());

        let mut version_three = [0_u8; STATE_BYTES_V3];
        version_three.copy_from_slice(&current[..STATE_BYTES_V3]);
        version_three[4..8].copy_from_slice(&3_u32.to_le_bytes());
        assert!(restored.load_state(&version_three));
        assert_eq!(restored.settings, Settings::default());

        let mut version_four = [0_u8; STATE_BYTES_V4];
        version_four.copy_from_slice(&current[..STATE_BYTES_V4]);
        version_four[4..8].copy_from_slice(&4_u32.to_le_bytes());
        assert!(restored.load_state(&version_four));
        assert_eq!(restored.settings, Settings::default());

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
        let migrated = Settings {
            leslie_mic_distance: f64::from(near) + 0.35 * f64::from(far - near),
            leslie_mic_spacing: 0.75 * f64::from(MIC_SPACING_MAX_M),
            ..Settings::default()
        };

        let mut version_five = [0_u8; STATE_BYTES_V5];
        version_five.copy_from_slice(&current[..STATE_BYTES_V5]);
        version_five[4..8].copy_from_slice(&5_u32.to_le_bytes());
        place_in_units(&mut version_five);
        assert!(restored.load_state(&version_five));
        assert_eq!(restored.settings, migrated);
        assert_eq!(restored.settings.transformer_trims, [[0.0; 2]; 3]);

        let mut version_six = [0_u8; STATE_BYTES_V6];
        version_six.copy_from_slice(&current[..STATE_BYTES_V6]);
        version_six[4..8].copy_from_slice(&6_u32.to_le_bytes());
        place_in_units(&mut version_six);
        assert!(restored.load_state(&version_six));
        assert_eq!(restored.settings, migrated);
        assert_eq!(restored.settings.leslie_mic_offset, 0.0);
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
        assert_eq!(processor.settings.leslie_mode, LeslieMode::Chorale);

        let mut output = [0.0_f32; 128];
        let brake = [MidiEvent {
            frame: 0,
            data: [0xb0, 4, 0],
            length: 3,
        }];
        processor.process(&[], &mut output, &brake, &[], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.leslie_mode(), LeslieMode::Brake);

        let release = [MidiEvent {
            frame: 0,
            data: [0xb0, 4, 127],
            length: 3,
        }];
        processor.process(&[], &mut output, &release, &[], 64, 0, 2);
        let engine = processor.engine.as_ref().expect("prepared engine");
        assert_eq!(engine.leslie_mode(), LeslieMode::Chorale);
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
