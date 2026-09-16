// SPDX-License-Identifier: GPL-2.0-or-later

use crate::client::{Client, PROTOCOL, host_lighting};
use js_sys::{JSON, Object};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    Document, Element, Event, HtmlInputElement, HtmlSelectElement, MessageEvent, Window,
};

const PLUGIN_ID: &str = "org.rackforge.organ";
const CONTROL_IDS: [&str; 45] = [
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
    "leslie-mode",
    "leslie-mix",
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
    "mic-distance",
    "stereo-width",
    "reflections",
    "rotor-balance",
];

struct App {
    window: Window,
    document: Document,
    origin: String,
    connected: bool,
    client: Client,
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
                "data-leslie",
                match self.client.display(16) as u8 {
                    0 => "off",
                    1 => "brake",
                    2 => "chorale",
                    _ => "tremolo",
                },
            );
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
