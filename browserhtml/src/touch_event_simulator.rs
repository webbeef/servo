/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Simulates touch events from mouse input for mobile development.
//! Copied from servoshell/desktop/headed_window.rs

use std::cell::Cell;

use euclid::Point2D;
use servo::{DevicePixel, InputEvent, TouchEvent, TouchEventType, TouchId, WebView};
use winit::event::{ElementState, MouseButton};

/// Simulates touch events from mouse input when mobile simulation is enabled.
#[derive(Default)]
pub struct TouchEventSimulator {
    left_mouse_button_down: Cell<bool>,
}

impl TouchEventSimulator {
    /// Convert mouse button event to touch. Returns true if consumed.
    pub fn maybe_consume_mouse_button_event(
        &self,
        webview: &WebView,
        button: MouseButton,
        action: ElementState,
        point: Point2D<f32, DevicePixel>,
    ) -> bool {
        if button != MouseButton::Left {
            return false;
        }

        if action == ElementState::Pressed && !self.left_mouse_button_down.get() {
            webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                TouchEventType::Down,
                TouchId(0),
                point.into(),
            )));
            self.left_mouse_button_down.set(true);
        } else if action == ElementState::Released {
            webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                TouchEventType::Up,
                TouchId(0),
                point.into(),
            )));
            self.left_mouse_button_down.set(false);
        }
        true
    }

    /// Convert mouse move to touch move. Returns true if consumed.
    pub fn maybe_consume_mouse_move_event(
        &self,
        webview: &WebView,
        point: Point2D<f32, DevicePixel>,
    ) -> bool {
        if !self.left_mouse_button_down.get() {
            return false;
        }
        webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
            TouchEventType::Move,
            TouchId(0),
            point.into(),
        )));
        true
    }
}
