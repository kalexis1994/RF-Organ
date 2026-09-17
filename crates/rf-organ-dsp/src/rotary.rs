// SPDX-License-Identifier: GPL-2.0-or-later
use core::f32::consts::TAU;

const DELAY_CAPACITY: usize = 8192;

/// Rotor speeds, as the cabinet's own specification gives them. Hammond's
/// documented ranges for a digital rotating cabinet are 20 to 120 rpm slow
/// and 200 to 500 rpm fast, and its worked example of a transition is 40 to
/// 400 rpm.
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

/// Below this the rotor is treated as already at rest, and a stop has nothing
/// left to aim.
const RESTING_HZ: f32 = 1.0e-4;

/// The supply the cabinet's motors are running from.
///
/// A rotor is belted to an alternating-current motor, and such a motor turns
/// at the supply's frequency divided by its pole pairs. The speeds quoted for
/// a cabinet are the ones its motors reach on the supply it was built for, so
/// the same cabinet on a different supply turns in that proportion: a
/// sixty-cycle cabinet plugged into fifty cycles runs at five sixths of every
/// speed it is rated for, which is a little under three semitones less
/// modulation and a noticeably lazier tremolo.
///
/// This models one cabinet on either supply, not the two cabinets a
/// manufacturer would sell into the two markets: an exported one was re-belted
/// or re-motored to reach its rated speeds there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum MainsFrequency {
    Fifty = 0,
    #[default]
    Sixty = 1,
}

impl MainsFrequency {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Fifty),
            1 => Some(Self::Sixty),
            _ => None,
        }
    }

    pub const fn hertz(self) -> f32 {
        match self {
            Self::Fifty => 50.0,
            Self::Sixty => 60.0,
        }
    }

    /// What the rated speeds become on this supply.
    fn scale(self) -> f32 {
        self.hertz() / RATED_MAINS_HZ
    }
}

/// The supply the quoted speeds belong to.
const RATED_MAINS_HZ: f32 = 60.0;

/// What the cabinet's microphones are.
///
/// Hammond offers two and describes them by what they do rather than by any
/// response: a dynamic one "enhances the sense of perspective", a condenser
/// one has a "natural" character. The part of that difference which is not a
/// matter of taste is in [`MicrophonePath`]: a directional capsule lifts the
/// bass as it approaches a source, and how much it lifts follows from the
/// distance and the pattern. What is left over - a presence lift and a top
/// that gives out earlier - is the character of a particular capsule, and no
/// capsule is named, so those two numbers are provisional.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum MicrophoneType {
    Dynamic = 0,
    #[default]
    Condenser = 1,
}

impl MicrophoneType {
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Dynamic),
            1 => Some(Self::Condenser),
            _ => None,
        }
    }
}

/// The presence a dynamic capsule adds and the top it gives up, both
/// provisional.
const DYNAMIC_PRESENCE_HZ: f32 = 3_000.0;
const DYNAMIC_PRESENCE_LIFT: f32 = 0.4;
const DYNAMIC_TOP_HZ: f32 = 11_000.0;
/// The most bass a capsule may gain by being close, which stands in for the
/// roll-off every real capsule has under it.
const PROXIMITY_MAX_LIFT: f32 = 0.8;

/// How loud each of the three microphone channels may be.
///
/// Hammond gives all three as decibels from unity down through -76 to
/// silence: one for the horn's pair, one for the drum's pair, and one for the
/// woofer's own sound underneath them. Where that last one sits by default is
/// not a number it publishes, so the default here is provisional; the other
/// two start at unity, which is where a volume with nothing to say sits.
pub const LEVEL_RANGE_DB: (f32, f32) = (-76.0, 0.0);
pub const LEVEL_SILENT_DB: f32 = -80.0;
pub const SUB_LEVEL_DEFAULT_DB: f32 = -9.0;
/// The woofer's sound reaches the drum's pair of microphones and, less of it,
/// the horn's. Hammond says which one hears it and which one barely does, not
/// by how much, so the ratio is provisional.
const SUB_INTO_DRUM: f32 = 1.0;
const SUB_INTO_HORN: f32 = 0.25;

/// Decibels to a gain, with the bottom of the range meaning silence.
fn from_decibels(decibels: f32) -> f32 {
    if decibels <= LEVEL_SILENT_DB {
        return 0.0;
    }
    // exp10(db / 20) without a library: 10^x is e^(x ln 10).
    exp_approx(decibels * (core::f32::consts::LN_10 / 20.0))
}

/// e to the power of a modest negative number, by squaring a short series.
/// The levels this serves change only when somebody moves a control.
fn exp_approx(power: f32) -> f32 {
    let halvings = 8;
    let mut term = power / (1 << halvings) as f32;
    term = 1.0 + term * (1.0 + 0.5 * term * (1.0 + term / 3.0));
    for _ in 0..halvings {
        term *= term;
    }
    term
}

/// Where a rotor is asked to come to rest. Hammond documents the setting as
/// 0 to 359 degrees or "Rnd", which stops it at a random angle. Zero is the
/// mouth pointing at the microphones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StopAngle {
    /// Turns from the microphones, in `[0, 1)`.
    Fixed(f32),
    Random,
}

impl StopAngle {
    /// Hammond's own encoding: a degree from 0 to 359, and one step past the
    /// end for "Rnd".
    pub fn from_degrees(degrees: f32) -> Option<Self> {
        if !degrees.is_finite() || degrees < 0.0 || degrees > 360.0 {
            return None;
        }
        if degrees >= 360.0 {
            Some(Self::Random)
        } else {
            Some(Self::Fixed(degrees / 360.0))
        }
    }
}

/// Fraction of a turn, from a number of turns that may be negative or large.
fn wrap_turn(turns: f32) -> f32 {
    let whole = (turns as i32) as f32;
    let fraction = turns - whole;
    if fraction < 0.0 {
        fraction + 1.0
    } else {
        fraction
    }
}

/// A stop that has to end at a chosen angle.
///
/// A constant rate cannot do that: the angle a rotor covers while slowing at a
/// fixed rate is whatever its speed happens to make it, and Hammond documents
/// both the time the stop takes and where it ends. What is left free is the
/// shape. The speed still falls from where it was to nothing over the same
/// time, but along a curve whose area is the angle that lands the mouth where
/// it was asked to stop.
///
/// The curve is `start * (1 - s) * (1 + bend * s)`, which leaves the speed
/// positive and falling for any bend in `[-1, 1]`, and covers between a third
/// and two thirds of `start * seconds`. A straight ramp is the half-way case.
/// Only when a whole turn still will not fit into that window does the stop
/// take longer than the documented time.
#[derive(Clone, Copy)]
struct Brake {
    start_hz: f32,
    frames: f32,
    elapsed: f32,
    bend: f32,
    /// Resolved once, so that a random angle stays put if the stop is
    /// re-shaped while it is under way.
    target: f32,
}

