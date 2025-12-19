// SPDX-License-Identifier: AGPL-3.0-or-later

//! Manages a single "OS level" window state.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::rc::Rc;

use euclid::{Point2D, Rect, Scale};
use servo::{
    DevicePixel, DevicePoint, InputEvent, InputEventId, InputEventResult, KeyboardEvent,
    MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent, MouseLeftViewportEvent,
    MouseMoveEvent, RenderingContext, Theme, TouchEvent, TouchEventType, TouchId, WebView,
    WebViewId, WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use url::Url;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::keyboard::ModifiersState;
use winit::window::Window;

use crate::keyutils::keyboard_event_from_winit;
use crate::touch_event_simulator::TouchEventSimulator;

/// Commands that the shell can queue for later processing.
#[derive(Clone, Debug)]
pub(crate) enum UserInterfaceCommand {
    /// Open a new browser window.
    NewWindow(Option<Url>, Option<String>),
}

/// A unique identifier for a browser window, derived from winit's WindowId.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct BrowserWindowId(u64);

impl From<winit::window::WindowId> for BrowserWindowId {
    fn from(id: winit::window::WindowId) -> Self {
        Self(id.into())
    }
}

/// A keyboard event sent to system webview, pending forwarding decision.
struct PendingKeyboardEvent {
    /// The original keyboard event for potential forwarding.
    keyboard_event: KeyboardEvent,
    /// The focused webview at time of event (target for forwarding).
    target_webview_id: Option<WebViewId>,
}

/// Per-window state. Each OS-level window has its own BrowserWindow instance.
pub(crate) struct BrowserWindow {
    pub(crate) window: Window,
    pub(crate) rendering_context: Rc<WindowRenderingContext>,
    pub(crate) webviews: RefCell<BTreeMap<WebViewId, WebView>>,
    webview_relative_mouse_point: Cell<Point2D<f32, DevicePixel>>,
    modifiers_state: Cell<ModifiersState>,
    /// The WebViewId of the currently focused webview (for keyboard input routing).
    /// Updated via notify_focus_changed delegate callback.
    pub(crate) focused_webview_id: Cell<Option<WebViewId>>,
    /// The WebViewId of the system UI
    pub(crate) system_webview_id: Cell<Option<WebViewId>>,
    /// Commands queued for later processing.
    pending_commands: RefCell<Vec<UserInterfaceCommand>>,
    /// Keyboard events awaiting callback from system webview to decide on forwarding.
    pending_keyboard_events: RefCell<HashMap<InputEventId, PendingKeyboardEvent>>,
    /// Simulates touch events from mouse when mobile simulation is enabled.
    touch_event_simulator: Option<TouchEventSimulator>,
}

impl BrowserWindow {
    pub(crate) fn new(
        window: Window,
        rendering_context: Rc<WindowRenderingContext>,
        simulate_touch: bool,
    ) -> Self {
        Self {
            window,
            rendering_context,
            webviews: Default::default(),
            webview_relative_mouse_point: Cell::new(Point2D::zero()),
            modifiers_state: Cell::new(ModifiersState::empty()),
            focused_webview_id: Cell::new(None),
            system_webview_id: Cell::new(None),
            pending_commands: Default::default(),
            pending_keyboard_events: Default::default(),
            touch_event_simulator: simulate_touch.then(Default::default),
        }
    }

    pub(crate) fn id(&self) -> BrowserWindowId {
        self.window.id().into()
    }

    /// Get the focused webview, or fall back to the first webview.
    pub(crate) fn focused_or_first_webview(&self) -> Option<WebView> {
        let webviews = self.webviews.borrow();
        let result = if let Some(focused_id) = self.focused_webview_id.get() {
            webviews.get(&focused_id)
        } else {
            // Fall back to first webview
            webviews.values().next()
        };
        result.cloned()
    }

    pub(crate) fn first_webview(&self) -> Option<WebView> {
        let webviews = self.webviews.borrow();
        webviews.values().next().cloned()
    }

    /// Get the system webview (browserhtml shell UI).
    pub(crate) fn system_webview(&self) -> Option<WebView> {
        let system_id = self.system_webview_id.get()?;
        self.webviews.borrow().get(&system_id).cloned()
    }

    /// Get a webview by its ID.
    pub(crate) fn webview(&self, id: WebViewId) -> Option<WebView> {
        self.webviews.borrow().get(&id).cloned()
    }

    /// Helper function to handle a click
    pub(crate) fn handle_mouse_button_event(
        &self,
        webview: &WebView,
        button: MouseButton,
        action: ElementState,
    ) {
        // `point` can be outside viewport, such as at toolbar with negative y-coordinate.
        let point = self.webview_relative_mouse_point.get();
        let webview_rect: Rect<_, _> = webview.size().into();
        if !webview_rect.contains(point) {
            return;
        }

        // Check for touch simulation first
        if self
            .touch_event_simulator
            .as_ref()
            .is_some_and(|sim| sim.maybe_consume_mouse_button_event(webview, button, action, point))
        {
            return;
        }

        let mouse_button = match &button {
            MouseButton::Left => ServoMouseButton::Left,
            MouseButton::Right => ServoMouseButton::Right,
            MouseButton::Middle => ServoMouseButton::Middle,
            MouseButton::Back => ServoMouseButton::Back,
            MouseButton::Forward => ServoMouseButton::Forward,
            MouseButton::Other(value) => ServoMouseButton::Other(*value),
        };

        let action = match action {
            ElementState::Pressed => MouseButtonAction::Down,
            ElementState::Released => MouseButtonAction::Up,
        };

        webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
            action,
            mouse_button,
            point.into(),
        )));
    }

    /// Helper function to handle mouse move events.
    pub(crate) fn handle_mouse_move_event(
        &self,
        webview: &WebView,
        position: PhysicalPosition<f64>,
    ) {
        let point = winit_position_to_euclid_point(position).to_f32();

        let previous_point = self.webview_relative_mouse_point.get();
        self.webview_relative_mouse_point.set(point);

        let webview_rect: Rect<_, _> = webview.size().into();
        if !webview_rect.contains(point) {
            if webview_rect.contains(previous_point) {
                webview.notify_input_event(InputEvent::MouseLeftViewport(
                    MouseLeftViewportEvent::default(),
                ));
            }
            return;
        }

        // Check for touch simulation first
        if self
            .touch_event_simulator
            .as_ref()
            .is_some_and(|sim| sim.maybe_consume_mouse_move_event(webview, point))
        {
            return;
        }

        webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point.into())));
    }

    /// Queue a command for later processing.
    pub(crate) fn queue_command(&self, command: UserInterfaceCommand) {
        self.pending_commands.borrow_mut().push(command);
    }

    /// Take all pending commands.
    pub(crate) fn take_pending_commands(&self) -> Vec<UserInterfaceCommand> {
        self.pending_commands.borrow_mut().drain(..).collect()
    }

    /// Handle a keyboard event result callback. If the event was sent to the system
    /// webview for interception, this will forward it to the target webview if not prevented.
    pub(crate) fn handle_keyboard_event_result(
        &self,
        event_id: InputEventId,
        result: InputEventResult,
    ) {
        let pending = self.pending_keyboard_events.borrow_mut().remove(&event_id);
        let Some(pending_event) = pending else {
            return;
        };

        // If system webview prevented or consumed the event, don't forward
        if result.intersects(InputEventResult::DefaultPrevented | InputEventResult::Consumed) {
            return;
        }

        // Forward to target webview if it exists and differs from system
        let Some(target_id) = pending_event.target_webview_id else {
            return;
        };

        if Some(target_id) == self.system_webview_id.get() {
            return;
        }

        if let Some(target_webview) = self.webview(target_id) {
            target_webview.notify_input_event(InputEvent::Keyboard(pending_event.keyboard_event));
        }
    }

    pub(crate) fn contains_webview(&self, id: WebViewId) -> bool {
        self.webviews.borrow().contains_key(&id)
    }

    pub(crate) fn window_event(&self, event: WindowEvent) {
        match event {
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let scale = Scale::new(scale_factor as _);
                for webview in self.webviews.borrow().values() {
                    webview.set_hidpi_scale_factor(scale);
                }
            },
            WindowEvent::RedrawRequested => {
                if let Some(webview) = self.first_webview() {
                    webview.paint();
                    self.rendering_context.present();
                }
            },
            WindowEvent::MouseInput { state, button, .. } => {
                // Send mouse events to the system webview (browserhtml shell) first.
                // The shell's DOM hit testing will determine if events should be
                // forwarded to embedded webviews.
                if let Some(webview) = self.system_webview().or_else(|| self.first_webview()) {
                    self.handle_mouse_button_event(&webview, button, state);
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                // Send mouse events to the system webview (browserhtml shell) first.
                // The shell's DOM hit testing will determine if events should be
                // forwarded to embedded webviews.
                if let Some(webview) = self.system_webview().or_else(|| self.first_webview()) {
                    self.handle_mouse_move_event(&webview, position);
                }
            },
            WindowEvent::CursorLeft { .. } => {
                if let Some(webview) = self.system_webview().or_else(|| self.first_webview()) {
                    let webview_rect: Rect<_, _> = webview.size().into();
                    if webview_rect.contains(self.webview_relative_mouse_point.get()) {
                        webview.notify_input_event(InputEvent::MouseLeftViewport(
                            MouseLeftViewportEvent::default(),
                        ));
                    }
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                // Send wheel events to the system webview (browserhtml shell) first.
                // The shell's DOM hit testing will determine if events should be
                // forwarded to embedded webviews.
                if let Some(webview) = self.system_webview().or_else(|| self.first_webview()) {
                    let (delta_x, delta_y, mode) = match delta {
                        MouseScrollDelta::LineDelta(dx, dy) => (
                            (dx * 76.0) as f64,
                            (dy * 76.0) as f64,
                            WheelMode::DeltaPixel,
                        ),
                        MouseScrollDelta::PixelDelta(delta) => {
                            (delta.x * 6.0, delta.y * 6.0, WheelMode::DeltaPixel)
                        },
                    };

                    let point = self.webview_relative_mouse_point.get();
                    webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                        WheelDelta {
                            x: delta_x,
                            y: delta_y,
                            z: 0.0,
                            mode,
                        },
                        point.into(),
                    )));
                }
            },
            WindowEvent::Touch(touch) => {
                // Send touch events to the system webview (browserhtml shell) first.
                // The shell's DOM hit testing will determine if events should be
                // forwarded to embedded webviews.
                if let Some(webview) = self.system_webview().or_else(|| self.first_webview()) {
                    webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                        winit_phase_to_touch_event_type(touch.phase),
                        TouchId(touch.id as i32),
                        DevicePoint::new(touch.location.x as f32, touch.location.y as f32).into(),
                    )));
                }
            },
            WindowEvent::PinchGesture { delta, .. } => {
                if let Some(webview) = self.first_webview() {
                    webview.pinch_zoom(delta as f32 + 1.0, self.webview_relative_mouse_point.get());
                }
            },
            WindowEvent::ThemeChanged(theme) => {
                for webview in self.webviews.borrow().values() {
                    webview.notify_theme_change(match theme {
                        winit::window::Theme::Light => Theme::Light,
                        winit::window::Theme::Dark => Theme::Dark,
                    });
                }
            },
            WindowEvent::Resized(new_size) => {
                if let Some(webview) = self.first_webview() {
                    webview.resize(new_size);
                }
            },
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers_state.set(modifiers.state());
            },
            WindowEvent::KeyboardInput { event, .. } => {
                let keyboard_event = keyboard_event_from_winit(&event, self.modifiers_state.get());

                let focused_webview_id = self.focused_webview_id.get();
                let system_webview_id = self.system_webview_id.get();

                // Check if we need parent-first forwarding flow
                let needs_forwarding = match (focused_webview_id, system_webview_id) {
                    (Some(focused), Some(system)) => focused != system,
                    _ => false,
                };

                if needs_forwarding {
                    // Send to system webview first, store for potential forwarding
                    if let Some(system_webview) = self.system_webview() {
                        let event_id = system_webview
                            .notify_input_event(InputEvent::Keyboard(keyboard_event.clone()));

                        self.pending_keyboard_events.borrow_mut().insert(
                            event_id,
                            PendingKeyboardEvent {
                                keyboard_event,
                                target_webview_id: focused_webview_id,
                            },
                        );
                    }
                } else {
                    // No forwarding needed - send directly to focused/first webview
                    if let Some(webview) = self.focused_or_first_webview() {
                        webview.notify_input_event(InputEvent::Keyboard(keyboard_event));
                    }
                }
            },
            _ => (),
        }
    }
}

fn winit_position_to_euclid_point<T>(position: PhysicalPosition<T>) -> Point2D<T, DevicePixel> {
    Point2D::new(position.x, position.y)
}

fn winit_phase_to_touch_event_type(phase: TouchPhase) -> TouchEventType {
    match phase {
        TouchPhase::Started => TouchEventType::Down,
        TouchPhase::Moved => TouchEventType::Move,
        TouchPhase::Ended => TouchEventType::Up,
        TouchPhase::Cancelled => TouchEventType::Cancel,
    }
}
