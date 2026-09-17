// SPDX-License-Identifier: GPL-2.0-or-later

use crate::client::{Client, PROTOCOL, host_lighting};
use crate::view;
use js_sys::{JSON, Object};
use rf_organ_dsp::{
    MainsFrequency, MicrophoneArray, MicrophonePair, Rotary, RotaryMode, StopAngle,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    Document, Element, Event, HtmlInputElement, HtmlSelectElement, MessageEvent, Window,
};

const PLUGIN_ID: &str = "org.rackforge.organ";
const CONTROL_IDS: [&str; 67] = [
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
];

/// Steps of the display's own rotor model per second.
///
/// The model is the engine's, clocked here by the browser instead of by the
/// audio device, so what the panel draws and what the cabinet does cannot
/// drift apart: same speeds, same ramps, same stop angles. A step is far
/// finer than a frame, and a frame takes as many as the clock says have
/// passed.
const VIEW_STEPS_PER_SECOND: f32 = 600.0;
/// How much of a frame the display will make up for at once. A tab that was
/// in the background comes back where it left off rather than spinning
/// through the minutes it missed.
const VIEW_MAX_FRAME_SECONDS: f64 = 0.1;
struct App {
    window: Window,
    document: Document,
    origin: String,
    connected: bool,
    client: Client,
    /// The rotor model behind the cabinet view, and the clock reading it was
    /// last advanced at.
    rotary: Box<Rotary>,
    drawn_at: Option<f64>,
}

type Shared = Rc<RefCell<App>>;

impl App {
    fn element(&self, id: &str) -> Element {
        self.document
            .get_element_by_id(id)
            .expect("static UI element")
    }

