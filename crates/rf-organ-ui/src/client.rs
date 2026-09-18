// SPDX-License-Identifier: GPL-2.0-or-later

use rf_organ_dsp::{
    DRUM_RADIUS_RANGE_M, HORN_RADIUS_RANGE_M, LEVEL_RANGE_DB, LEVEL_SILENT_DB,
    MIC_DISTANCE_RANGE_M, MIC_OFFSET_MAX_M, MIC_SPACING_MAX_M, MainsFrequency, MicrophoneType,
    Registration, STAGE_CHARACTER_RANGE, StopAngle,
};
use serde_json::{Value, json};

pub const PROTOCOL: &str = "rackforge.plugin.web@1";
pub const PARAMETERS: usize = 95;
pub const DEFAULTS: [f64; PARAMETERS] = [
    0.72, 1.0, 8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.55, 0.45, 0.2, 0.38, 0.32, 0.0, 0.82,
    0.5, 0.0, 0.0, 1.0, 1.0, 1.0, 8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.0, 0.0, 1.0, 0.0,
    0.32, 0.0, 0.0, 0.55, 0.35, 0.3, 0.22, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.18, 0.12,
    0.0, 0.0, 1.0, -9.0, 1.0, 0.0, 0.35, 0.3, 0.0, 0.0, 0.0, 1.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0,
    0.1, 1.0, 8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 2.0, 2.0,
];

/// The element each parameter is bound to, in parameter order.
///
/// The surface reaches for these by name and cannot carry on without one, so
/// a control that leaves the page takes the panel with it. A test below holds
/// the packaged page against this list rather than leaving that to be found
/// by opening the plugin.
pub const CONTROL_IDS: [&str; PARAMETERS] = [
    "output",
    "expression",
    "u16",
    "u513",
    "u8",
    "u4",
    "u223",
    "u2",
    "u135",
    "u113",
    "u1",
    "contact-spread",
    "contact-bounce",
    "leakage",
    "transformer-drive",
    "transformer-memory",
    "rotary-mode",
    "rotary-mix",
    "rotor-inertia",
    "scanner-mode",
    "percussion",
    "percussion-harmonic",
    "percussion-volume",
    "percussion-decay",
    "l16",
    "l513",
    "l8",
    "l4",
    "l223",
    "l2",
    "l135",
    "l113",
    "l1",
    "p16",
    "p8",
    "upper-vibrato",
    "lower-vibrato",
    "console-drive",
    "console-bass",
    "console-treble",
    "swell-character",
    "horn-mic-distance",
    "horn-mic-spacing",
    "reflections",
    "horn-level",
    "t1-drive-trim",
    "t1-memory-trim",
    "t2-drive-trim",
    "t2-memory-trim",
    "t3-drive-trim",
    "t3-memory-trim",
    "horn-mic-offset",
    "mic-pattern",
    "horn-radius",
    "drum-radius",
    "horn-stop-angle",
    "drum-stop-angle",
    "mains-frequency",
    "sub-level",
    "mic-type",
    "drum-level",
    "drum-mic-distance",
    "drum-mic-spacing",
    "drum-mic-offset",
    "horn-mic-sides",
    "drum-mic-sides",
    "drive-wobble",
    "leakage-boost",
    "console-stage-character",
    "v4a-trim",
    "v4b-trim",
    "v3b-trim",
    "contact-delay",
    "transformer-asymmetry",
    "key-click",
    "drawbar-a-16",
    "drawbar-a-5-1-3",
    "drawbar-a-8",
    "drawbar-a-4",
    "drawbar-a-2-2-3",
    "drawbar-a-2",
    "drawbar-a-1-3-5",
    "drawbar-a-1-1-3",
    "drawbar-a-1",
    "lower-drawbar-a-16",
    "lower-drawbar-a-5-1-3",
    "lower-drawbar-a-8",
    "lower-drawbar-a-4",
    "lower-drawbar-a-2-2-3",
    "lower-drawbar-a-2",
    "lower-drawbar-a-1-3-5",
    "lower-drawbar-a-1-1-3",
    "lower-drawbar-a-1",
    "registration",
    "lower-registration",
];

