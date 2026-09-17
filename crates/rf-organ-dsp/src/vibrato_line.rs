// SPDX-License-Identifier: GPL-2.0-or-later

//! The B-3 vibrato/chorus LC delay line.
//!
//! The console generates vibrato with a lumped LC ladder and an
//! electromechanical scanner rather than with a delay and an oscillator. This
//! module models the ladder itself: eighteen 500 mH series inductors, eighteen
//! shunt capacitors, six tap dividers, a 15 kΩ termination and the 22 kΩ
//! source resistor that the vibrato/chorus switch shorts. Component values and
//! tap assignments come from the B-3 service manual as tabulated in the
//! DAFx-2016 vibrato/chorus paper; see `THIRD_PARTY_NOTICES.md`.
//!
//! The ladder is linear, so it is discretised once with the trapezoidal rule
//! into a fixed state transition, and each sample costs one matrix-vector
//! product. Nothing is allocated and the coefficients never change while
//! audio runs: the two switch positions are built up front.

/// Series inductors, shunt capacitors and ladder stages.
const SECTIONS: usize = 18;
/// Inductor currents plus capacitor voltages, the largest of the two switch
/// positions. The vibrato position holds one state fewer, because a shorted
/// source resistor forces the first node.
const STATES: usize = 2 * SECTIONS;
const NODES: usize = SECTIONS + 1;
/// Nine scanner terminals; sixteen plate stacks scan them out and back.
pub const TERMINALS: usize = 9;
const STACKS: usize = 16;

const INDUCTANCE_H: f64 = 0.5;
const CAPACITANCE_F: [f64; SECTIONS] = [
    4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9,
    4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 4.0e-9, 1.0e-9,
];
/// Where the ladder gives up, from the components alone: a constant-k
/// low-pass section built from a series inductance and a shunt capacitance
/// turns over at one over pi root LC, which for 500 mH and 4 nF is just over
/// seven kilohertz. This is not fitted; it is what the two values above make,
/// and the laboratory measures the gain there at every supported rate.
pub const CUTOFF_HZ: f64 = 7117.6259;

const TERMINATION_OHM: f64 = 15_000.0;
/// Rc: in circuit for chorus, shorted for vibrato.
const SOURCE_OHM: f64 = 22_000.0;
/// The six tap dividers, as (upper, lower) resistance in ohms. Their ratio is
/// the tap gain and their sum loads the ladder node.
const DIVIDERS: [(f64, f64); 6] = [
    (27_000.0, 68_000.0),
    (56_000.0, 150_000.0),
    (39_000.0, 150_000.0),
    (33_000.0, 180_000.0),
    (18_000.0, 180_000.0),
    (12_000.0, 180_000.0),
];

/// Ladder nodes wired to terminals t1..t9 for each depth position, one-based
/// as the schematic numbers them.
const DEPTH_TERMINAL_NODES: [[usize; TERMINALS]; 3] = [
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 2, 3, 5, 7, 9, 11, 12, 13],
    [1, 2, 4, 7, 10, 13, 16, 18, 19],
];

/// Terminal reached by each of the sixteen plate stacks: out from t1 to t9 and
/// back again, which is why one revolution produces two vibrato cycles.
const STACK_TERMINAL: [usize; STACKS] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 7, 6, 5, 4, 3, 2, 1];

/// Scanner revolutions per second. The scanner is driven from the generator's
/// synchronous motor; the DAFx paper quotes about 6 Hz for the same part.
pub const ROTOR_HZ: f32 = 6.9;

/// sqrt(L / C) of a uniform section. Scaling inductor currents by it puts
/// them in volts, which keeps every state matrix entry near 1/sqrt(LC)
/// instead of spanning eight decades. It is a change of variable, not a
/// physical quantity; a test checks it against the component values, because
/// the engine carries no square root of its own.
const IMPEDANCE_OHM: f64 = 11_180.339_887_498_949;

fn divider_gain(node: usize) -> f64 {
    match DIVIDERS.get(node - 1) {
        Some((upper, lower)) => lower / (upper + lower),
        None => 1.0,
    }
}

fn divider_load(node: usize) -> f64 {
    match DIVIDERS.get(node - 1) {
        Some((upper, lower)) => 1.0 / (upper + lower),
        None => 0.0,
    }
}