    fn render(&self) {
        let ready = self.connected && self.client.loaded;
        for (index, id) in CONTROL_IDS.into_iter().enumerate() {
            let element = self.element(id);
            if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
                if input.type_() == "checkbox" {
                    input.set_checked(self.client.display(index) == 1.0);
                } else {
                    input.set_value_as_number(self.client.display(index));
                }
                input.set_disabled(!ready);
            } else if let Some(select) = element.dyn_ref::<HtmlSelectElement>() {
                select.set_value(&format!("{}", self.client.display(index)));
                select.set_disabled(!ready);
            }
        }
        let programs = self.element("programs");
        let signature = self
            .client
            .sounds
            .iter()
            .map(|sound| format!("{}:{}", sound.id, sound.name))
            .collect::<Vec<_>>()
            .join("|");
        if programs.get_attribute("data-catalog").as_deref() != Some(&signature) {
            programs.set_text_content(None);
            for sound in &self.client.sounds {
                let option = self.document.create_element("option").expect("option");
                let _ = option.set_attribute("value", &sound.id);
                option.set_text_content(Some(&sound.name));
                let _ = programs.append_child(&option);
            }
            let _ = programs.set_attribute("data-catalog", &signature);
        }
        programs
            .unchecked_ref::<HtmlSelectElement>()
            .set_value(&self.client.selected);
        if ready {
            let _ = programs.remove_attribute("disabled");
        } else {
            let _ = programs.set_attribute("disabled", "");
        }
        self.element("status")
            .set_text_content(Some(&self.client.status));
        let _ = self
            .document
            .document_element()
            .expect("document root")
            .set_attribute(
                "data-rotary",
                match self.client.display(16) as u8 {
                    0 => "off",
                    1 => "brake",
                    2 => "chorale",
                    _ => "tremolo",
                },
            );
    }

    /// Advances the cabinet view by however much time has passed and draws
    /// where the two rotors are now.
    fn draw_cabinet(&mut self, now: f64) {
        let mode = RotaryMode::from_index(self.client.display(16) as u8).unwrap_or_default();
        if mode != self.rotary.mode() {
            self.rotary.set_mode(mode);
        }
        let _ = self
            .rotary
            .set_acceleration(self.client.display(18).clamp(0.0, 1.0) as f32);
        let _ = self.rotary.set_rotor_radii(
            self.client.display(53) as f32,
            self.client.display(54) as f32,
        );
        if let (Some(horn), Some(drum)) = (
            StopAngle::from_degrees(self.client.display(55) as f32),
            StopAngle::from_degrees(self.client.display(56) as f32),
        ) {
            let _ = self.rotary.set_stop_angles(horn, drum);
        }
        if let Some(mains) = MainsFrequency::from_index(self.client.display(57) as u8) {
            self.rotary.set_mains(mains);
        }
        let _ = self.rotary.set_microphones(MicrophoneArray {
            horn: MicrophonePair {
                distance_m: self.client.display(41) as f32,
                spacing_m: self.client.display(42) as f32,
                offset_m: self.client.display(51) as f32,
                at_the_sides: self.client.display(64) == 1.0,
            },
            drum: MicrophonePair {
                distance_m: self.client.display(61) as f32,
                spacing_m: self.client.display(62) as f32,
                offset_m: self.client.display(63) as f32,
                at_the_sides: self.client.display(65) == 1.0,
            },
            pattern: self.client.display(52) as f32,
        });

        let elapsed = self
            .drawn_at
            .map_or(0.0, |before| (now - before).max(0.0) / 1000.0)
            .min(VIEW_MAX_FRAME_SECONDS);
        self.drawn_at = Some(now);
        let steps = (elapsed * f64::from(VIEW_STEPS_PER_SECOND)) as usize;
        for _ in 0..steps {
            self.rotary.process(0.0);
        }

        let state = self.rotary.diagnostics();
        let frame = view::frame(self.rotary.geometry());
        let turn = |element: &str, degrees: f32, scale: f64| {
            let _ = self.element(element).set_attribute(
                "transform",
                &format!("rotate({degrees:.2}) scale({scale:.4})"),
            );
        };
        turn("horn-rotor", state.horn_angle_degrees, frame.horn_scale);
        turn("drum-rotor", state.drum_angle_degrees, frame.drum_scale);
        let sweep = |element: &str, radius: f64| {
            let _ = self
                .element(element)
                .set_attribute("r", &format!("{radius:.2}"));
        };
        sweep("horn-sweep", frame.horn_sweep);
        sweep("drum-sweep", frame.drum_sweep);
        for (element, at) in [
            "horn-mic-left",
            "horn-mic-right",
            "drum-mic-left",
            "drum-mic-right",
        ]
        .into_iter()
        .zip(frame.microphones)
        {
            let capsule = self.element(element);
            let _ = capsule.set_attribute("cx", &format!("{:.2}", at.0));
            let _ = capsule.set_attribute("cy", &format!("{:.2}", at.1));
        }
        for (element, end) in [
            ("horn-mic-axis", frame.horn_axis),
            ("drum-mic-axis", frame.drum_axis),
        ] {
            let axis = self.element(element);
            let _ = axis.set_attribute("x2", &format!("{:.2}", end.0));
            let _ = axis.set_attribute("y2", &format!("{:.2}", end.1));
        }
        let (x, y, width, height) = frame.view_box;
        let _ = self
            .element("rotary-view")
            .set_attribute("viewBox", &format!("{x:.2} {y:.2} {width:.2} {height:.2}"));
        self.element("mic-readout").set_text_content(Some(&format!(
            "horn {:.0} / drum {:.0} cm",
            frame.horn_distance_cm, frame.drum_distance_cm
        )));
    }

    fn send(&self, message: &Value) -> Result<(), JsValue> {
        let parent = self
            .window
            .parent()?
            .ok_or_else(|| JsValue::from_str("missing RackForge host"))?;
        parent.post_message(&JSON::parse(&message.to_string())?, &self.origin)
    }

    fn pump(&mut self, poll: bool) {
        if !self.connected {
            return;
        }
        let now = self
            .window
            .performance()
            .expect("monotonic browser clock")
            .now();
        if let Some(message) = self.client.next(now, poll)
            && self.send(&message).is_err()
        {
            self.client.loaded = false;
            self.client.status = "Could not contact RackForge…".into();
        }
        self.render();
    }
}

