// SPDX-License-Identifier: GPL-2.0-or-later
use core::f32::consts::TAU;

const DELAY_CAPACITY: usize = 8192;

/// Rotor speeds, as the cabinet's own specification gives them. Hammond's
/// documented ranges for a digital Leslie are 20 to 120 rpm slow and 200 to
/// 500 rpm fast, and its worked example of a transition is 40 to 400 rpm.
const HORN_SLOW_RPM: f32 = 40.0;
const HORN_FAST_RPM: f32 = 400.0;
const DRUM_SLOW_RPM: f32 = 40.0;
const DRUM_FAST_RPM: f32 = 340.0;
/// Transition times in seconds at the middle of the acceleration control. A
/// rotor and its belt cannot move faster than Hammond's documented floor of
/// 0.8 s for the horn and 1.0 s for the drum, and the heavy drum takes several
/// times longer than the horn. The values themselves are provisional; the
/// laboratory measures what they produce.
const HORN_RISE_SECONDS: f32 = 1.0;
const HORN_FALL_SECONDS: f32 = 1.2;
const HORN_BRAKE_SECONDS: f32 = 1.0;
const DRUM_RISE_SECONDS: f32 = 2.6;
const DRUM_FALL_SECONDS: f32 = 3.6;
const DRUM_BRAKE_SECONDS: f32 = 2.4;
/// Documented floors and ceiling for those times.
const HORN_TIME_FLOOR: f32 = 0.8;
const DRUM_TIME_FLOOR: f32 = 1.0;
const TIME_CEILING: f32 = 12.5;
/// A mode switch reaches the motor through a relay and a clutch, so the rotor
/// does not begin to change speed at once. Hammond exposes this as 0 to 1 s.
const MODE_DELAY_SECONDS: f32 = 0.04;

const fn rpm_to_hz(rpm: f32) -> f32 {
    rpm / 60.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum LeslieMode {
    #[default]
    Off = 0,
    Brake = 1,
    Chorale = 2,
    Tremolo = 3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeslieDiagnostics {
    pub horn_speed_hz: f32,
    pub horn_target_hz: f32,
    pub drum_speed_hz: f32,
    pub drum_target_hz: f32,
    /// Revolutions per minute, the unit a cabinet is specified in.
    pub horn_speed_rpm: f32,
    pub drum_speed_rpm: f32,
    /// Seconds the rotor will take to cross its full speed range at the rate
    /// it is currently ramping, which is the transition time as Hammond
    /// defines it.
    pub horn_transition_seconds: f32,
    pub drum_transition_seconds: f32,
}

impl LeslieMode {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Off),
            1 => Some(Self::Brake),
            2 => Some(Self::Chorale),
            3 => Some(Self::Tremolo),
            _ => None,
        }
    }
}

struct DelayLine {
    data: [f32; DELAY_CAPACITY],
    write: usize,
}

impl DelayLine {
    const fn new() -> Self {
        Self {
            data: [0.0; DELAY_CAPACITY],
            write: 0,
        }
    }

    fn push(&mut self, value: f32) {
        self.data[self.write] = value;
        self.write = (self.write + 1) % DELAY_CAPACITY;
    }

    fn read(&self, delay: f32) -> f32 {
        let delay = delay.clamp(1.0, (DELAY_CAPACITY - 2) as f32);
        let whole = delay as usize;
        let fraction = delay - whole as f32;
        let newer = (self.write + DELAY_CAPACITY - whole - 1) % DELAY_CAPACITY;
        let older = (newer + DELAY_CAPACITY - 1) % DELAY_CAPACITY;
        self.data[newer] * (1.0 - fraction) + self.data[older] * fraction
    }

    fn clear(&mut self) {
        self.data.fill(0.0);
        self.write = 0;
    }
}

#[derive(Clone, Copy)]
struct Rotor {
    sine: f32,
    cosine: f32,
    speed_hz: f32,
    target_hz: f32,
    /// Hertz per second while ramping. Hammond defines a transition time as
    /// the time to cross the whole speed range, so a shorter move takes
    /// proportionally less time and the rate is what stays constant.
    rate_hz_per_second: f32,
    /// Samples still to wait before the speed starts changing.
    delay_frames: u32,
    slow_hz: f32,
    fast_hz: f32,
    rise_seconds: f32,
    fall_seconds: f32,
    brake_seconds: f32,
    direction: f32,
}