/// The travel a brake of this length can be shaped to cover, in turns.
fn brake_reach(speed_hz: f32, seconds: f32) -> (f32, f32) {
    let sweep = speed_hz * seconds;
    (sweep / 3.0, 2.0 * sweep / 3.0)
}

/// Radius at which each rotor's mouth travels, in metres. Smith, Serafin,
/// Abel and Berners model the horn as an omnidirectional source on a circle
/// of radius r, whose far-field Doppler amplitude is r times angular velocity
/// over the speed of sound; a diffuser in the horn is what makes the
/// omnidirectional assumption fair. These two are the only quantity in the
/// cabinet's geometry we could not find published, so they are defaults for a
/// pair of controls rather than constants, and the laboratory reports the
/// pitch deviation they produce against the deviation the geometry predicts.
pub const HORN_RADIUS_DEFAULT_M: f32 = 0.18;
pub const DRUM_RADIUS_DEFAULT_M: f32 = 0.12;
pub const HORN_RADIUS_RANGE_M: (f32, f32) = (0.05, 0.40);
pub const DRUM_RADIUS_RANGE_M: (f32, f32) = (0.05, 0.30);
const SOUND_SPEED_M_PER_S: f32 = 343.0;
/// Microphone placement, in the units and ranges Hammond documents: up to
/// 170 cm in front of the cabinet, up to 40 cm between the pair, and up to
/// 50 cm of offset between the centre of the pair and the rotor's pivot. The
/// near limit is ours: a microphone cannot be put inside the cabinet.
pub const MIC_DISTANCE_RANGE_M: (f32, f32) = (0.12, 1.7);
pub const MIC_SPACING_MAX_M: f32 = 0.4;
pub const MIC_OFFSET_MAX_M: f32 = 0.5;

/// Shortest path a rotor's mouth can be from a microphone, squared. The
/// microphones cannot be put inside the cabinet, so no path is ever this
/// short; it only keeps the arithmetic away from zero.
const NEAREST_SQUARED_M2: f32 = 4.0e-4;

/// Reciprocal square root, from the usual bit-pattern seed refined by three
/// Newton steps.
///
/// A rotor needs four path lengths every sample and `core` has no square root
/// of its own, so this way each one costs multiplications and no divisions.
/// The reciprocal is what the level and the direction want anyway, and the
/// length is the square times it. A test holds the result against the real
/// square root over every distance a cabinet can produce.
fn inverse_root(square: f32) -> f32 {
    let mut root = f32::from_bits(0x5f37_59df - (square.to_bits() >> 1));
    let half = 0.5 * square;
    root *= 1.5 - half * root * root;
    root *= 1.5 - half * root * root;
    root *= 1.5 - half * root * root;
    root
}

/// One microphone on its stand: where it is, and where it points.
///
/// The pair stands in front of the cabinet and aims at the rotor's pivot, so
/// the axis only has to be worked out when somebody moves a stand.
#[derive(Clone, Copy)]
struct Placement {
    distance: f32,
    lateral: f32,
    /// Unit vector from the microphone toward the pivot.
    axis: (f32, f32),
}

impl Placement {
    fn new(distance: f32, lateral: f32) -> Self {
        let inverse =
            inverse_root((distance * distance + lateral * lateral).max(NEAREST_SQUARED_M2));
        Self {
            distance,
            lateral,
            axis: (distance * inverse, lateral * inverse),
        }
    }
}

/// Distance from a rotor's mouth to one microphone, and the level that
/// distance gives it.
///
/// The mouth is at `radius` on a circle around the pivot and the straight line
/// between the two is what the sound travels: the delay is its length over the
/// speed of sound, the level falls with it, and the pattern decides how much
/// of what arrives off the microphone's axis it keeps. Doppler is not applied
/// on top of any of this, it is what a moving path length already does.
struct MicrophonePath {
    delay_samples: f32,
    gain: f32,
    /// Metres from the mouth to the capsule, which is what decides how much
    /// bass a directional capsule lifts.
    length: f32,
    /// How squarely the mouth faces this microphone: 1 when it points
    /// straight at it, 0 when it points away.
    facing: f32,
}