fn control_events(app: &Shared) -> Result<(), JsValue> {
    for (index, id) in CONTROL_IDS.into_iter().enumerate() {
        let element = app.borrow().element(id);
        let control = element.clone();
        let shared = app.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let value = if let Some(input) = control.dyn_ref::<HtmlInputElement>() {
                if input.type_() == "checkbox" {
                    if input.checked() { 1.0 } else { 0.0 }
                } else {
                    input.value_as_number()
                }
            } else {
                control
                    .unchecked_ref::<HtmlSelectElement>()
                    .value()
                    .parse()
                    .unwrap_or(f64::NAN)
            };
            let mut app = shared.borrow_mut();
            app.client.queue(index, value);
            app.pump(false);
        });
        element.add_event_listener_with_callback("input", callback.as_ref().unchecked_ref())?;
        callback.forget();
    }
    Ok(())
}

fn program_events(app: &Shared) -> Result<(), JsValue> {
    let element = app.borrow().element("programs");
    let shared = app.clone();
    let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
        let mut app = shared.borrow_mut();
        let id = app
            .element("programs")
            .unchecked_ref::<HtmlSelectElement>()
            .value();
        app.client.select(&id);
        app.pump(false);
    });
    element.add_event_listener_with_callback("change", callback.as_ref().unchecked_ref())?;
    callback.forget();
    Ok(())
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("missing document"))?;
    let origin = window.location().origin()?;
    let app = Rc::new(RefCell::new(App {
        window,
        document,
        origin,
        connected: false,
        client: Client::default(),
        rotary: Box::new(Rotary::new(VIEW_STEPS_PER_SECOND)),
        drawn_at: None,
    }));
    control_events(&app)?;
    program_events(&app)?;

    let messages = app.clone();
    let callback = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let mut app = messages.borrow_mut();
        let from_parent = app
            .window
            .parent()
            .ok()
            .flatten()
            .zip(event.source())
            .is_some_and(|(parent, source)| Object::is(parent.as_ref(), source.as_ref()));
        if !from_parent || event.origin() != app.origin {
            return;
        }
        let Some(text) = JSON::stringify(&event.data())
            .ok()
            .and_then(|json| json.as_string())
        else {
            return;
        };
        if text.len() > 262_144 {
            return;
        }
        let Ok(message) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        if message["protocol"] != PROTOCOL {
            return;
        }
        match message["kind"].as_str() {
            Some("context") if message["instance"]["plugin_id"] == PLUGIN_ID => {
                app.connected = true;
                app.client.context(&message["instance"]);
                if let Some(lighting) = host_lighting(&message) {
                    let _ = app
                        .document
                        .document_element()
                        .expect("document root")
                        .set_attribute("data-lighting", lighting);
                }
                app.pump(true);
            }
            Some("response") => {
                app.client.response(&message);
                app.pump(false);
            }
            Some("parameter_changed") => {
                let index = message["parameter_index"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok());
                let value = message["value"].as_f64();
                if let (Some(index), Some(value)) = (index, value)
                    && app.client.parameter_changed(index, value)
                {
                    app.render();
                }
            }
            _ => {}
        }
    });
    app.borrow()
        .window
        .add_event_listener_with_callback("message", callback.as_ref().unchecked_ref())?;
    callback.forget();

    // The cabinet view redraws with the browser, not with the message pump:
    // a rotor turns continuously and a twice-a-second refresh would show it
    // stepping.
    let frames = app.clone();
    let next = Rc::new(RefCell::new(None::<Closure<dyn FnMut(f64)>>));
    let again = next.clone();
    *next.borrow_mut() = Some(Closure::<dyn FnMut(f64)>::new(move |now: f64| {
        {
            let mut app = frames.borrow_mut();
            app.draw_cabinet(now);
        }
        if let Some(callback) = again.borrow().as_ref() {
            let _ = frames
                .borrow()
                .window
                .request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    if let Some(callback) = next.borrow().as_ref() {
        app.borrow()
            .window
            .request_animation_frame(callback.as_ref().unchecked_ref())?;
    }

    let timer = app.clone();
    let callback = Closure::<dyn FnMut()>::new(move || timer.borrow_mut().pump(true));
    app.borrow()
        .window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            500,
        )?;
    callback.forget();
    app.borrow().render();
    app.borrow()
        .send(&json!({"protocol": PROTOCOL, "kind": "ready"}))
}