/// Direct-current gain from the input to the ladder, which is where the
/// source resistor's insertion loss comes from. The tap dividers load the
/// ladder here; their own ratio is applied at the tap and is not part of this.
fn insertion_gain(driven: bool) -> f64 {
    if driven {
        return 1.0;
    }
    let mut conductance = 1.0 / TERMINATION_OHM;
    for node in 1..=DIVIDERS.len() {
        conductance += divider_load(node);
    }
    let load = 1.0 / conductance;
    load / (SOURCE_OHM + load)
}

/// One switch position, discretised.
///
/// Interleaving the states section by section makes the ladder tridiagonal: a
/// capacitor sees the currents either side of it and an inductor sees the
/// voltages either side of it, and nothing reaches further. So instead of a
/// transition matrix, each sample applies the explicit half of the trapezoidal
/// step and then solves the implicit half directly, which is both exact and
/// cheaper than the banded matrix it replaces.
struct Ladder {
    /// Sub-, main- and super-diagonal of `I + (h/2)F`, applied to the state.
    explicit: [[f32; 3]; STATES],
    /// Factors of `I - (h/2)F`, ready for a forward and a back sweep: the
    /// multiplier below the diagonal, the reciprocal of the pivot, and the
    /// eliminated super-diagonal.
    lower: [f32; STATES],
    inverse_pivot: [f32; STATES],
    upper: [f32; STATES],
    input: [f32; STATES],
    states: usize,
    /// True when the source resistor is shorted and node 1 is the input.
    driven: bool,
    /// Recovers the switch position's insertion loss. The AO-28 drives and
    /// recovers the line with its own amplifier stage, which is not modelled
    /// yet; without this, selecting chorus would drop the console by 11 dB.
    /// Provisional, and derived from the circuit rather than fitted.
    makeup: f32,
}

impl Ladder {
    fn build(sample_rate: f64, driven: bool) -> Self {
        let states = if driven { STATES - 1 } else { STATES };
        let (derivative, source) = assemble(driven);
        let (explicit, lower, inverse_pivot, upper, input) =
            factor(&derivative, &source, states, step_seconds_for(sample_rate));
        Self {
            explicit,
            lower,
            inverse_pivot,
            upper,
            input,
            states,
            driven,
            makeup: (1.0 / insertion_gain(driven)) as f32,
        }
    }

    fn step(&self, state: &mut [f32; STATES], excitation: f32) {
        // The explicit half of the step, plus the excitation.
        let mut rhs = [0.0_f32; STATES];
        for (row, slot) in rhs.iter_mut().enumerate().take(self.states) {
            let [below, middle, above] = self.explicit[row];
            let mut sum = self.input[row] * excitation + middle * state[row];
            if row > 0 {
                sum += below * state[row - 1];
            }
            if row + 1 < self.states {
                sum += above * state[row + 1];
            }
            *slot = sum;
        }

        // Forward sweep, then back substitution, over the factors of the
        // implicit half.
        let mut solved = [0.0_f32; STATES];
        let mut previous = 0.0;
        for row in 0..self.states {
            let value = (rhs[row] - self.lower[row] * previous) * self.inverse_pivot[row];
            solved[row] = value;
            previous = value;
        }
        let mut next = 0.0;
        for row in (0..self.states).rev() {
            let value = solved[row] - self.upper[row] * next;
            state[row] = value;
            next = value;
        }
    }

    /// Voltage at a schematic node, one-based. Node 19 is algebraic: it only
    /// exists across the termination resistor.
    fn node_voltage(&self, state: &[f32; STATES], input: f32, node: usize) -> f32 {
        if node == NODES {
            return state[current_index(SECTIONS, self.driven)]
                * (TERMINATION_OHM / IMPEDANCE_OHM) as f32;
        }
        match voltage_index(node, self.driven) {
            Some(index) => state[index],
            None => input,
        }
    }
}

/// States are interleaved section by section, voltage before current, so that
/// neighbouring sections land in neighbouring indices. The ladder only couples
/// adjacent sections, which is what keeps the transition matrix banded and the
/// per-sample cost linear in the number of sections.
const fn current_index(section: usize, driven: bool) -> usize {
    if driven {
        2 * (section - 1)
    } else {
        2 * (section - 1) + 1
    }
}

/// Index of a node's capacitor voltage, or `None` when the node is the forced
/// input of the vibrato position.
const fn voltage_index(node: usize, driven: bool) -> Option<usize> {
    if driven {
        if node == 1 { None } else { Some(2 * node - 3) }
    } else {
        Some(2 * (node - 1))
    }
}