fn microphone_path(
    place: Placement,
    radius: f32,
    sine: f32,
    cosine: f32,
    samples_per_metre: f32,
    pattern: f32,
) -> MicrophonePath {
    let along = place.distance - radius * cosine;
    let across = place.lateral - radius * sine;
    let squared = (along * along + across * across).max(NEAREST_SQUARED_M2);
    let inverse = inverse_root(squared);
    // Where the sound arrives from, against where the microphone is pointing:
    // an omnidirectional capsule ignores this and a figure of eight lives by
    // it, including the change of sign behind it.
    let incidence = (place.axis.0 * along + place.axis.1 * across) * inverse;
    MicrophonePath {
        delay_samples: squared * inverse * samples_per_metre,
        gain: place.distance * inverse * ((1.0 - pattern) + pattern * incidence),
        length: squared * inverse,
        // The mouth radiates outward along the radius, so how far off axis
        // the microphone lies is the angle between that radius and the line
        // to it.
        facing: 0.5 + 0.5 * (along * cosine + across * sine) * inverse,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum RotaryMode {
    #[default]
    Off = 0,
    Brake = 1,
    Chorale = 2,
    Tremolo = 3,
}

/// Where the microphones stand and what they are, in the units a tape
/// measure and a microphone cabinet use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicrophoneArray {
    /// In front of the cabinet.
    pub distance_m: f32,
    /// Between the pair.
    pub spacing_m: f32,
    /// Of the pair's centre from the rotor's pivot.
    pub offset_m: f32,
    /// Omnidirectional at zero, cardioid at a half, figure of eight at one.
    pub pattern: f32,
}

pub const MIC_DISTANCE_DEFAULT_M: f32 = 0.35;
pub const MIC_SPACING_DEFAULT_M: f32 = 0.3;
pub const MIC_PATTERN_DEFAULT: f32 = 0.5;

impl Default for MicrophoneArray {
    fn default() -> Self {
        Self {
            distance_m: MIC_DISTANCE_DEFAULT_M,
            spacing_m: MIC_SPACING_DEFAULT_M,
            offset_m: 0.0,
            pattern: MIC_PATTERN_DEFAULT,
        }
    }
}

/// The four stands: the horn's pair, then the drum's pair on the other side of
/// the offset.
fn stands(distance: f32, spacing: f32, offset: f32) -> [Placement; 4] {
    let half = 0.5 * spacing;
    [
        Placement::new(distance, offset + half),
        Placement::new(distance, offset - half),
        Placement::new(distance, -offset + half),
        Placement::new(distance, -offset - half),
    ]
}

/// The cabinet's geometry, in metres, for tools that need to predict what it
/// should be doing rather than take its word for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotaryGeometry {
    pub horn_radius_m: f32,
    pub drum_radius_m: f32,
    pub sound_speed_m_per_s: f32,
    /// Microphone placement: in front of the cabinet, half the spacing
    /// between the pair, and the pair's offset from the pivot.
    pub mic_distance_m: f32,
    pub mic_half_width_m: f32,
    pub mic_centre_m: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotaryDiagnostics {
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
    /// Where each mouth is pointing, in degrees, with zero facing the
    /// microphones.
    pub horn_angle_degrees: f32,
    pub drum_angle_degrees: f32,
}

impl RotaryMode {
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
    /// The same angle as the pair above, in turns, kept alongside them because
    /// aiming a stop needs to know where the mouth is and not only where it
    /// points.
    phase: f32,
    stop_angle: StopAngle,
    brake: Option<Brake>,
    speed_hz: f32,
    target_hz: f32,
    /// Hertz per second while ramping. Hammond defines a transition time as
    /// the time to cross the whole speed range, so a shorter move takes
    /// proportionally less time and the rate is what stays constant.
    rate_hz_per_second: f32,
    /// Samples still to wait before the speed starts changing.
    delay_frames: u32,
    /// Rated speeds, on the supply the cabinet was built for.
    slow_hz: f32,
    fast_hz: f32,
    /// What the supply in use does to them.
    supply: f32,
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
            phase: 0.0,
            stop_angle: StopAngle::Fixed(0.0),
            brake: None,
            speed_hz: 0.0,
            target_hz: 0.0,
            rate_hz_per_second: 0.0,
            delay_frames: 0,
            slow_hz: rpm_to_hz(slow_rpm),
            fast_hz: rpm_to_hz(fast_rpm),
            supply: 1.0,
            rise_seconds: rise,
            fall_seconds: fall,
            brake_seconds: brake,
            direction,
        }
    }

    /// Points the rotor at a new speed and works out how fast it may get
    /// there: up takes the rise time, down to the slow speed the fall time,
    /// and down to a stop the brake time.
    fn aim(&mut self, target_hz: f32, sample_rate: f32, random: f32) {
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
        self.brake = if target_hz <= 0.0 {
            let target = match (self.stop_angle, self.brake) {
                (StopAngle::Fixed(turns), _) => turns,
                (StopAngle::Random, Some(brake)) => brake.target,
                (StopAngle::Random, None) => random,
            };
            self.shape_stop(sample_rate, target)
        } else {
            None
        };
    }

    /// Works out the curve that brings this rotor to rest facing the right
    /// way. Returns nothing when there is nothing to aim, which is when the
    /// rotor has already stopped.
    fn shape_stop(&self, sample_rate: f32, target: f32) -> Option<Brake> {
        let speed = self.speed_hz;
        if speed <= RESTING_HZ || self.rate_hz_per_second <= 0.0 {
            return None;
        }
        // How far the mouth has to travel, measured the way it is turning,
        // plus however many whole turns bring that nearest a straight ramp.
        // The relay and clutch have not let go yet, so the turns the rotor
        // makes at full speed while waiting come off the journey first.
        let documented = speed / self.rate_hz_per_second;
        let straight = 0.5 * speed * documented;
        let waiting = speed * self.delay_frames as f32 / sample_rate;
        let ahead = wrap_turn((target - self.phase) * self.direction - waiting);
        let extra = (((straight - ahead + 0.5) as i32).max(0)) as f32;
        let turns = (ahead + extra).max(1.0e-5);

        let (shortest, longest) = brake_reach(speed, documented);
        let seconds = if turns > longest {
            1.5 * turns / speed
        } else if turns < shortest {
            3.0 * turns / speed
        } else {
            documented
        }
        .clamp(1.0 / sample_rate, TIME_CEILING);
        let bend = (6.0 * (turns / (speed * seconds) - 0.5)).clamp(-1.0, 1.0);
        Some(Brake {
            start_hz: speed,
            frames: (seconds * sample_rate).max(1.0),
            elapsed: 0.0,
            bend,
            target,
        })
    }

    /// The speeds this rotor actually reaches on the supply it is running
    /// from. The rate it ramps at does not change with the supply - the belt
    /// and the motor are the same - so a cabinet on fifty cycles reaches its
    /// speeds sooner, having less of a range to cross.
    fn slow(&self) -> f32 {
        self.slow_hz * self.supply
    }

    fn fast(&self) -> f32 {
        self.fast_hz * self.supply
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
        } else if let Some(brake) = &mut self.brake {
            brake.elapsed += 1.0;
            let progress = (brake.elapsed / brake.frames).min(1.0);
            self.speed_hz = brake.start_hz * (1.0 - progress) * (1.0 + brake.bend * progress);
            if progress >= 1.0 {
                self.speed_hz = 0.0;
                self.brake = None;
            }
        } else {
            let step = self.rate_hz_per_second / sample_rate;
            if self.speed_hz < self.target_hz {
                self.speed_hz = (self.speed_hz + step).min(self.target_hz);
            } else {
                self.speed_hz = (self.speed_hz - step).max(self.target_hz);
            }
        }
        self.phase = wrap_turn(self.phase + self.direction * self.speed_hz / sample_rate);
        let angle = self.direction * TAU * self.speed_hz / sample_rate;
        let (rotation_sine, rotation_cosine) = small_rotation(angle);
        let sine = self.sine * rotation_cosine + self.cosine * rotation_sine;
        let cosine = self.cosine * rotation_cosine - self.sine * rotation_sine;
        let correction = 1.5 - 0.5 * (sine * sine + cosine * cosine);
        self.sine = sine * correction;
        self.cosine = cosine * correction;
    }
}

