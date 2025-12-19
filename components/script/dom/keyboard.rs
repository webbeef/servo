/* SPDX Id: AGPL-3.0-or-later */

//! The `Keyboard` interface provides virtual keyboard input injection.
//! This is a Servo-specific, non-standard API for privileged pages.

use constellation_traits::ScriptToConstellationMessage;
use dom_struct::dom_struct;
use embedder_traits::{
    ImeEvent, InputEvent, InputEventAndId, KeyboardEvent as EmbedderKeyboardEvent,
};
use keyboard_types::{
    Code, CompositionEvent, CompositionState, Key, KeyState, Location, Modifiers,
};

use crate::dom::bindings::codegen::Bindings::KeyboardBinding::{
    KeyboardMethods, ServoCompositionEventInit, ServoKeyboardEventInit,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::reflector::{DomGlobal, Reflector, reflect_dom_object};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::globalscope::GlobalScope;
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct Keyboard {
    reflector_: Reflector,
}

impl Keyboard {
    fn new_inherited() -> Keyboard {
        Keyboard {
            reflector_: Reflector::new(),
        }
    }

    pub(crate) fn new(global: &GlobalScope, can_gc: CanGc) -> DomRoot<Keyboard> {
        reflect_dom_object(Box::new(Keyboard::new_inherited()), global, can_gc)
    }

    fn send_to_active_ime(&self, event: InputEvent) {
        let global = self.global();
        let event_and_id = InputEventAndId::from(event);
        let _ = global.script_to_constellation_chan().send(
            ScriptToConstellationMessage::InjectInputToActiveIme(event_and_id),
        );
    }

    fn parse_key_state(state: &DOMString) -> Fallible<KeyState> {
        match &*state.str() {
            "down" => Ok(KeyState::Down),
            "up" => Ok(KeyState::Up),
            _ => Err(Error::Type(format!(
                "Invalid key state '{}': expected 'down' or 'up'",
                state
            ))),
        }
    }

    fn parse_key(key: &DOMString) -> Key {
        let key_str = key.str();
        // Try to parse as a named key first
        if key_str.len() > 1 {
            // This could be a named key like "Backspace", "Enter", etc.
            match &*key_str {
                "Backspace" => Key::Named(keyboard_types::NamedKey::Backspace),
                "Tab" => Key::Named(keyboard_types::NamedKey::Tab),
                "Enter" => Key::Named(keyboard_types::NamedKey::Enter),
                "Escape" => Key::Named(keyboard_types::NamedKey::Escape),
                "Delete" => Key::Named(keyboard_types::NamedKey::Delete),
                "ArrowUp" => Key::Named(keyboard_types::NamedKey::ArrowUp),
                "ArrowDown" => Key::Named(keyboard_types::NamedKey::ArrowDown),
                "ArrowLeft" => Key::Named(keyboard_types::NamedKey::ArrowLeft),
                "ArrowRight" => Key::Named(keyboard_types::NamedKey::ArrowRight),
                "Home" => Key::Named(keyboard_types::NamedKey::Home),
                "End" => Key::Named(keyboard_types::NamedKey::End),
                "PageUp" => Key::Named(keyboard_types::NamedKey::PageUp),
                "PageDown" => Key::Named(keyboard_types::NamedKey::PageDown),
                "Space" => Key::Character(" ".to_string()),
                _ => Key::Character(key_str.to_string()),
            }
        } else {
            Key::Character(key_str.to_string())
        }
    }

    fn parse_code(code: &DOMString) -> Code {
        let code_str = code.str();
        if code_str.is_empty() {
            Code::Unidentified
        } else {
            // Try to parse the code string
            code_str.to_string().parse().unwrap_or(Code::Unidentified)
        }
    }

    fn parse_location(location: u32) -> Location {
        match location {
            0 => Location::Standard,
            1 => Location::Left,
            2 => Location::Right,
            3 => Location::Numpad,
            _ => Location::Standard,
        }
    }

    fn build_modifiers(init: &ServoKeyboardEventInit) -> Modifiers {
        let mut modifiers = Modifiers::empty();
        if init.shift {
            modifiers.insert(Modifiers::SHIFT);
        }
        if init.ctrl {
            modifiers.insert(Modifiers::CONTROL);
        }
        if init.alt {
            modifiers.insert(Modifiers::ALT);
        }
        if init.meta {
            modifiers.insert(Modifiers::META);
        }
        modifiers
    }

    fn parse_composition_state(state: &DOMString) -> Fallible<CompositionState> {
        match &*state.str() {
            "start" => Ok(CompositionState::Start),
            "update" => Ok(CompositionState::Update),
            "end" => Ok(CompositionState::End),
            _ => Err(Error::Type(format!(
                "Invalid composition state '{}': expected 'start', 'update', or 'end'",
                state
            ))),
        }
    }
}

impl KeyboardMethods<crate::DomTypeHolder> for Keyboard {
    /// Send a keyboard event (keydown/keyup) to the active IME input.
    fn SendKeyboardEvent(&self, init: &ServoKeyboardEventInit) -> Fallible<()> {
        let state = Self::parse_key_state(&init.state)?;
        let key = Self::parse_key(&init.key);
        let code = Self::parse_code(&init.code);
        let location = Self::parse_location(init.location);
        let modifiers = Self::build_modifiers(init);

        let keyboard_event = EmbedderKeyboardEvent::new_without_event(
            state,
            key,
            code,
            location,
            modifiers,
            init.repeat,
            init.isComposing,
        );

        let event = InputEvent::Keyboard(keyboard_event);
        self.send_to_active_ime(event);
        Ok(())
    }

    /// Send a composition event to the active IME input.
    fn SendCompositionEvent(&self, init: &ServoCompositionEventInit) -> Fallible<()> {
        let state = Self::parse_composition_state(&init.state)?;

        let composition_event = CompositionEvent {
            state,
            data: init.data.to_string(),
        };

        let event = InputEvent::Ime(ImeEvent::Composition(composition_event));
        self.send_to_active_ime(event);
        Ok(())
    }
}