/// Writes the ladder's own equations: a capacitor row is its two neighbouring
/// currents and whatever the node leaks through, an inductor row is the
/// voltage either side of it, and the last inductor sees the termination
/// resistor instead of a node.
fn assemble(driven: bool) -> ([[f64; STATES]; STATES], [f64; STATES]) {
    let scale = IMPEDANCE_OHM;
    let mut derivative = [[0.0_f64; STATES]; STATES];
    let mut source = [0.0_f64; STATES];

    for node in 1..=SECTIONS {
        let Some(row) = voltage_index(node, driven) else {
            continue;
        };
        let capacitance = CAPACITANCE_F[node - 1];
        if node >= 2 {
            derivative[row][current_index(node - 1, driven)] += 1.0 / (scale * capacitance);
        }
        derivative[row][current_index(node, driven)] -= 1.0 / (scale * capacitance);
        derivative[row][row] -= divider_load(node) / capacitance;
        if !driven && node == 1 {
            derivative[row][row] -= 1.0 / (SOURCE_OHM * capacitance);
            source[row] += 1.0 / (SOURCE_OHM * capacitance);
        }
    }

    for section in 1..=SECTIONS {
        let row = current_index(section, driven);
        match voltage_index(section, driven) {
            Some(left) => derivative[row][left] += scale / INDUCTANCE_H,
            None => source[row] += scale / INDUCTANCE_H,
        }
        if section < SECTIONS {
            let right = voltage_index(section + 1, driven).expect("interior node is a state");
            derivative[row][right] -= scale / INDUCTANCE_H;
        } else {
            derivative[row][current_index(SECTIONS, driven)] -= TERMINATION_OHM / INDUCTANCE_H;
        }
    }
    (derivative, source)
}

/// The step the trapezoidal rule is given, which is the one the clock ticks
/// at, and an account of why it is not warped.
///
/// Integrating a continuous circuit by the trapezoidal rule is the bilinear
/// transform, and the bilinear transform bends frequency: what the components
/// put at a corner arrives a little lower, and by more the slower the clock.
/// The remedy is standard, and the DAFx-2016 vibrato paper uses it - replace
/// the step T by `T' = 2 tan(W T / 2) / W`, which maps one chosen frequency
/// exactly - so it was tried here, warped at the ladder's own corner.
///
/// It does what it promises. The corner stops moving with the clock: across
/// the five supported rates its gain went from a spread of 4.78 dB to one of
/// 0.001 dB, and the worst spread anywhere in the band fell from 6.06 dB to
/// 4.43 dB.
///
/// It was reverted anyway, because of what it costs. A warped step is a
/// different step, so the ladder's delay is scaled by T'/T - twenty per cent
/// at 32 kHz - and this ladder's whole purpose is its delay. The vibrato's
/// sideband level, which is the depth a player hears, moved 8.7 dB across
/// those same rates with the warping in and stays inside 1 dB without it. The
/// paper warps to match magnitude responses and is right to; RF-Organ uses
/// the ladder as a delay, so it keeps the delay and lets the corner's
/// magnitude drift. `docs/CALIBRATION.md` has the measurement.
fn step_seconds_for(sample_rate: f64) -> f64 {
    1.0 / sample_rate
}

/// Splits the trapezoidal step into the explicit half and the factors of the
/// implicit half.
///
/// `dx/dt = F x + g u` becomes `(I - hF/2) x[n] = (I + hF/2) x[n-1] + hg/2 (u[n] + u[n-1])`.
/// Both matrices are tridiagonal here, so the left-hand side factors once into
/// the sweeps a Thomas solve needs and the right-hand side is three
/// multiplications per row.
type Tridiagonal = (
    [[f32; 3]; STATES],
    [f32; STATES],
    [f32; STATES],
    [f32; STATES],
    [f32; STATES],
);

