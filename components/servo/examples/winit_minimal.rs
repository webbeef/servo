/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;

use dpi::PhysicalPosition;
use euclid::{Point2D, Scale, Size2D};
use log::info;
use servo::config::prefs::Preferences;
use servo::{
    InputEvent, MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, RenderingContext, Servo, ServoBuilder, TouchEvent,
    TouchEventType, TouchId, WebView, WebViewBuilder, WindowRenderingContext,
};
use tracing::warn;
use url::Url;
use webrender_api::ScrollLocation;
use webrender_api::units::{DeviceIntPoint, DevicePixel, LayoutVector2D};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::EventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    let event_loop = EventLoop::with_user_event()
        .build()
        .expect("Failed to create EventLoop");
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app)?;

    if let App::Running(state) = app {
        if let Some(state) = Rc::into_inner(state) {
            state.servo.deinit();
        }
    }

    Ok(())
}

struct AppState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
    webview_relative_mouse_point: Cell<Point2D<f32, DevicePixel>>,
}

impl ::servo::WebViewDelegate for AppState {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.window.request_redraw();
    }

    fn request_open_auxiliary_webview(&self, parent_webview: WebView) -> Option<WebView> {
        let webview = WebViewBuilder::new_auxiliary(&self.servo)
            .hidpi_scale_factor(Scale::new(self.window.scale_factor() as f32))
            .delegate(parent_webview.delegate())
            .build();
        webview.focus_and_raise_to_top(true);

        self.webviews.borrow_mut().push(webview.clone());
        Some(webview)
    }

    fn notify_history_changed(&self, webview: WebView, entries: Vec<Url>, _current: usize) {
        info!(
            "History changed for webview with hidpi={:?} ({})",
            webview.hidpi_scale_factor(),
            entries.last().unwrap()
        );
    }
}

enum App {
    Initial(Waker),
    Running(Rc<AppState>),
}

impl App {
    fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self::Initial(Waker::new(event_loop))
    }
}

impl ApplicationHandler<WakerEvent> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Self::Initial(waker) = self {
            let display_handle = event_loop
                .display_handle()
                .expect("Failed to get display handle");
            let window = event_loop
                .create_window(Window::default_attributes())
                .expect("Failed to create winit Window");
            let window_handle = window.window_handle().expect("Failed to get window handle");

            let rendering_context = Rc::new(
                WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                    .expect("Could not create RenderingContext for window."),
            );

            let _ = rendering_context.make_current();

            let preferences = Preferences {
                viewport_meta_enabled: true,
                ..Default::default()
            };

            let servo = ServoBuilder::new(rendering_context.clone())
                .event_loop_waker(Box::new(waker.clone()))
                .preferences(preferences)
                .build();
            servo.setup_logging();

            let app_state = Rc::new(AppState {
                window,
                servo,
                rendering_context,
                webviews: Default::default(),
                webview_relative_mouse_point: Cell::new(Point2D::zero()),
            });

            // Make a new WebView and assign the `AppState` as the delegate.
            let url = Url::parse(
                &std::env::args()
                    .nth(1)
                    .or(Some(
                        "https://demo.servo.org/experiments/twgl-tunnel/".to_owned(),
                    ))
                    .unwrap(),
            )
            .expect("Invalid url");

            info!(
                "Creating initial webview with scale factor {}",
                app_state.window.scale_factor()
            );

            let webview = WebViewBuilder::new(&app_state.servo)
                .url(url)
                .hidpi_scale_factor(Scale::new(app_state.window.scale_factor() as f32))
                .delegate(app_state.clone())
                .build();

            webview.focus_and_raise_to_top(true);