impl Rotor {
    const fn new(
        direction: f32,
        slow_rpm: f32,
        fast_rpm: f32,
        rise: f32,
        fall: f32,
        brake: f32,
    ) -> Self {
        Self {
            sine: 0.0,
            cosine: 1.0,
            speed_hz: 0.0,
            target_hz: 0.0,
            rate_hz_per_second: 0.0,
            delay_frames: 0,
            slow_hz: rpm_to_hz(slow_rpm),
            fast_hz: rpm_to_hz(fast_rpm),
            rise_seconds: rise,
            fall_seconds: fall,
            brake_seconds: brake,
            direction,
        }
    }

    /// Points the rotor at a new speed and works out how fast it may get
    /// there: up takes the rise time, down to the slow speed the fall time,
    /// and down to a stop the brake time.
    fn aim(&mut self, target_hz: f32, sample_rate: f32) {
        let seconds = if target_hz > self.speed_hz {
            self.rise_seconds
        } else if target_hz <= 0.0 {
            self.brake_seconds
        } else {
            self.fall_seconds
        };
        let span = if target_hz <= 0.0 {
            self.fast_hz
        } else {
            self.fast_hz - self.slow_hz
        };
        self.target_hz = target_hz;
        self.rate_hz_per_second = span / seconds.max(0.05);
        self.delay_frames = (MODE_DELAY_SECONDS * sample_rate) as u32;
    }

    /// Seconds this rotor would need to cross its whole range at the rate it
    /// is ramping at now.
    const fn transition_seconds(&self) -> f32 {
        if self.rate_hz_per_second <= 0.0 {
            0.0
        } else {
            (self.fast_hz - self.slow_hz) / self.rate_hz_per_second
        }
    }

    fn advance(&mut self, sample_rate: f32) {
        if self.delay_frames > 0 {
            self.delay_frames -= 1;
        } else {
            let step = self.rate_hz_per_second / sample_rate;
            if self.speed_hz < self.target_hz {
                self.speed_hz = (self.speed_hz + step).min(self.target_hz);
            } else {
                self.speed_hz = (self.speed_hz - step).max(self.target_hz);
            }
        }
        let angle = self.direction * TAU * self.speed_hz / sample_rate;
        let (rotation_sine, rotation_cosine) = small_rotation(angle);
        let sine = self.sine * rotation_cosine + self.cosine * rotation_sine;
        let cosine = self.cosine * rotation_cosine - self.sine * rotation_sine;
        let correction = 1.5 - 0.5 * (sine * sine + cosine * cosine);
        self.sine = sine * correction;
        self.cosine = cosine * correction;
    }
}

pub struct Leslie {
    sample_rate: f32,
    mode: LeslieMode,
    mix: f32,
    crossover: f32,
    low_state: f32,
    horn: Rotor,
    drum: Rotor,
    horn_delay: DelayLine,
    drum_delay: DelayLine,
    cabinet_left: DelayLine,
    cabinet_right: DelayLine,
    horn_tone_left: f32,
    horn_tone_right: f32,
    mic_distance: f32,
    stereo_width: f32,
    reflections: f32,
    horn_drum_balance: f32,
}

impl Leslie {
    pub fn new(sample_rate: f32) -> Self {
        let x = TAU * 800.0 / sample_rate;
        Self {
            sample_rate,
            mode: LeslieMode::Off,
            mix: 0.82,
            crossover: x / (1.0 + x),
            low_state: 0.0,
            // A cabinet turns its horn counter-clockwise and its drum
            // clockwise, which is why they sweep past the microphones in
            // opposite directions.
            horn: Rotor::new(
                1.0,
                HORN_SLOW_RPM,
                HORN_FAST_RPM,
                HORN_RISE_SECONDS,
                HORN_FALL_SECONDS,
                HORN_BRAKE_SECONDS,
            ),
            drum: Rotor::new(
                -1.0,
                DRUM_SLOW_RPM,
                DRUM_FAST_RPM,
                DRUM_RISE_SECONDS,
                DRUM_FALL_SECONDS,
                DRUM_BRAKE_SECONDS,
            ),
            horn_delay: DelayLine::new(),
            drum_delay: DelayLine::new(),
            cabinet_left: DelayLine::new(),
            cabinet_right: DelayLine::new(),
            horn_tone_left: 0.0,
            horn_tone_right: 0.0,
            mic_distance: 0.35,
            stereo_width: 0.75,
            reflections: 0.22,
            horn_drum_balance: 0.0,
        }
    }