fn factor(
    derivative: &[[f64; STATES]; STATES],
    source: &[f64; STATES],
    states: usize,
    step_seconds: f64,
) -> Tridiagonal {
    let half = 0.5 * step_seconds;
    let mut explicit = [[0.0_f32; 3]; STATES];
    let mut lower = [0.0_f32; STATES];
    let mut inverse_pivot = [0.0_f32; STATES];
    let mut upper = [0.0_f32; STATES];
    let mut input = [0.0_f32; STATES];

    // Pull the three diagonals out, and check while doing it that the ladder
    // really does not reach further than its neighbours.
    let mut below = [0.0_f64; STATES];
    let mut middle = [0.0_f64; STATES];
    let mut above = [0.0_f64; STATES];
    for (row, equation) in derivative.iter().enumerate().take(states) {
        for (column, value) in equation.iter().copied().enumerate().take(states) {
            match column as isize - row as isize {
                -1 => below[row] = value,
                0 => middle[row] = value,
                1 => above[row] = value,
                _ => debug_assert!(
                    value == 0.0,
                    "the ladder is not tridiagonal in this ordering"
                ),
            }
        }
        explicit[row] = [
            (half * below[row]) as f32,
            (1.0 + half * middle[row]) as f32,
            (half * above[row]) as f32,
        ];
        input[row] = (half * source[row]) as f32;
    }

    // Thomas factorisation of I - hF/2, which the step then reuses every
    // sample. The ladder's own couplings keep the pivots far from zero at
    // every supported sample rate; a degenerate one would leave its row
    // untouched rather than divide by zero.
    let mut eliminated = 0.0_f64;
    for row in 0..states {
        let sub = -half * below[row];
        let pivot = 1.0 - half * middle[row] - sub * eliminated;
        let reciprocal = if pivot == 0.0 || !pivot.is_finite() {
            0.0
        } else {
            1.0 / pivot
        };
        eliminated = -half * above[row] * reciprocal;
        lower[row] = sub as f32;
        inverse_pivot[row] = reciprocal as f32;
        upper[row] = eliminated as f32;
    }
    (explicit, lower, inverse_pivot, upper, input)
}

/// Discretises `dx/dt = F x + g u` with the trapezoidal rule, returning the
/// transition matrix and the input vector for the averaged excitation
/// `u[n] + u[n-1]`. The engine solves the same step directly; this dense form
/// is what the tests hold that solve against.
#[cfg(test)]
fn trapezoidal(
    derivative: &[[f64; STATES]; STATES],
    source: &[f64; STATES],
    states: usize,
    step_seconds: f64,
) -> ([[f32; STATES]; STATES], [f32; STATES]) {
    let half = 0.5 * step_seconds;
    // Solve (I - h F) X = [(I + h F) | h g] in one elimination, which yields
    // the transition matrix and the input vector without forming an inverse.
    let mut system = [[0.0_f64; STATES]; STATES];
    let mut augmented = [[0.0_f64; STATES + 1]; STATES];
    for row in 0..states {
        for column in 0..states {
            let scaled = half * derivative[row][column];
            let identity = if row == column { 1.0 } else { 0.0 };
            system[row][column] = identity - scaled;
            augmented[row][column] = identity + scaled;
        }
        augmented[row][STATES] = half * source[row];
    }
    gauss_jordan(&mut system, &mut augmented, states);

    let mut transition = [[0.0_f32; STATES]; STATES];
    let mut input = [0.0_f32; STATES];
    for row in 0..states {
        for column in 0..states {
            transition[row][column] = augmented[row][column] as f32;
        }
        input[row] = augmented[row][STATES] as f32;
    }
    (transition, input)
}

/// In-place Gauss-Jordan elimination with partial pivoting, for the dense
/// reference the tests use.
#[cfg(test)]
fn gauss_jordan(
    system: &mut [[f64; STATES]; STATES],
    augmented: &mut [[f64; STATES + 1]; STATES],
    states: usize,
) {
    for pivot in 0..states {
        let mut best = pivot;
        for row in pivot + 1..states {
            if system[row][pivot].abs() > system[best][pivot].abs() {
                best = row;
            }
        }
        if best != pivot {
            system.swap(pivot, best);
            augmented.swap(pivot, best);
        }
        let divisor = system[pivot][pivot];
        if divisor == 0.0 || !divisor.is_finite() {
            continue;
        }
        for value in system[pivot][pivot..states].iter_mut() {
            *value /= divisor;
        }
        for value in augmented[pivot].iter_mut() {
            *value /= divisor;
        }
        let pivot_row = system[pivot];
        let pivot_augmented = augmented[pivot];
        for row in 0..states {
            if row == pivot {
                continue;
            }
            let factor = system[row][pivot];
            if factor == 0.0 {
                continue;
            }
            for (target, source) in system[row][pivot..states]
                .iter_mut()
                .zip(&pivot_row[pivot..states])
            {
                *target -= factor * source;
            }
            for (target, source) in augmented[row].iter_mut().zip(&pivot_augmented) {
                *target -= factor * source;
            }
        }
    }
}