pub struct Rotary {
    sample_rate: f32,
    mode: RotaryMode,
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
    /// Horn left and right, then drum left and right.
    places: [Placement; 4],
    pattern: f32,
    horn_radius: f32,
    drum_radius: f32,
    reflections: f32,
    /// The three microphone volumes, as gains.
    horn_gain: f32,
    drum_gain: f32,
    mains: MainsFrequency,
    capsule: MicrophoneType,
    sub_gain: f32,
    /// Where the woofer's own sound reaches each of the four microphones. It
    /// does not turn with anything, so these do not change until a stand
    /// moves.
    sub_paths: [(f32, f32); 4],
    /// One capsule's worth of filtering per output channel: the bass a
    /// directional capsule lifts as it nears the cabinet, and what a dynamic
    /// one does to the top.
    proximity: [f32; 2],
    proximity_coefficient: f32,
    proximity_lift: f32,
    presence_coefficient: f32,
    top_coefficient: f32,
    presence: [f32; 2],
    top: [f32; 2],
    /// Drawn on only when a rotor is asked to stop at a random angle, so a
    /// render with fixed angles stays bit-identical run to run.
    entropy: u32,
}

impl Rotary {
    pub fn new(sample_rate: f32) -> Self {
        let x = TAU * 800.0 / sample_rate;
        let mut cabinet = Self {
            sample_rate,
            mode: RotaryMode::Off,
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
            places: stands(MIC_DISTANCE_DEFAULT_M, MIC_SPACING_DEFAULT_M, 0.0),
            pattern: MIC_PATTERN_DEFAULT,
            horn_radius: HORN_RADIUS_DEFAULT_M,
            drum_radius: DRUM_RADIUS_DEFAULT_M,
            reflections: 0.22,
            horn_gain: 1.0,
            drum_gain: 1.0,
            mains: MainsFrequency::Sixty,
            capsule: MicrophoneType::Condenser,
            sub_gain: from_decibels(SUB_LEVEL_DEFAULT_DB),
            sub_paths: [(1.0, 1.0); 4],
            proximity: [0.0; 2],
            proximity_coefficient: 0.0,
            proximity_lift: 0.0,
            presence_coefficient: one_pole(DYNAMIC_PRESENCE_HZ, sample_rate),
            top_coefficient: one_pole(DYNAMIC_TOP_HZ, sample_rate),
            presence: [0.0; 2],
            top: [0.0; 2],
            entropy: 0x9e37_79b9,
        };
        cabinet.settle_fixed_paths();
        cabinet
    }

    /// Which capsule the pair is.
    pub fn set_microphone_type(&mut self, capsule: MicrophoneType) {
        self.capsule = capsule;
    }

    /// Which supply the cabinet's motors are running from. Changing it while
    /// the rotors are turning re-aims them, the way re-plugging a cabinet
    /// would.
    pub fn set_mains(&mut self, mains: MainsFrequency) {
        if mains == self.mains {
            return;
        }
        self.mains = mains;
        self.horn.supply = mains.scale();
        self.drum.supply = mains.scale();
        let mode = self.mode;
        self.mode = RotaryMode::Off;
        self.set_mode(mode);
    }

    pub const fn mains(&self) -> MainsFrequency {
        self.mains
    }

    /// Where each rotor comes to rest when the cabinet is stopped.
    pub fn set_stop_angles(&mut self, horn: StopAngle, drum: StopAngle) -> bool {
        for angle in [horn, drum] {
            if let StopAngle::Fixed(turns) = angle
                && !(0.0..1.0).contains(&turns)
            {
                return false;
            }
        }
        self.horn.stop_angle = horn;
        self.drum.stop_angle = drum;
        true
    }

    /// A turn's worth of randomness, from a generator that only moves when a
    /// rotor actually asks for it.
    fn roll(&mut self) -> f32 {
        self.entropy ^= self.entropy << 13;
        self.entropy ^= self.entropy >> 17;
        self.entropy ^= self.entropy << 5;
        (self.entropy >> 8) as f32 / (1 << 24) as f32
    }

    pub fn set_mode(&mut self, mode: RotaryMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        let (horn, drum) = match mode {
            RotaryMode::Off | RotaryMode::Brake => (0.0, 0.0),
            RotaryMode::Chorale => (self.horn.slow(), self.drum.slow()),
            RotaryMode::Tremolo => (self.horn.fast(), self.drum.fast()),
        };
        let (horn_roll, drum_roll) = (self.roll(), self.roll());
        self.horn.aim(horn, self.sample_rate, horn_roll);
        self.drum.aim(drum, self.sample_rate, drum_roll);
    }

    pub const fn mode(&self) -> RotaryMode {
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
        let (horn_roll, drum_roll) = (self.roll(), self.roll());
        self.horn.aim(horn_target, self.sample_rate, horn_roll);
        self.drum.aim(drum_target, self.sample_rate, drum_roll);
        self.horn.delay_frames = horn_delay;
        self.drum.delay_frames = drum_delay;
        true
    }

    pub fn set_cabinet(&mut self, reflections: f32) -> bool {
        if !unit(reflections) {
            return false;
        }
        self.reflections = reflections;
        true
    }

    /// The three microphone volumes, in decibels: the horn's pair, the drum's
    /// pair, and the woofer's own sound underneath them.
    pub fn set_levels(&mut self, horn_db: f32, drum_db: f32, sub_db: f32) -> bool {
        let sane = |decibels: f32| {
            decibels.is_finite() && decibels <= LEVEL_RANGE_DB.1 && decibels >= LEVEL_SILENT_DB
        };
        if !sane(horn_db) || !sane(drum_db) || !sane(sub_db) {
            return false;
        }
        self.horn_gain = from_decibels(horn_db);
        self.drum_gain = from_decibels(drum_db);
        self.sub_gain = from_decibels(sub_db);
        true
    }

    /// Moves the microphone stands, in metres, and chooses their pattern from
    /// omnidirectional at zero through cardioid at a half to a figure of eight
    /// at one.
    ///
    /// `offset` slides the pair away from the pivot, and it slides the horn's
    /// pair and the drum's pair in opposite directions: a cabinet's two rotors
    /// turn against each other, so a placement that catches the horn coming
    /// toward you catches the drum going away.
    pub fn set_microphones(&mut self, array: MicrophoneArray) -> bool {
        let (near, far) = MIC_DISTANCE_RANGE_M;
        if !(near..=far).contains(&array.distance_m)
            || !(0.0..=MIC_SPACING_MAX_M).contains(&array.spacing_m)
            || array.offset_m.abs() > MIC_OFFSET_MAX_M
            || !unit(array.pattern)
            || !array.distance_m.is_finite()
            || !array.spacing_m.is_finite()
            || !array.offset_m.is_finite()
        {
            return false;
        }
        self.places = stands(array.distance_m, array.spacing_m, array.offset_m);
        self.pattern = array.pattern;
        self.settle_fixed_paths();
        true
    }