    pub fn set_mode(&mut self, mode: LeslieMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        let (horn, drum) = match mode {
            LeslieMode::Off | LeslieMode::Brake => (0.0, 0.0),
            LeslieMode::Chorale => (self.horn.slow_hz, self.drum.slow_hz),
            LeslieMode::Tremolo => (self.horn.fast_hz, self.drum.fast_hz),
        };
        self.horn.aim(horn, self.sample_rate);
        self.drum.aim(drum, self.sample_rate);
    }

    pub const fn mode(&self) -> LeslieMode {
        self.mode
    }

    pub fn set_mix(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        self.mix = value;
        true
    }

    /// Scales the six transition times together, from brisk at zero to
    /// sluggish at one, never past the floors and ceiling Hammond documents.
    pub fn set_acceleration(&mut self, value: f32) -> bool {
        if !unit(value) {
            return false;
        }
        // The middle of the control is where the documented times sit.
        let scale = 0.55 + 1.8 * value * value;
        let horn = |seconds: f32| (seconds * scale).clamp(HORN_TIME_FLOOR, TIME_CEILING);
        let drum = |seconds: f32| (seconds * scale).clamp(DRUM_TIME_FLOOR, TIME_CEILING);
        self.horn.rise_seconds = horn(HORN_RISE_SECONDS);
        self.horn.fall_seconds = horn(HORN_FALL_SECONDS);
        self.horn.brake_seconds = horn(HORN_BRAKE_SECONDS);
        self.drum.rise_seconds = drum(DRUM_RISE_SECONDS);
        self.drum.fall_seconds = drum(DRUM_FALL_SECONDS);
        self.drum.brake_seconds = drum(DRUM_BRAKE_SECONDS);
        // Keep whatever move is under way consistent with the new times.
        let (horn_target, drum_target) = (self.horn.target_hz, self.drum.target_hz);
        let (horn_delay, drum_delay) = (self.horn.delay_frames, self.drum.delay_frames);
        self.horn.aim(horn_target, self.sample_rate);
        self.drum.aim(drum_target, self.sample_rate);
        self.horn.delay_frames = horn_delay;
        self.drum.delay_frames = drum_delay;
        true
    }

    pub fn set_cabinet(
        &mut self,
        mic_distance: f32,
        stereo_width: f32,
        reflections: f32,
        horn_drum_balance: f32,
    ) -> bool {
        if !unit(mic_distance)
            || !unit(stereo_width)
            || !unit(reflections)
            || !bipolar(horn_drum_balance)
        {
            return false;
        }
        self.mic_distance = mic_distance;
        self.stereo_width = stereo_width;
        self.reflections = reflections;
        self.horn_drum_balance = horn_drum_balance;
        true
    }