/// The complete vibrato/chorus: the ladder in both switch positions, the
/// rotating scanner and the depth selector.
pub struct VibratoLine {
    vibrato: Ladder,
    chorus: Ladder,
    state: [f32; STATES],
    previous_input: f32,
    phase: f32,
    phase_step: f32,
}

impl VibratoLine {
    pub fn new(sample_rate: f32) -> Self {
        let rate = f64::from(sample_rate);
        Self {
            vibrato: Ladder::build(rate, true),
            chorus: Ladder::build(rate, false),
            state: [0.0; STATES],
            previous_input: 0.0,
            phase: 0.0,
            phase_step: ROTOR_HZ / sample_rate,
        }
    }

    /// Advances the ladder by one sample and returns the scanner output for
    /// the given depth, where `depth` selects the tap set as V1/C1, V2/C2 or
    /// V3/C3 and `chorus` opens the source resistor.
    pub fn process(&mut self, input: f32, depth: usize, chorus: bool) -> f32 {
        let excitation = input + self.previous_input;
        if chorus {
            self.chorus.step(&mut self.state, excitation);
        } else {
            self.vibrato.step(&mut self.state, excitation);
        }
        self.previous_input = input;

        self.advance();
        let position = self.phase * STACKS as f32;
        let stack = position as usize % STACKS;
        let blend = position - (position as u32) as f32;
        let ladder = if chorus { &self.chorus } else { &self.vibrato };
        let nodes = &DEPTH_TERMINAL_NODES[depth.min(DEPTH_TERMINAL_NODES.len() - 1)];
        let first = nodes[STACK_TERMINAL[stack]];
        let second = nodes[STACK_TERMINAL[(stack + 1) % STACKS]];
        let near = self.terminal(ladder, input, first);
        let far = self.terminal(ladder, input, second);
        (near + blend * (far - near)) * ladder.makeup
    }

    /// Keeps the scanner turning while the console is switched to bypass, so
    /// that selecting a vibrato lands at the angle the rotor would really be
    /// at.
    pub fn advance_scanner(&mut self) {
        self.advance();
    }

    fn advance(&mut self) {
        self.phase += self.phase_step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
    }

    fn terminal(&self, ladder: &Ladder, input: f32, node: usize) -> f32 {
        ladder.node_voltage(&self.state, input, node) * divider_gain(node) as f32
    }

    pub fn reset(&mut self) {
        self.state = [0.0; STATES];
        self.previous_input = 0.0;
    }

    /// Scanner angle in revolutions, for tests.
    #[cfg(test)]
    pub const fn phase(&self) -> f32 {
        self.phase
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    fn impulse_response(chorus: bool, depth: usize, frames: usize) -> Vec<f32> {
        let mut line = VibratoLine::new(48_000.0);
        (0..frames)
            .map(|frame| {
                let input = if frame == 0 { 1.0 } else { 0.0 };
                line.process(input, depth, chorus)
            })
            .collect()
    }

    /// The ladder is passive, so nothing it does can grow without bound.
    #[test]
    fn both_switch_positions_are_stable() {
        for chorus in [false, true] {
            let response = impulse_response(chorus, 2, 48_000);
            assert!(response.iter().all(|sample| sample.is_finite()));
            let tail = response[24_000..]
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max);
            assert!(tail < 1.0e-3, "chorus={chorus} tail={tail}");
        }
    }

    /// A lumped LC line delays by sqrt(sum L * sum C), about 0.85 ms for the
    /// documented values. The scanner reaches the far end of the line at the
    /// widest depth setting.
    #[test]
    fn the_line_delays_by_its_documented_time() {
        let mut line = VibratoLine::new(48_000.0);
        let mut energy_time = 0.0;
        let mut energy = 0.0;
        for frame in 0..4_800 {
            let input = if frame == 0 { 1.0 } else { 0.0 };
            // Hold the scanner on the last terminal by reading the node
            // directly: the far tap of V3 is the end of the ladder.
            line.process(input, 2, false);
            let value = line.vibrato.node_voltage(&line.state, 0.0, NODES).abs();
            energy_time += value * value * frame as f32 / 48_000.0;
            energy += value * value;
        }
        let centroid = energy_time / energy;
        assert!(
            (0.0005..0.0020).contains(&centroid),
            "centroid {centroid} s is not the documented ladder delay"
        );
    }