/// Elements the cabinet view writes to, which are not parameters and are
/// reached by name just the same.
pub const VIEW_IDS: [&str; 12] = [
    "programs",
    "status",
    "rotary-view",
    "mic-readout",
    "horn-rotor",
    "drum-rotor",
    "horn-sweep",
    "drum-sweep",
    "horn-mic-left",
    "horn-mic-right",
    "drum-mic-left",
    "drum-mic-right",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Operation {
    Fetch,
    Set(usize, f64),
    Select(String),
}

pub struct Client {
    pub values: [f64; PARAMETERS],
    pub sounds: Vec<Sound>,
    pub selected: String,
    pub status: String,
    pub loaded: bool,
    queued: [Option<f64>; PARAMETERS],
    pending: Option<(String, Operation, f64)>,
    selection: Option<String>,
    refresh: bool,
    serial: u64,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            values: DEFAULTS,
            sounds: Vec::new(),
            selected: String::new(),
            status: "Connecting to RackForge…".into(),
            loaded: false,
            queued: [None; PARAMETERS],
            pending: None,
            selection: None,
            refresh: false,
            serial: 0,
        }
    }
}

/// A range the engine itself defines, so that moving one cannot leave the
/// surface checking against the old one.
fn engine_range(range: (f32, f32)) -> std::ops::RangeInclusive<f64> {
    f64::from(range.0)..=f64::from(range.1)
}

pub fn valid(index: usize, value: f64) -> bool {
    value.is_finite()
        && match index {
            0 => (0.0..=1.5).contains(&value),
            1 | 11..=15 | 17..=18 | 37 | 40 | 43 | 52 | 66..=67 | 72..=74 => {
                (0.0..=1.0).contains(&value)
            }
            2..=10 | 24..=34 | 75..=92 => value.fract() == 0.0 && (0.0..=8.0).contains(&value),
            16 => value.fract() == 0.0 && (0.0..=3.0).contains(&value),
            19 => value.fract() == 0.0 && (0.0..=6.0).contains(&value),
            20..=23 | 35..=36 | 64..=65 => [0.0, 1.0].contains(&value),
            38..=39 | 45..=50 | 69..=71 => (-1.0..=1.0).contains(&value),
            44 | 58 | 60 => {
                (f64::from(LEVEL_SILENT_DB)..=f64::from(LEVEL_RANGE_DB.1)).contains(&value)
            }
            41 | 61 => engine_range(MIC_DISTANCE_RANGE_M).contains(&value),
            42 | 62 => (0.0..=f64::from(MIC_SPACING_MAX_M)).contains(&value),
            51 | 63 => value.abs() <= f64::from(MIC_OFFSET_MAX_M),
            53 => engine_range(HORN_RADIUS_RANGE_M).contains(&value),
            54 => engine_range(DRUM_RADIUS_RANGE_M).contains(&value),
            55..=56 => StopAngle::from_degrees(value as f32).is_some(),
            68 => engine_range(STAGE_CHARACTER_RANGE).contains(&value),
            57 => value.fract() == 0.0 && MainsFrequency::from_index(value as u8).is_some(),
            59 => value.fract() == 0.0 && MicrophoneType::from_index(value as u8).is_some(),
            93..=94 => value.fract() == 0.0 && Registration::from_index(value as u8).is_some(),
            _ => false,
        }
}

pub fn host_lighting(context: &Value) -> Option<&'static str> {
    match context["host"]["lighting"].as_str() {
        Some("day") => Some("day"),
        Some("stage") => Some("stage"),
        _ => None,
    }
}