    pub fn process(&mut self, input: f32) -> [f32; 2] {
        self.low_state += self.crossover * (input - self.low_state);
        let low = self.low_state;
        let high = input - low;
        self.horn_delay.push(high);
        self.drum_delay.push(low);

        self.horn.advance(self.sample_rate);
        self.drum.advance(self.sample_rate);

        let horn_base = self.sample_rate * (0.00075 + 0.0015 * self.mic_distance);
        let horn_depth = self.sample_rate * 0.00042 * (1.0 - 0.55 * self.mic_distance);
        let drum_base = self.sample_rate * (0.0011 + 0.0020 * self.mic_distance);
        let drum_depth = self.sample_rate * 0.00017 * (1.0 - 0.5 * self.mic_distance);
        let raw_horn_left = self
            .horn_delay
            .read(horn_base + horn_depth * self.horn.sine);
        let raw_horn_right = self
            .horn_delay
            .read(horn_base - horn_depth * self.horn.sine);
        let drum_left = self
            .drum_delay
            .read(drum_base + drum_depth * self.drum.sine);
        let drum_right = self
            .drum_delay
            .read(drum_base - drum_depth * self.drum.sine);

        let horn_filter = one_pole(2_200.0, self.sample_rate);
        self.horn_tone_left += horn_filter * (raw_horn_left - self.horn_tone_left);
        self.horn_tone_right += horn_filter * (raw_horn_right - self.horn_tone_right);
        let horn_high_left = raw_horn_left - self.horn_tone_left;
        let horn_high_right = raw_horn_right - self.horn_tone_right;
        let horn_facing_left = 0.5 + 0.5 * self.horn.cosine;
        let horn_facing_right = 1.0 - horn_facing_left;
        let horn_left = self.horn_tone_left * (0.55 + 0.35 * horn_facing_left)
            + horn_high_left * (0.20 + 0.80 * horn_facing_left);
        let horn_right = self.horn_tone_right * (0.55 + 0.35 * horn_facing_right)
            + horn_high_right * (0.20 + 0.80 * horn_facing_right);

        let drum_facing_left = 0.5 + 0.5 * self.drum.cosine;
        let drum_facing_right = 1.0 - drum_facing_left;
        let horn_gain = if self.horn_drum_balance < 0.0 {
            1.0 + self.horn_drum_balance
        } else {
            1.0
        };
        let drum_gain = if self.horn_drum_balance > 0.0 {
            1.0 - self.horn_drum_balance
        } else {
            1.0
        };
        let mut wet_left =
            horn_gain * horn_left + drum_gain * drum_left * (0.66 + 0.34 * drum_facing_left);
        let mut wet_right =
            horn_gain * horn_right + drum_gain * drum_right * (0.66 + 0.34 * drum_facing_right);

        let mid = 0.5 * (wet_left + wet_right);
        let side = 0.5 * (wet_left - wet_right) * (0.35 + 1.3 * self.stereo_width);
        wet_left = mid + side;
        wet_right = mid - side;

        self.cabinet_left.push(wet_left);
        self.cabinet_right.push(wet_right);
        let reflection_left = 0.42 * self.cabinet_left.read(self.sample_rate * 0.0037)
            + 0.28 * self.cabinet_right.read(self.sample_rate * 0.0063)
            - 0.18 * self.cabinet_left.read(self.sample_rate * 0.0109);
        let reflection_right = 0.42 * self.cabinet_right.read(self.sample_rate * 0.0041)
            + 0.28 * self.cabinet_left.read(self.sample_rate * 0.0069)
            - 0.18 * self.cabinet_right.read(self.sample_rate * 0.0117);
        wet_left += 0.65 * self.reflections * reflection_left;
        wet_right += 0.65 * self.reflections * reflection_right;
        let wet = if self.mode == LeslieMode::Off {
            0.0
        } else {
            self.mix
        };
        [
            input * (1.0 - wet) + wet_left * wet,
            input * (1.0 - wet) + wet_right * wet,
        ]
    }

    /// Read-only mechanical state for deterministic calibration tools.
    pub const fn diagnostics(&self) -> LeslieDiagnostics {
        LeslieDiagnostics {
            horn_speed_hz: self.horn.speed_hz,
            horn_target_hz: self.horn.target_hz,
            drum_speed_hz: self.drum.speed_hz,
            drum_target_hz: self.drum.target_hz,
            horn_speed_rpm: self.horn.speed_hz * 60.0,
            drum_speed_rpm: self.drum.speed_hz * 60.0,
            horn_transition_seconds: self.horn.transition_seconds(),
            drum_transition_seconds: self.drum.transition_seconds(),
        }
    }

    pub fn reset(&mut self) {
        self.low_state = 0.0;
        self.horn_delay.clear();
        self.drum_delay.clear();
        self.cabinet_left.clear();
        self.cabinet_right.clear();
        self.horn_tone_left = 0.0;
        self.horn_tone_right = 0.0;
    }
}