    /// Nine terminals, sixteen stacks, out and back: the scanner returns to
    /// the first terminal once per revolution and reaches the last one once.
    #[test]
    fn the_scanner_visits_every_terminal_twice_except_the_ends() {
        let mut counts = [0_usize; TERMINALS];
        for stack in STACK_TERMINAL {
            counts[stack] += 1;
        }
        assert_eq!(counts[0], 1);
        assert_eq!(counts[TERMINALS - 1], 1);
        assert!(counts[1..TERMINALS - 1].iter().all(|count| *count == 2));
    }

    /// Only the chorus position loses level, and only because of Rc.
    /// The step solves the same equation the dense transition describes, so a
    /// second of impulse response has to come out the same however it is
    /// worked out. This is the check that the exact solve replaced an
    /// approximation with an equivalent, not with something close.
    #[test]
    fn the_direct_solve_matches_the_dense_transition() {
        for rate in [32_000.0, 48_000.0, 96_000.0, 192_000.0] {
            for driven in [true, false] {
                let ladder = Ladder::build(rate, driven);
                let (derivative, source) = assemble(driven);
                let (transition, input) =
                    // The same step the engine took, warped and all: the
                    // reference is there to check the solve, not the
                    // discretisation.
                    trapezoidal(&derivative, &source, ladder.states, step_seconds_for(rate));

                let mut solved = [0.0_f32; STATES];
                let mut dense = [0.0_f32; STATES];
                let mut worst = 0.0_f32;
                let mut peak = 0.0_f32;
                for frame in 0..rate as usize {
                    let excitation = if frame == 0 { 1.0 } else { 0.0 };
                    ladder.step(&mut solved, excitation);

                    let mut next = [0.0_f32; STATES];
                    for (row, slot) in next.iter_mut().enumerate().take(ladder.states) {
                        let mut sum = input[row] * excitation;
                        for (column, value) in dense.iter().enumerate().take(ladder.states) {
                            sum += transition[row][column] * value;
                        }
                        *slot = sum;
                    }
                    dense = next;

                    for (left, right) in solved.iter().zip(&dense) {
                        worst = worst.max((left - right).abs());
                        peak = peak.max(right.abs());
                    }
                }
                assert!(
                    worst < peak * 1.0e-4,
                    "{rate} Hz driven={driven}: {worst} against a peak of {peak}"
                );
            }
        }
    }

    /// Interleaving the states is what makes the ladder tridiagonal, and the
    /// solve depends on it: nothing may reach past a neighbour.
    #[test]
    fn the_ladder_only_couples_neighbouring_states() {
        for driven in [true, false] {
            let states = if driven { STATES - 1 } else { STATES };
            let (derivative, _) = assemble(driven);
            for (row, equation) in derivative.iter().enumerate().take(states) {
                for (column, value) in equation.iter().copied().enumerate().take(states) {
                    if row.abs_diff(column) > 1 {
                        assert_eq!(value, 0.0, "driven={driven} couples {row} to {column}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_state_scaling_matches_the_component_values() {
        let expected = (INDUCTANCE_H / CAPACITANCE_F[0]).sqrt();
        assert!((IMPEDANCE_OHM - expected).abs() < 1.0e-6, "{IMPEDANCE_OHM}");
    }

    #[test]
    fn the_source_resistor_is_the_only_insertion_loss() {
        assert_eq!(insertion_gain(true), 1.0);
        let chorus = insertion_gain(false);
        assert!(
            (0.25..0.35).contains(&chorus),
            "chorus insertion gain is {chorus}"
        );
    }

    #[test]
    fn tap_dividers_match_the_documented_gains() {
        let decibels = |node| 20.0 * divider_gain(node).log10();
        for (node, expected) in [
            (1, -2.9),
            (2, -2.8),
            (3, -2.0),
            (4, -1.5),
            (5, -0.83),
            (6, -0.56),
        ] {
            assert!(
                (decibels(node) - expected).abs() < 0.05,
                "node {node} divider is {} dB",
                decibels(node)
            );
        }
        assert_eq!(divider_gain(7), 1.0);
        assert_eq!(divider_gain(NODES), 1.0);
    }
}