impl Client {
    pub fn context(&mut self, instance: &Value) {
        self.sounds = instance["sounds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|sound| {
                Some(Sound {
                    id: sound["id"].as_str()?.to_owned(),
                    name: sound["name"].as_str()?.to_owned(),
                })
            })
            .collect();
        let selected = instance["selected_sound_id"].as_str().unwrap_or("");
        if selected != self.selected {
            self.selected = selected.to_owned();
            self.queued.fill(None);
            self.loaded = false;
            self.refresh = true;
        }
    }

    pub fn queue(&mut self, index: usize, value: f64) {
        if self.loaded && self.selection.is_none() && valid(index, value) {
            self.queued[index] = Some(value);
        }
    }

    pub fn select(&mut self, id: &str) {
        if self.loaded && self.sounds.iter().any(|sound| sound.id == id) {
            self.queued.fill(None);
            self.selection = Some(id.to_owned());
            self.loaded = false;
            self.status = "Loading program…".into();
        }
    }

    pub fn parameter_changed(&mut self, index: usize, value: f64) -> bool {
        if !valid(index, value) {
            return false;
        }
        self.values[index] = value;
        true
    }

    pub fn next(&mut self, now: f64, poll: bool) -> Option<Value> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, _, sent)| now - sent > 5_000.0)
        {
            self.pending = None;
            self.selection = None;
            self.queued.fill(None);
            self.loaded = false;
            self.refresh = true;
            self.status = "Host timed out. Reconnecting…".into();
        }
        if self.pending.is_some() {
            return None;
        }
        let operation = if let Some(id) = self.selection.take() {
            Operation::Select(id)
        } else if self.refresh {
            self.refresh = false;
            Operation::Fetch
        } else if let Some(index) = self.queued.iter().position(Option::is_some) {
            Operation::Set(index, self.queued[index].take().expect("queued parameter"))
        } else if poll {
            Operation::Fetch
        } else {
            return None;
        };
        self.serial = self.serial.wrapping_add(1);
        let request_id = format!("rf-organ-ui-{}", self.serial);
        let (method, params) = match &operation {
            Operation::Fetch => ("plugin.parameters", json!({})),
            Operation::Set(index, value) => (
                "plugin.set_parameter",
                json!({"parameter_index": index, "value": value}),
            ),
            Operation::Select(id) => ("plugin.select_sound", json!({"sound_id": id})),
        };
        self.pending = Some((request_id.clone(), operation, now));
        Some(json!({
            "protocol": PROTOCOL,
            "kind": "request",
            "request_id": request_id,
            "method": method,
            "params": params
        }))
    }

    pub fn response(&mut self, message: &Value) {
        let Some((request_id, operation, _)) = &self.pending else {
            return;
        };
        if message["request_id"].as_str() != Some(request_id) {
            return;
        }
        let operation = operation.clone();
        self.pending = None;
        if message["ok"].as_bool() != Some(true) {
            self.loaded = false;
            self.refresh = true;
            self.status = "RackForge rejected the request. Reconnecting…".into();
            return;
        }
        match operation {
            Operation::Select(_) => {
                self.loaded = false;
                self.refresh = true;
            }
            Operation::Set(index, _) => {
                if let Some(value) = message["result"]["value"]
                    .as_f64()
                    .filter(|value| valid(index, *value))
                {
                    self.values[index] = value;
                    self.loaded = true;
                    self.status = String::new();
                } else {
                    self.refresh = true;
                }
            }
            Operation::Fetch => {
                if let Some(values) = snapshot(&message["result"]) {
                    self.values = values;
                    self.loaded = true;
                    self.status = String::new();
                } else {
                    self.loaded = false;
                    self.refresh = true;
                }
            }
        }
    }

    pub fn display(&self, index: usize) -> f64 {
        self.queued[index]
            .or_else(
                || match self.pending.as_ref().map(|(_, operation, _)| operation) {
                    Some(Operation::Set(pending, value)) if *pending == index => Some(*value),
                    _ => None,
                },
            )
            .unwrap_or(self.values[index])
    }
}