            app_state.webviews.borrow_mut().push(webview);
            *self = Self::Running(app_state);
        }
    }

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, _event: WakerEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
        }

        match event {
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                info!("Scale factor changed to {scale_factor}");
                let scale = Scale::new(scale_factor as _);
                if let Self::Running(state) = self {
                    for webview in &*state.webviews.borrow() {
                        webview.set_hidpi_scale_factor(scale);
                    }
                }
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                if let Self::Running(state) = self {
                    state.webviews.borrow().last().unwrap().paint();
                    state.rendering_context.present();
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        let moved_by = match delta {
                            MouseScrollDelta::LineDelta(horizontal, vertical) => {
                                LayoutVector2D::new(20. * horizontal, -20. * vertical)
                            },
                            MouseScrollDelta::PixelDelta(pos) => {
                                LayoutVector2D::new(pos.x as f32, -pos.y as f32)
                            },
                        };
                        webview.notify_scroll_event(
                            ScrollLocation::Delta(moved_by),
                            DeviceIntPoint::new(10, 10),
                        );
                    }
                }
            },
            WindowEvent::MouseInput { state, button, .. } => {
                let action = state;
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        let mouse_button = match &button {
                            MouseButton::Left => ServoMouseButton::Left,
                            MouseButton::Right => ServoMouseButton::Right,
                            MouseButton::Middle => ServoMouseButton::Middle,
                            MouseButton::Back => ServoMouseButton::Back,
                            MouseButton::Forward => ServoMouseButton::Forward,
                            MouseButton::Other(value) => ServoMouseButton::Other(*value),
                        };

                        let point = state.webview_relative_mouse_point.get();
                        // `point` can be outside viewport, such as at toolbar with negative y-coordinate.
                        if !webview.rect().contains(point) {
                            return;
                        }
                        let action = match action {
                            ElementState::Pressed => MouseButtonAction::Down,
                            ElementState::Released => MouseButtonAction::Up,
                        };

                        webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                            action,
                            mouse_button,
                            point,
                        )));
                    }
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        let point = winit_position_to_euclid_point(position).to_f32();
                        let previous_point = state.webview_relative_mouse_point.get();
                        if webview.rect().contains(point) {
                            webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
                                point,
                            )));
                        } else if webview.rect().contains(previous_point) {
                            webview.notify_input_event(InputEvent::MouseLeftViewport(
                                MouseLeftViewportEvent::default(),
                            ));
                        }

                        state.webview_relative_mouse_point.set(point);
                    }
                }
            },
            WindowEvent::Touch(touch) => {
                if let Self::Running(state) = self {
                    info!("Touch: {:?}", touch);
                    if let Some(webview) = state.webviews.borrow().last() {
                        webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                            winit_phase_to_touch_event_type(touch.phase),
                            TouchId(touch.id as i32),
                            Point2D::new(touch.location.x as f32, touch.location.y as f32),
                        )));
                    }
                }
            },
            WindowEvent::KeyboardInput { event, .. } => {
                // When pressing 'q' close the latest WebView, then show the next most recently
                // opened view or quit when none are left.
                if event.logical_key.to_text() == Some("q") {
                    if let Self::Running(state) = self {
                        let _ = state.webviews.borrow_mut().pop();
                        match state.webviews.borrow().last() {
                            Some(last) => last.show(true),
                            None => event_loop.exit(),
                        }
                    }
                }
            },
            WindowEvent::Resized(new_size) => {
                if let Self::Running(state) = self {
                    if let Some(webview) = state.webviews.borrow().last() {
                        let mut rect = webview.rect();
                        rect.set_size(winit_size_to_euclid_size(new_size).to_f32());
                        webview.move_resize(rect);
                        webview.resize(new_size);
                    }
                }
            },
            _ => (),
        }
    }
}

#[derive(Clone)]
struct Waker(winit::event_loop::EventLoopProxy<WakerEvent>);
#[derive(Debug)]
struct WakerEvent;

impl Waker {
    fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self(event_loop.create_proxy())
    }
}

impl embedder_traits::EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn embedder_traits::EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }

    fn wake(&self) {
        if let Err(error) = self.0.send_event(WakerEvent) {
            warn!(?error, "Failed to wake event loop");
        }
    }
}

fn winit_size_to_euclid_size<T>(size: PhysicalSize<T>) -> Size2D<T, DevicePixel> {
    Size2D::new(size.width, size.height)
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