    /// Recomputes everything that does not turn: the woofer's paths to the
    /// four microphones, and the bass a directional capsule lifts at this
    /// distance.
    fn settle_fixed_paths(&mut self) {
        let samples_per_metre = self.sample_rate / SOUND_SPEED_M_PER_S;
        for (slot, place) in self.sub_paths.iter_mut().zip(self.places) {
            // The woofer sits on the axis and stays there, so its path is a
            // rotor's path with no radius to swing on.
            let path = microphone_path(place, 0.0, 0.0, 1.0, samples_per_metre, self.pattern);
            *slot = (path.delay_samples, path.gain);
        }
        // A pressure-gradient capsule hears the difference between two points
        // in the air, and near a source that difference grows toward the
        // bottom: below roughly the speed of sound over the circumference of
        // the distance, the response rises. An omnidirectional capsule reads
        // pressure alone and does none of this, which is why the pattern is
        // in it.
        let path = microphone_path(self.places[0], 0.0, 0.0, 1.0, samples_per_metre, 0.0);
        let corner = SOUND_SPEED_M_PER_S / (TAU * path.length.max(0.05));
        let x = TAU * corner / self.sample_rate;
        self.proximity_coefficient = x / (1.0 + x);
        self.proximity_lift = PROXIMITY_MAX_LIFT * self.pattern;
    }

    /// The radius each rotor's mouth turns at, in metres. This is the one
    /// quantity of the cabinet's geometry we have no published figure for, so
    /// it is a control and not a constant.
    pub fn set_rotor_radii(&mut self, horn_m: f32, drum_m: f32) -> bool {
        let (horn_near, horn_far) = HORN_RADIUS_RANGE_M;
        let (drum_near, drum_far) = DRUM_RADIUS_RANGE_M;
        if !(horn_near..=horn_far).contains(&horn_m) || !(drum_near..=drum_far).contains(&drum_m) {
            return false;
        }
        self.horn_radius = horn_m;
        self.drum_radius = drum_m;
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

        // Four straight lines, from each rotor's mouth to each microphone.
        // Their lengths give the delays, the levels and, because they change
        // as the rotor turns, the Doppler shift.
        let samples_per_metre = self.sample_rate / SOUND_SPEED_M_PER_S;
        let horn_left = microphone_path(
            self.places[0],
            self.horn_radius,
            self.horn.sine,
            self.horn.cosine,
            samples_per_metre,
            self.pattern,
        );
        let horn_right = microphone_path(
            self.places[1],
            self.horn_radius,
            self.horn.sine,
            self.horn.cosine,
            samples_per_metre,
            self.pattern,
        );
        let drum_path_left = microphone_path(
            self.places[2],
            self.drum_radius,
            self.drum.sine,
            self.drum.cosine,
            samples_per_metre,
            self.pattern,
        );
        let drum_path_right = microphone_path(
            self.places[3],
            self.drum_radius,
            self.drum.sine,
            self.drum.cosine,
            samples_per_metre,
            self.pattern,
        );
        let horn_facing_left = horn_left.facing;
        let horn_facing_right = horn_right.facing;
        let drum_facing_left = drum_path_left.facing;
        let drum_facing_right = drum_path_right.facing;
        let raw_horn_left = self.horn_delay.read(horn_left.delay_samples) * horn_left.gain;
        let raw_horn_right = self.horn_delay.read(horn_right.delay_samples) * horn_right.gain;
        let drum_left = self.drum_delay.read(drum_path_left.delay_samples) * drum_path_left.gain;
        let drum_right = self.drum_delay.read(drum_path_right.delay_samples) * drum_path_right.gain;

        let horn_filter = one_pole(2_200.0, self.sample_rate);
        self.horn_tone_left += horn_filter * (raw_horn_left - self.horn_tone_left);
        self.horn_tone_right += horn_filter * (raw_horn_right - self.horn_tone_right);
        let horn_high_left = raw_horn_left - self.horn_tone_left;
        let horn_high_right = raw_horn_right - self.horn_tone_right;
        let horn_left = self.horn_tone_left * (0.55 + 0.35 * horn_facing_left)
            + horn_high_left * (0.20 + 0.80 * horn_facing_left);
        let horn_right = self.horn_tone_right * (0.55 + 0.35 * horn_facing_right)
            + horn_high_right * (0.20 + 0.80 * horn_facing_right);

        let (horn_gain, drum_gain) = (self.horn_gain, self.drum_gain);
        let mut wet_left =
            horn_gain * horn_left + drum_gain * drum_left * (0.66 + 0.34 * drum_facing_left);
        let mut wet_right =
            horn_gain * horn_right + drum_gain * drum_right * (0.66 + 0.34 * drum_facing_right);
        // Where the microphones meet, so does the image: no width, no stereo.

        // The woofer's own sound never enters a rotor. It leaves the cabinet
        // unmodulated and arrives at the microphones that happen to be there,
        // mostly at the drum's pair and a little at the horn's.
        if self.sub_gain > 0.0 {
            let pickup = |paths: [(f32, f32); 4], line: &DelayLine| {
                let horn = SUB_INTO_HORN;
                let drum = SUB_INTO_DRUM;
                (
                    drum * line.read(paths[2].0) * paths[2].1
                        + horn * line.read(paths[0].0) * paths[0].1,
                    drum * line.read(paths[3].0) * paths[3].1
                        + horn * line.read(paths[1].0) * paths[1].1,
                )
            };
            let (sub_left, sub_right) = pickup(self.sub_paths, &self.drum_delay);
            wet_left += self.sub_gain * sub_left;
            wet_right += self.sub_gain * sub_right;
        }

        // What the capsules make of all that.
        let capsule = self.capsule;
        let lift = self.proximity_lift;
        let coefficient = self.proximity_coefficient;
        for (channel, value) in [&mut wet_left, &mut wet_right].into_iter().enumerate() {
            let bass = &mut self.proximity[channel];
            *bass += coefficient * (*value - *bass);
            let mut voiced = *value + lift * *bass;
            if capsule == MicrophoneType::Dynamic {
                let presence = &mut self.presence[channel];
                *presence += self.presence_coefficient * (voiced - *presence);
                voiced += DYNAMIC_PRESENCE_LIFT * (voiced - *presence);
                let top = &mut self.top[channel];
                *top += self.top_coefficient * (voiced - *top);
                voiced = *top;
            }
            *value = voiced;
        }

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
        let wet = if self.mode == RotaryMode::Off {
            0.0
        } else {
            self.mix
        };
        [
            input * (1.0 - wet) + wet_left * wet,
            input * (1.0 - wet) + wet_right * wet,
        ]
    }