fn snapshot(result: &Value) -> Option<[f64; PARAMETERS]> {
    let mut values = [None; PARAMETERS];
    for entry in result["values"].as_array()? {
        let index = usize::try_from(entry["index"].as_u64()?).ok()?;
        if index >= PARAMETERS {
            continue;
        }
        let value = entry["value"].as_f64()?;
        if !valid(index, value) || values[index].replace(value).is_some() {
            return None;
        }
    }
    let mut complete = [0.0; PARAMETERS];
    for (slot, value) in complete.iter_mut().zip(values) {
        *slot = value?;
    }
    Some(complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(request: &Value, result: Value) -> Value {
        json!({"request_id": request["request_id"], "ok": true, "result": result})
    }

    fn connect(client: &mut Client) {
        let request = client.next(0.0, true).unwrap();
        let values = DEFAULTS
            .iter()
            .enumerate()
            .map(|(index, value)| json!({"index": index, "value": value}))
            .collect::<Vec<_>>();
        client.response(&reply(&request, json!({"values": values})));
        assert!(client.loaded);
    }

    #[test]
    fn all_defaults_are_valid() {
        for (index, value) in DEFAULTS.into_iter().enumerate() {
            assert!(valid(index, value), "parameter {index}");
        }
    }

    #[test]
    fn writes_coalesce_while_one_request_is_in_flight() {
        let mut client = Client::default();
        connect(&mut client);
        client.queue(16, 2.0);
        let first = client.next(1.0, false).unwrap();
        client.queue(16, 3.0);
        client.response(&reply(&first, json!({"value": 2.0})));
        assert_eq!(client.display(16), 3.0);
        assert_eq!(client.next(2.0, false).unwrap()["params"]["value"], 3.0);
    }

    #[test]
    fn context_catalog_allows_program_selection() {
        let mut client = Client::default();
        client.context(&json!({
            "selected_sound_id": "straight-888",
            "sounds": [
                {"id": "straight-888", "name": "Straight 888"},
                {"id": "chorale-888", "name": "Chorale 888"}
            ]
        }));
        assert_eq!(client.selected, "straight-888");
        assert_eq!(client.sounds.len(), 2);
        connect(&mut client);
        client.select("chorale-888");
        assert_eq!(
            client.next(1.0, false).unwrap()["method"],
            "plugin.select_sound"
        );
    }

    #[test]
    fn host_updates_do_not_hide_a_local_write_in_flight() {
        let mut client = Client::default();
        connect(&mut client);
        client.queue(2, 6.0);
        let request = client.next(1.0, false).unwrap();
        assert!(client.parameter_changed(2, 4.0));
        assert_eq!(client.display(2), 6.0);
        client.response(&reply(&request, json!({"value": 6.0})));
        assert_eq!(client.display(2), 6.0);
        assert!(!client.parameter_changed(2, 6.5));
    }

    #[test]
    fn context_accepts_only_the_two_host_lighting_modes() {
        assert_eq!(
            host_lighting(&json!({"host": {"lighting": "day"}})),
            Some("day")
        );
        assert_eq!(
            host_lighting(&json!({"host": {"lighting": "stage"}})),
            Some("stage")
        );
        assert_eq!(host_lighting(&json!({"host": {"lighting": "auto"}})), None);
    }

    /// Every element the surface drives has to be in the page it ships with.
    /// Rearranging the panels is easy to do and easy to do incompletely, and
    /// the failure is not a missing knob but a surface that will not start.
    #[test]
    fn packaged_surface_contains_every_element_the_surface_drives() {
        let html = include_str!("../../../package/web/play.html");
        for id in CONTROL_IDS.into_iter().chain(VIEW_IDS) {
            assert_eq!(
                html.matches(&format!("id=\"{id}\"")).count(),
                1,
                "element {id}"
            );
        }
    }

    /// The axes are written to by name as well, and they are lines rather
    /// than the circles above.
    #[test]
    fn packaged_surface_contains_the_microphone_axes() {
        let html = include_str!("../../../package/web/play.html");
        for id in ["horn-mic-axis", "drum-mic-axis"] {
            assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "{id}");
        }
    }

    #[test]
    fn packaged_surface_contains_every_parameter() {
        let html = include_str!("../../../package/web/play.html");
        for index in 0..PARAMETERS {
            assert_eq!(
                html.matches(&format!("data-parameter=\"{index}\"")).count(),
                1,
                "parameter {index}"
            );
        }
    }
}