fn small_rotation(angle: f32) -> (f32, f32) {
    let squared = angle * angle;
    (
        angle * (1.0 - squared / 6.0),
        1.0 - squared / 2.0 * (1.0 - squared / 12.0),
    )
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn bipolar(value: f32) -> bool {
    value.is_finite() && (-1.0..=1.0).contains(&value)
}

fn one_pole(frequency: f32, sample_rate: f32) -> f32 {
    let normalized = TAU * frequency / sample_rate;
    normalized / (1.0 + normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_is_an_exact_bypass_while_rotors_keep_state() {
        let mut leslie = Leslie::new(48_000.0);
        for index in 0..1024 {
            let input = index as f32 / 1024.0 - 0.5;
            assert_eq!(leslie.process(input), [input, input]);
        }
    }

    #[test]
    fn cabinet_reflections_create_a_distinct_tail() {
        let mut dry_cabinet = Leslie::new(48_000.0);
        let mut live_cabinet = Leslie::new(48_000.0);
        dry_cabinet.set_mode(LeslieMode::Brake);
        live_cabinet.set_mode(LeslieMode::Brake);
        assert!(dry_cabinet.set_mix(1.0));
        assert!(live_cabinet.set_mix(1.0));
        assert!(dry_cabinet.set_cabinet(0.35, 0.75, 0.0, 0.0));
        assert!(live_cabinet.set_cabinet(0.35, 0.75, 1.0, 0.0));
        let mut difference = 0.0;
        for index in 0..2048 {
            let input = if index == 0 { 1.0 } else { 0.0 };
            let dry = dry_cabinet.process(input);
            let live = live_cabinet.process(input);
            if index > 256 {
                difference += (dry[0] - live[0]).abs() + (dry[1] - live[1]).abs();
            }
        }
        assert!(difference > 0.01);
    }

    #[test]
    fn diagnostics_report_rotor_targets_and_motion() {
        let mut leslie = Leslie::new(48_000.0);
        leslie.set_mode(LeslieMode::Tremolo);
        let initial = leslie.diagnostics();
        // 400 rpm on the horn and 340 on the drum, as a cabinet is specified.
        assert!((initial.horn_target_hz * 60.0 - 400.0).abs() < 0.01);
        assert!((initial.drum_target_hz * 60.0 - 340.0).abs() < 0.01);
        assert_eq!(initial.horn_speed_hz, 0.0);
        for _ in 0..48_000 {
            leslie.process(0.0);
        }
        let moving = leslie.diagnostics();
        assert!(moving.horn_speed_rpm > moving.drum_speed_rpm);
        assert!(moving.horn_speed_hz <= moving.horn_target_hz);
        assert!(moving.horn_transition_seconds >= 0.8);
        assert!(moving.drum_transition_seconds >= 1.0);
    }

    /// Seconds a rotor needs to reach a speed, sampled at the frame it
    /// arrives.
    fn settle_seconds(leslie: &mut Leslie, horn: bool) -> f32 {
        for frame in 0..48_000 * 20 {
            leslie.process(0.0);
            let state = leslie.diagnostics();
            let (speed, target) = if horn {
                (state.horn_speed_hz, state.horn_target_hz)
            } else {
                (state.drum_speed_hz, state.drum_target_hz)
            };
            if (speed - target).abs() < 1.0e-6 {
                return frame as f32 / 48_000.0;
            }
        }
        f32::MAX
    }

    /// Hammond defines a transition time as the time to cross the whole speed
    /// range, so the rate is what stays fixed: braking from a standstill at
    /// the slow speed takes a fraction of the time that braking from fast
    /// does, in proportion to the speed given up.
    #[test]
    fn transitions_run_at_a_constant_rate() {
        let mut leslie = Leslie::new(48_000.0);
        assert!(leslie.set_acceleration(0.5));
        // From the slow speed, so the rotor crosses exactly the span the
        // transition time is defined over.
        leslie.set_mode(LeslieMode::Chorale);
        settle_seconds(&mut leslie, true);
        leslie.set_mode(LeslieMode::Tremolo);
        let rise = settle_seconds(&mut leslie, true);
        assert!(
            (rise - HORN_RISE_SECONDS - MODE_DELAY_SECONDS).abs() < 0.1,
            "horn rise took {rise} s"
        );

        leslie.set_mode(LeslieMode::Chorale);
        let fall = settle_seconds(&mut leslie, true);
        assert!(
            (fall - HORN_FALL_SECONDS - MODE_DELAY_SECONDS).abs() < 0.1,
            "horn fall took {fall} s"
        );

        // From the slow speed a stop gives up a tenth of the range, so it
        // takes about a tenth of the brake time.
        leslie.set_mode(LeslieMode::Brake);
        let brake = settle_seconds(&mut leslie, true);
        let expected = HORN_BRAKE_SECONDS * HORN_SLOW_RPM / HORN_FAST_RPM + MODE_DELAY_SECONDS;
        assert!(
            (brake - expected).abs() < 0.1,
            "braking from slow took {brake} s against {expected}"
        );
    }

    /// The drum is heavier than the horn and Hammond floors both, so no
    /// setting of the control can make either move faster than a cabinet can.
    #[test]
    fn transition_times_stay_inside_the_documented_range() {
        let mut leslie = Leslie::new(48_000.0);
        for step in 0..=10 {
            assert!(leslie.set_acceleration(step as f32 / 10.0));
            leslie.set_mode(LeslieMode::Chorale);
            leslie.set_mode(LeslieMode::Tremolo);
            let state = leslie.diagnostics();
            assert!(
                (HORN_TIME_FLOOR..=TIME_CEILING).contains(&state.horn_transition_seconds),
                "horn at {}",
                state.horn_transition_seconds
            );
            assert!(
                (DRUM_TIME_FLOOR..=TIME_CEILING).contains(&state.drum_transition_seconds),
                "drum at {}",
                state.drum_transition_seconds
            );
            assert!(state.drum_transition_seconds > state.horn_transition_seconds);
        }
    }
}