    /// Read-only geometry for deterministic calibration tools.
    pub fn geometry(&self) -> RotaryGeometry {
        RotaryGeometry {
            horn_radius_m: self.horn_radius,
            drum_radius_m: self.drum_radius,
            sound_speed_m_per_s: SOUND_SPEED_M_PER_S,
            mic_distance_m: self.places[0].distance,
            mic_half_width_m: 0.5 * (self.places[0].lateral - self.places[1].lateral),
            mic_centre_m: 0.5 * (self.places[0].lateral + self.places[1].lateral),
        }
    }

    /// Read-only mechanical state for deterministic calibration tools.
    pub const fn diagnostics(&self) -> RotaryDiagnostics {
        RotaryDiagnostics {
            horn_speed_hz: self.horn.speed_hz,
            horn_target_hz: self.horn.target_hz,
            drum_speed_hz: self.drum.speed_hz,
            drum_target_hz: self.drum.target_hz,
            horn_speed_rpm: self.horn.speed_hz * 60.0,
            drum_speed_rpm: self.drum.speed_hz * 60.0,
            horn_transition_seconds: self.horn.transition_seconds(),
            drum_transition_seconds: self.drum.transition_seconds(),
            horn_angle_degrees: self.horn.phase * 360.0,
            drum_angle_degrees: self.drum.phase * 360.0,
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
        self.proximity = [0.0; 2];
        self.presence = [0.0; 2];
        self.top = [0.0; 2];
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

fn one_pole(frequency: f32, sample_rate: f32) -> f32 {
    let normalized = TAU * frequency / sample_rate;
    normalized / (1.0 + normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the cabinet up to a mode, stops it, and reports where each rotor
    /// came to rest and how long the stop took.
    fn stop_from(mode: RotaryMode, horn: StopAngle, drum: StopAngle) -> (f32, f32, f32) {
        let rate = 48_000.0_f32;
        let mut rotary = Rotary::new(rate);
        assert!(rotary.set_stop_angles(horn, drum));
        rotary.set_mode(mode);
        for _ in 0..(rate as usize * 12) {
            rotary.process(0.0);
        }
        rotary.set_mode(RotaryMode::Brake);
        let mut frames = 0.0;
        let mut moving = true;
        for _ in 0..(rate as usize * 20) {
            rotary.process(0.0);
            let state = rotary.diagnostics();
            if moving {
                frames += 1.0;
                if state.horn_speed_hz <= 0.0 && state.drum_speed_hz <= 0.0 {
                    moving = false;
                }
            }
        }
        let state = rotary.diagnostics();
        (
            state.horn_angle_degrees,
            state.drum_angle_degrees,
            frames / rate,
        )
    }

    /// The distance between two angles on a circle, in degrees.
    fn apart(left: f32, right: f32) -> f32 {
        let gap = (left - right).abs() % 360.0;
        if gap > 180.0 { 360.0 - gap } else { gap }
    }

    /// A stopped rotor ends up facing where it was told to, from either
    /// running speed. The horn and the drum turn against each other, so the
    /// same aim is a different journey for each.
    #[test]
    fn a_stop_lands_where_it_was_aimed() {
        for degrees in [0.0_f32, 37.0, 180.0, 300.0] {
            let aim = StopAngle::from_degrees(degrees).unwrap();
            for mode in [RotaryMode::Tremolo, RotaryMode::Chorale] {
                let (horn, drum, seconds) = stop_from(mode, aim, aim);
                assert!(
                    apart(horn, degrees) < 2.0,
                    "horn stopped at {horn} instead of {degrees} from {mode:?}"
                );
                assert!(
                    apart(drum, degrees) < 2.0,
                    "drum stopped at {drum} instead of {degrees} from {mode:?}"
                );
                // Hammond's brake time is the time to stop from the fast
                // speed, and that is where it has to hold: the drum is the
                // slower of the pair, so it sets what the pair takes.
                let expected = if mode == RotaryMode::Tremolo {
                    (1.7, 3.2)
                } else {
                    (0.05, 3.2)
                };
                assert!(
                    (expected.0..expected.1).contains(&seconds),
                    "the stop took {seconds}s from {mode:?}"
                );
            }
        }
    }

    /// Decibels to a gain, against the values a table would give.
    #[test]
    fn levels_convert_the_way_decibels_do() {
        for (decibels, gain) in [
            (0.0_f32, 1.0_f32),
            (-6.0, 0.501_187_2),
            (-20.0, 0.1),
            (-40.0, 0.01),
            (LEVEL_RANGE_DB.0, 1.584_893_2e-4),
        ] {
            let found = from_decibels(decibels);
            assert!(
                (found - gain).abs() < 1.0e-4 * gain,
                "{decibels} dB gave {found} against {gain}"
            );
        }
        assert_eq!(from_decibels(LEVEL_SILENT_DB), 0.0);
        assert_eq!(from_decibels(-200.0), 0.0);
    }

    /// The woofer's own sound is not in a rotor, so turning one does not
    /// change it: raising it flattens what the cabinet does to a bass note
    /// rather than deepening it.
    #[test]
    fn the_woofer_arrives_unmodulated() {
        let rate = 48_000.0_f32;
        let swing = |decibels: f32| {
            let mut rotary = Rotary::new(rate);
            assert!(rotary.set_mix(1.0));
            assert!(rotary.set_cabinet(0.0));
            assert!(rotary.set_levels(0.0, 0.0, decibels));
            rotary.set_mode(RotaryMode::Tremolo);
            for index in 0..(rate as usize * 12) {
                rotary.process((index as f32 * 0.03).sin());
            }
            let (mut quietest, mut loudest) = (f32::MAX, 0.0_f32);
            let mut envelope = 0.0_f32;
            for index in 0..(rate as usize) {
                // 200 Hz, which the crossover sends to the drum and which the
                // woofer is also putting out on its own.
                let tone = (index as f32 * TAU * 200.0 / rate).sin();
                let [left, _] = rotary.process(tone);
                envelope += 0.002 * (left.abs() - envelope);
                if index > rate as usize / 4 {
                    quietest = quietest.min(envelope);
                    loudest = loudest.max(envelope);
                }
            }
            loudest / quietest.max(1.0e-9)
        };
        let alone = swing(LEVEL_SILENT_DB);
        let with_woofer = swing(0.0);
        assert!(
            with_woofer < alone,
            "the woofer deepened the sweep: {with_woofer} against {alone}"
        );
    }

    /// A directional capsule gains bass as it nears what it is pointed at,
    /// and an omnidirectional one does not, which is the part of the
    /// microphone choice that is physics rather than taste.
    #[test]
    fn closing_in_lifts_the_bass_of_a_directional_capsule() {
        let bass = |distance_m: f32, pattern: f32| {
            let mut rotary = Rotary::new(48_000.0);
            assert!(rotary.set_mix(1.0));
            assert!(rotary.set_cabinet(0.0));
            assert!(rotary.set_levels(0.0, 0.0, LEVEL_SILENT_DB));
            assert!(rotary.set_microphones(MicrophoneArray {
                distance_m,
                spacing_m: 0.0,
                offset_m: 0.0,
                pattern,
            }));
            rotary.set_mode(RotaryMode::Brake);
            let mut sum = 0.0_f32;
            for index in 0..48_000 {
                let tone = (index as f32 * TAU * 50.0 / 48_000.0).sin();
                let [left, _] = rotary.process(tone);
                if index > 24_000 {
                    sum += left * left;
                }
            }
            sum
        };
        let near = bass(0.15, 1.0);
        let far = bass(1.5, 1.0);
        assert!(near > far, "near {near} against far {far}");
        let omni_near = bass(0.15, 0.0);
        let omni_far = bass(1.5, 0.0);
        assert!(
            (near / far) > 1.2 * (omni_near / omni_far),
            "the pattern made no difference: {} against {}",
            near / far,
            omni_near / omni_far
        );
    }

    /// A cabinet on fifty cycles turns at five sixths of every speed it is
    /// rated for, and reaches them sooner, because the belt and the motor
    /// ramp at the rate they always did over a shorter range.
    #[test]
    fn the_supply_sets_the_speeds() {
        let mut sixty = Rotary::new(48_000.0);
        let mut fifty = Rotary::new(48_000.0);
        fifty.set_mains(MainsFrequency::Fifty);
        assert_eq!(fifty.mains(), MainsFrequency::Fifty);
        for mode in [RotaryMode::Chorale, RotaryMode::Tremolo] {
            sixty.set_mode(mode);
            fifty.set_mode(mode);
            for _ in 0..(48_000 * 14) {
                sixty.process(0.0);
                fifty.process(0.0);
            }
            let rated = sixty.diagnostics();
            let exported = fifty.diagnostics();
            for (rated, exported) in [
                (rated.horn_speed_rpm, exported.horn_speed_rpm),
                (rated.drum_speed_rpm, exported.drum_speed_rpm),
            ] {
                assert!(
                    (exported / rated - 50.0 / 60.0).abs() < 1.0e-3,
                    "{exported} rpm against {rated} in {mode:?}"
                );
            }
        }
    }

    /// "Rnd" is the documented alternative, and it has to actually land
    /// somewhere else each time without ever leaving the circle.
    #[test]
    fn a_random_stop_lands_somewhere_new() {
        let rate = 48_000.0_f32;
        let mut rotary = Rotary::new(rate);
        assert!(rotary.set_stop_angles(StopAngle::Random, StopAngle::Random));
        let mut seen = [0.0_f32; 4];
        for slot in &mut seen {
            rotary.set_mode(RotaryMode::Tremolo);
            for _ in 0..(rate as usize * 12) {
                rotary.process(0.0);
            }
            rotary.set_mode(RotaryMode::Brake);
            for _ in 0..(rate as usize * 20) {
                rotary.process(0.0);
            }
            *slot = rotary.diagnostics().horn_angle_degrees;
            assert!(
                (0.0..360.0).contains(slot),
                "angle {slot} is off the circle"
            );
        }
        for (index, angle) in seen.iter().enumerate() {
            for other in &seen[index + 1..] {
                assert!(apart(*angle, *other) > 1.0, "{seen:?} repeats");
            }
        }
    }

    /// The reciprocal root against the real one, over every distance a
    /// cabinet can put between a mouth and a microphone, and well past both
    /// ends of it.
    #[test]
    fn the_path_length_root_is_exact_to_a_micrometre() {
        let mut worst = 0.0_f32;
        for step in 0..20_000 {
            let length = 0.01 + step as f32 * 0.001;
            let recovered = length * length * inverse_root(length * length);
            worst = worst.max((recovered - length).abs() / length);
        }
        assert!(worst < 1.0e-6, "worst relative error {worst}");
    }

    /// The microphones stand where the documented ranges say, in metres, and
    /// a stand outside them is refused rather than folded back in.
    #[test]
    fn the_microphones_span_their_documented_ranges() {
        let mut rotary = Rotary::new(48_000.0);
        let (near, far) = MIC_DISTANCE_RANGE_M;
        assert!(rotary.set_microphones(MicrophoneArray {
            distance_m: near,
            spacing_m: 0.0,
            offset_m: 0.0,
            pattern: 0.0,
        }));
        let close = rotary.geometry();
        assert!((close.mic_distance_m - near).abs() < 1.0e-6);
        assert_eq!(close.mic_half_width_m, 0.0);
        assert!(rotary.set_microphones(MicrophoneArray {
            distance_m: far,
            spacing_m: MIC_SPACING_MAX_M,
            offset_m: MIC_OFFSET_MAX_M,
            pattern: 1.0,
        }));
        let wide = rotary.geometry();
        assert!((wide.mic_distance_m - far).abs() < 1.0e-6);
        assert!((wide.mic_half_width_m - 0.5 * MIC_SPACING_MAX_M).abs() < 1.0e-6);
        assert!((wide.mic_centre_m - MIC_OFFSET_MAX_M).abs() < 1.0e-6);
        for refused in [
            MicrophoneArray {
                distance_m: far + 0.01,
                ..MicrophoneArray::default()
            },
            MicrophoneArray {
                distance_m: 0.0,
                ..MicrophoneArray::default()
            },
            MicrophoneArray {
                spacing_m: MIC_SPACING_MAX_M + 0.01,
                ..MicrophoneArray::default()
            },
            MicrophoneArray {
                offset_m: -MIC_OFFSET_MAX_M - 0.01,
                ..MicrophoneArray::default()
            },
            MicrophoneArray {
                pattern: 1.1,
                ..MicrophoneArray::default()
            },
        ] {
            assert!(!rotary.set_microphones(refused), "{refused:?}");
        }
        assert!(!rotary.set_rotor_radii(HORN_RADIUS_RANGE_M.1 + 0.01, DRUM_RADIUS_DEFAULT_M));
        assert!(!rotary.set_rotor_radii(HORN_RADIUS_DEFAULT_M, 0.0));
        assert!(rotary.set_rotor_radii(0.2, 0.1));
        assert_eq!(rotary.geometry().horn_radius_m, 0.2);
    }

    /// A microphone pair that meets in the middle hears one signal: the two
    /// paths are the same path, so the image is mono without anything having
    /// to fold it down.
    #[test]
    fn no_spacing_leaves_no_stereo() {
        let mut rotary = Rotary::new(48_000.0);
        assert!(rotary.set_mix(1.0));
        assert!(rotary.set_cabinet(0.0));
        assert!(rotary.set_microphones(MicrophoneArray {
            distance_m: 0.4,
            spacing_m: 0.0,
            offset_m: 0.0,
            ..MicrophoneArray::default()
        }));
        rotary.set_mode(RotaryMode::Tremolo);
        let mut worst = 0.0_f32;
        for index in 0..48_000 {
            let input = (index as f32 * 0.01).sin();
            let [left, right] = rotary.process(input);
            worst = worst.max((left - right).abs());
        }
        assert!(worst < 1.0e-6, "channels differ by {worst}");
    }

    /// Moving the pair off centre moves the horn's microphones one way and
    /// the drum's the other, so the two rotors are never emphasised from the
    /// same side at once.
    #[test]
    fn the_offset_separates_the_rotors() {
        let mut rotary = Rotary::new(48_000.0);
        assert!(rotary.set_microphones(MicrophoneArray {
            distance_m: 0.4,
            spacing_m: 0.2,
            offset_m: 0.3,
            ..MicrophoneArray::default()
        }));
        let geometry = rotary.geometry();
        assert!(geometry.mic_centre_m > 0.0);
        let horn_left = geometry.mic_centre_m + geometry.mic_half_width_m;
        let drum_left = -geometry.mic_centre_m + geometry.mic_half_width_m;
        assert!(horn_left > drum_left);
    }

    #[test]
    fn off_is_an_exact_bypass_while_rotors_keep_state() {
        let mut rotary = Rotary::new(48_000.0);
        for index in 0..1024 {
            let input = index as f32 / 1024.0 - 0.5;
            assert_eq!(rotary.process(input), [input, input]);
        }
    }

    #[test]
    fn cabinet_reflections_create_a_distinct_tail() {
        let mut dry_cabinet = Rotary::new(48_000.0);
        let mut live_cabinet = Rotary::new(48_000.0);
        dry_cabinet.set_mode(RotaryMode::Brake);
        live_cabinet.set_mode(RotaryMode::Brake);
        assert!(dry_cabinet.set_mix(1.0));
        assert!(live_cabinet.set_mix(1.0));
        assert!(dry_cabinet.set_cabinet(0.0));
        assert!(live_cabinet.set_cabinet(1.0));
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
        let mut rotary = Rotary::new(48_000.0);
        rotary.set_mode(RotaryMode::Tremolo);
        let initial = rotary.diagnostics();
        // 400 rpm on the horn and 340 on the drum, as a cabinet is specified.
        assert!((initial.horn_target_hz * 60.0 - 400.0).abs() < 0.01);
        assert!((initial.drum_target_hz * 60.0 - 340.0).abs() < 0.01);
        assert_eq!(initial.horn_speed_hz, 0.0);
        for _ in 0..48_000 {
            rotary.process(0.0);
        }
        let moving = rotary.diagnostics();
        assert!(moving.horn_speed_rpm > moving.drum_speed_rpm);
        assert!(moving.horn_speed_hz <= moving.horn_target_hz);
        assert!(moving.horn_transition_seconds >= 0.8);
        assert!(moving.drum_transition_seconds >= 1.0);
    }

    /// Seconds a rotor needs to reach a speed, sampled at the frame it
    /// arrives.
    fn settle_seconds(rotary: &mut Rotary, horn: bool) -> f32 {
        for frame in 0..48_000 * 20 {
            rotary.process(0.0);
            let state = rotary.diagnostics();
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
        let mut rotary = Rotary::new(48_000.0);
        assert!(rotary.set_acceleration(0.5));
        // From the slow speed, so the rotor crosses exactly the span the
        // transition time is defined over.
        rotary.set_mode(RotaryMode::Chorale);
        settle_seconds(&mut rotary, true);
        rotary.set_mode(RotaryMode::Tremolo);
        let rise = settle_seconds(&mut rotary, true);
        assert!(
            (rise - HORN_RISE_SECONDS - MODE_DELAY_SECONDS).abs() < 0.1,
            "horn rise took {rise} s"
        );

        rotary.set_mode(RotaryMode::Chorale);
        let fall = settle_seconds(&mut rotary, true);
        assert!(
            (fall - HORN_FALL_SECONDS - MODE_DELAY_SECONDS).abs() < 0.1,
            "horn fall took {fall} s"
        );

        // Hammond defines the brake time as the time to stop from the fast
        // speed, which is the one case where the angle the rotor has to reach
        // always fits inside it. From any slower speed the stop takes as long
        // as reaching that angle needs, which is what
        // `a_stop_lands_where_it_was_aimed` covers.
        rotary.set_mode(RotaryMode::Tremolo);
        settle_seconds(&mut rotary, true);
        rotary.set_mode(RotaryMode::Brake);
        let brake = settle_seconds(&mut rotary, true);
        assert!(
            (brake - HORN_BRAKE_SECONDS - MODE_DELAY_SECONDS).abs() < 0.1,
            "braking from fast took {brake} s"
        );
    }

    /// The drum is heavier than the horn and Hammond floors both, so no
    /// setting of the control can make either move faster than a cabinet can.
    #[test]
    fn transition_times_stay_inside_the_documented_range() {
        let mut rotary = Rotary::new(48_000.0);
        for step in 0..=10 {
            assert!(rotary.set_acceleration(step as f32 / 10.0));
            rotary.set_mode(RotaryMode::Chorale);
            rotary.set_mode(RotaryMode::Tremolo);
            let state = rotary.diagnostics();
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
