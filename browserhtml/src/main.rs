// SPDX-License-Identifier: AGPL-3.0-or-later

mod browser_window;
#[cfg(feature = "global-hotkeys")]
mod hotkeys;
mod keyutils;
mod prefs;
mod protocols;
mod resources;
mod touch_event_simulator;
#[cfg(feature = "status-tray")]
mod tray;
mod vhost;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;

use euclid::Scale;
#[cfg(feature = "global-hotkeys")]
use hotkeys::HotkeyManager;
use log::{error, info, warn};
use protocols::resource::ResourceProtocolHandler;
use servo::prefs::get_embedder_pref;
use servo::protocol_handler::ProtocolRegistry;
use servo::{
    AllowOrDenyRequest, BrowsingContextId, ConsoleLogLevel, CreateNewWebViewRequest, Cursor,
    EmbedderControl, EmbedderControlId, InputEventId, InputEventResult, Notification, Opts,
    PipelineId, PrefValue, Preferences, RenderingContext, Servo, ServoBuilder, ServoDelegate,
    ServoError, WebView, WebViewBuilder, WebViewDelegate, WebViewId, WindowRenderingContext,
};
#[cfg(feature = "status-tray")]
use tray::Tray;
#[cfg(feature = "status-tray")]
use tray_icon::menu::MenuId;
use url::Url;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{CursorIcon, WindowAttributes, WindowLevel};

use crate::browser_window::{BrowserWindow, BrowserWindowId, UserInterfaceCommand};
use crate::prefs::enable_experimental_prefs;

/// Returns the profile name, or "default" if none is set.
fn profile_name() -> String {
    std::env::var("BROWSERHTML_PROFILE").unwrap_or_else(|_| "default".into())
}

/// Get the configuration directory for browserhtml.
/// Returns None if the directory cannot be determined.
fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::data_dir().map(|d| d.join("BrowserHTML").join(profile_name()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::config_dir().map(|d| d.join("browserhtml").join(profile_name()))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");
    crate::resources::init();

    // Load browserhtml preferences from config directory
    if let Some(prefs_path) = config_dir().map(|d| d.join("prefs.json")) {
        if let Err(e) = prefs::load(&prefs_path) {
            log::warn!("Failed to load browserhtml preferences: {}", e);
        }
    }

    // Check if this is the main process or a content process.
    // content processes get a "--content-process /tmp/.tmpTwE4hh/socket" parameter.
    let args: Vec<_> = std::env::args().collect();
    if let Some(token) = args
        .windows(2)
        .find(|arg| arg[0] == "--content-process")
        .map(|arg| arg[1].clone())
    {
        servo::run_content_process(token);
        return Ok(());
    }

    let event_loop = EventLoop::with_user_event()
        .build()
        .expect("Failed to create EventLoop");
    let mut app = App::new(&event_loop);
    Ok(event_loop.run_app(&mut app)?)
}

/// Global application state that holds all windows and the shared Servo instance.
struct AppState {
    servo: Servo,
    windows: RefCell<HashMap<BrowserWindowId, Rc<BrowserWindow>>>,
    focused_window: RefCell<Option<Rc<BrowserWindow>>>,
    #[cfg(feature = "status-tray")]
    tray: Option<Tray>,
    exit_requested: Cell<bool>,
    #[cfg(feature = "global-hotkeys")]
    hotkey_manager: Option<HotkeyManager>,
}

impl AppState {
    /// Look up a window by its ID.
    fn window(&self, id: BrowserWindowId) -> Option<Rc<BrowserWindow>> {
        self.windows.borrow().get(&id).cloned()
    }

    /// Explicitly shut down Servo. This is needed because there's a reference
    /// cycle between AppState and Servo (via the delegate), which prevents
    /// the normal Drop-based cleanup from running. We need to:
    /// 1. Clear all windows (which hold WebViews that reference Servo)
    /// 2. Set a dummy delegate to break the AppState <-> Servo cycle
    fn shutdown(&self) {
        // Clear focused_window reference
        *self.focused_window.borrow_mut() = None;
        // Clear windows - this drops WebViews which hold Servo references
        self.windows.borrow_mut().clear();
        // Break the delegate cycle
        self.servo.set_delegate(Rc::new(ShutdownDelegate));
    }

    /// Find the window containing the given WebView.
    fn window_for_webview_id(&self, webview_id: WebViewId) -> Option<Rc<BrowserWindow>> {
        for window in self.windows.borrow().values() {
            if window.contains_webview(webview_id) {
                return Some(window.clone());
            }
        }
        None
    }

    /// Open a new browser window.
    fn open_window(
        self: &Rc<Self>,
        event_loop: &ActiveEventLoop,
        url: Url,
        features: Option<String>,
    ) -> Rc<BrowserWindow> {
        let mut with_decorations = true;
        let mut with_resizable = true;
        let mut with_window_level = WindowLevel::Normal;

        for feature in features.unwrap_or_default().split(',') {
            if feature == "noresize" {
                with_resizable = false;
            } else if feature == "notitle" {
                with_decorations = false;
            } else if feature == "ontop" {
                with_window_level = WindowLevel::AlwaysOnTop;
            } else if feature == "onbottom" {
                with_window_level = WindowLevel::AlwaysOnBottom;
            }
        }

        let display_handle = event_loop
            .display_handle()
            .expect("Failed to get display handle");

        // Build window attributes
        let mut window_attributes = WindowAttributes::default();
        window_attributes = window_attributes
            .with_transparent(true)
            .with_decorations(with_decorations)
            .with_resizable(with_resizable)
            .with_window_level(with_window_level);

        // On macOS, disable shadow for frameless windows to avoid double border effect
        #[cfg(target_os = "macos")]
        if !with_decorations {
            window_attributes = window_attributes.with_has_shadow(false);
        }

        // In Mobile simulation mode, start with a smaller window size
        let simulate_touch = matches!(
            get_embedder_pref("browserhtml.mobile_simulation"),
            Some(PrefValue::Bool(true))
        );

        if simulate_touch {
            window_attributes = window_attributes.with_inner_size(winit::dpi::PhysicalSize {
                width: 650,
                height: 1000,
            });
        }

        let window = event_loop
            .create_window(window_attributes)
            .expect("Failed to create winit Window");
        let window_handle = window.window_handle().expect("Failed to get window handle");

        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                .expect("Could not create RenderingContext for window."),
        );

        let _ = rendering_context.make_current();

        // Check if mobile simulation is enabled
        let simulate_touch = matches!(
            get_embedder_pref("browserhtml.mobile_simulation"),
            Some(PrefValue::Bool(true))
        );

        let browser_window = Rc::new(BrowserWindow::new(
            window,
            rendering_context.clone(),
            simulate_touch,
        ));

        let id = browser_window.id();
        self.windows.borrow_mut().insert(id, browser_window.clone());

        // Create the initial WebView for this window
        let webview = WebViewBuilder::new(&self.servo, rendering_context)
            .url(url)
            .hidpi_scale_factor(Scale::new(browser_window.window.scale_factor() as f32))
            .delegate(self.clone())
            .build();

        let webview_id = webview.id();
        browser_window
            .webviews
            .borrow_mut()
            .insert(webview_id, webview);

        // Set this as the system webview (the browserhtml shell UI)
        browser_window.system_webview_id.set(Some(webview_id));

        browser_window
    }

    /// Close a window by its ID.
    fn close_window(&self, id: BrowserWindowId) {
        self.windows.borrow_mut().remove(&id);
        // Clear focused_window if it was this one
        if self
            .focused_window
            .borrow()
            .as_ref()
            .is_some_and(|w| w.id() == id)
        {
            *self.focused_window.borrow_mut() = None;
        }
    }

    /// Process pending commands for all windows.
    fn process_pending_commands(self: &Rc<Self>, event_loop: &ActiveEventLoop) {
        // Collect commands from all windows first to avoid borrow conflicts
        let commands: Vec<_> = self
            .windows
            .borrow()
            .values()
            .flat_map(|w| w.take_pending_commands())
            .collect();

        for command in commands {
            match command {
                UserInterfaceCommand::NewWindow(maybe_url, maybe_features) => {
                    let url = maybe_url.unwrap_or(
                        Url::parse("http://system.localhost:8888/index.html")
                            .expect("Invalid default URL"),
                    );
                    self.open_window(event_loop, url, maybe_features);
                },
            }
        }
    }

    /// Handle a tray menu event.
    #[cfg(feature = "status-tray")]
    fn handle_tray_menu_event(self: &Rc<Self>, event_loop: &ActiveEventLoop, menu_id: &MenuId) {
        let Some(tray) = &self.tray else {
            return;
        };
        let ids = tray.menu_ids();

        if menu_id == &ids.show_all {
            self.show_all_windows();
        } else if menu_id == &ids.hide_all {
            self.hide_all_windows();
        } else if menu_id == &ids.search {
            self.open_search_window(event_loop);
        } else if menu_id == &ids.settings {
            self.open_settings_window(event_loop);
        } else if menu_id == &ids.quit {
            // TODO: Make sure to close all windows first.
            self.exit_requested.set(true);
        }
    }

    /// Show and focus all windows.
    #[cfg(feature = "status-tray")]
    fn show_all_windows(&self) {
        for window in self.windows.borrow().values() {
            window.window.set_visible(true);
            window.window.focus_window();
        }
    }

    /// Hide all windows.
    #[cfg(feature = "status-tray")]
    fn hide_all_windows(&self) {
        for window in self.windows.borrow().values() {
            println!("hiding window {:?}", window.window.id());
            window.window.set_visible(false);
        }
    }

    /// Open a new search window, centered on screen.
    /// TODO: switch to a real search window.
    #[cfg(any(feature = "status-tray", feature = "global-hotkeys"))]
    fn open_search_window(self: &Rc<Self>, event_loop: &ActiveEventLoop) {
        let url = Url::parse("http://system.localhost:8888/search.html").expect("Invalid URL");
        let window = self.open_window(event_loop, url, Some("notitle,ontop".into()));

        // Try to center the window on the primary monitor
        if let Some(monitor) = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
        {
            let screen_size = monitor.size();
            let window_size = window.window.outer_size();
            let x = (screen_size.width.saturating_sub(window_size.width)) / 2;
            let y = (screen_size.height.saturating_sub(window_size.height)) / 2;
            let position = winit::dpi::PhysicalPosition::new(
                x as i32 + monitor.position().x,
                y as i32 + monitor.position().y,
            );
            println!("Moving window ({window_size:?}) to {position:?}");
            window.window.set_outer_position(position);
        }
    }

    /// Open the settings window.
    #[cfg(feature = "status-tray")]
    fn open_settings_window(self: &Rc<Self>, event_loop: &ActiveEventLoop) {
        let url = Url::parse("http://system.localhost:8888/index.html?open=http%3A%2F%2Fsettings.localhost%3A8888%2Findex.html")
            .expect("Invalid URL");
        self.open_window(event_loop, url, None);
    }

    fn window_if_system_webview(&self, webview: &WebView) -> Option<Rc<BrowserWindow>> {
        let Some(ref window) = self.window_for_webview_id(webview.id()) else {
            return None;
        };

        let Some(system_webview_id) = window.system_webview_id.get() else {
            return None;
        };

        if webview.id() != system_webview_id {
            return None;
        }

        Some(window.clone())
    }
}

impl WebViewDelegate for AppState {
    fn notify_new_frame_ready(&self, webview: WebView) {
        if let Some(window) = self.window_for_webview_id(webview.id()) {
            window.window.request_redraw();
        }
    }

    fn show_console_message(&self, _webview: WebView, level: ConsoleLogLevel, message: String) {
        println!("[{level:?}] {message}");
    }

    fn request_create_embedded(&self, parent_webview: WebView, request: CreateNewWebViewRequest) {
        let Some(window) = self.window_for_webview_id(parent_webview.id()) else {
            return;
        };

        // Create the embedded webview using the same rendering context as the parent.
        // This is required for embedded webviews to be composited within the parent's scene.
        let webview = request
            .builder(window.rendering_context.clone())
            .hidpi_scale_factor(Scale::new(window.window.scale_factor() as f32))
            .delegate(parent_webview.delegate())
            .build();

        // Store the webview so it doesn't get dropped
        window.webviews.borrow_mut().insert(webview.id(), webview);
    }

    fn notify_cursor_changed(&self, webview: WebView, cursor: Cursor) {
        let Some(window) = self.window_for_webview_id(webview.id()) else {
            return;
        };

        let winit_cursor = match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::ContextMenu => CursorIcon::ContextMenu,
            Cursor::Help => CursorIcon::Help,
            Cursor::Progress => CursorIcon::Progress,
            Cursor::Wait => CursorIcon::Wait,
            Cursor::Cell => CursorIcon::Cell,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::Text => CursorIcon::Text,
            Cursor::VerticalText => CursorIcon::VerticalText,
            Cursor::Alias => CursorIcon::Alias,
            Cursor::Copy => CursorIcon::Copy,
            Cursor::Move => CursorIcon::Move,
            Cursor::NoDrop => CursorIcon::NoDrop,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
            Cursor::EResize => CursorIcon::EResize,
            Cursor::NResize => CursorIcon::NResize,
            Cursor::NeResize => CursorIcon::NeResize,
            Cursor::NwResize => CursorIcon::NwResize,
            Cursor::SResize => CursorIcon::SResize,
            Cursor::SeResize => CursorIcon::SeResize,
            Cursor::SwResize => CursorIcon::SwResize,
            Cursor::WResize => CursorIcon::WResize,
            Cursor::EwResize => CursorIcon::EwResize,
            Cursor::NsResize => CursorIcon::NsResize,
            Cursor::NeswResize => CursorIcon::NeswResize,
            Cursor::NwseResize => CursorIcon::NwseResize,
            Cursor::ColResize => CursorIcon::ColResize,
            Cursor::RowResize => CursorIcon::RowResize,
            Cursor::AllScroll => CursorIcon::AllScroll,
            Cursor::ZoomIn => CursorIcon::ZoomIn,
            Cursor::ZoomOut => CursorIcon::ZoomOut,
            Cursor::None => {
                window.window.set_cursor_visible(false);
                return;
            },
        };
        window.window.set_cursor(winit_cursor);
        window.window.set_cursor_visible(true);
    }

    fn notify_focus_changed(&self, webview: WebView, focused: bool) {
        if let Some(window) = self.window_for_webview_id(webview.id()) {
            if focused {
                window.focused_webview_id.set(Some(webview.id()));
            } else if window.focused_webview_id.get() == Some(webview.id()) {
                // Only clear if this webview was the focused one
                window.focused_webview_id.set(None);
            }
        }
    }

    fn notify_closed(&self, webview: WebView) {
        if let Some(window) = self.window_for_webview_id(webview.id()) {
            window.webviews.borrow_mut().remove(&webview.id());
        }
    }

    fn notify_embedded_webview_created(
        &self,
        parent_webview: WebView,
        new_webview_id: WebViewId,
        new_browsing_context_id: BrowsingContextId,
        new_pipeline_id: PipelineId,
        url: Url,
    ) {
        // Call JavaScript API on the parent (browserhtml shell) to create a new tab
        // that adopts the pre-created webview using the provided IDs.
        // The IDs are serialized as strings since their internal structure is opaque.
        let script = format!(
            "window.servo?.adoptNewWebView({}, {}, {}, {})",
            serde_json::to_string(url.as_str()).unwrap_or_else(|_| "null".to_string()),
            serde_json::to_string(&new_webview_id.to_string())
                .unwrap_or_else(|_| "null".to_string()),
            serde_json::to_string(&new_browsing_context_id.to_string())
                .unwrap_or_else(|_| "null".to_string()),
            serde_json::to_string(&new_pipeline_id.to_string())
                .unwrap_or_else(|_| "null".to_string())
        );
        parent_webview.evaluate_javascript(&script, move |result| {
            if let Err(error) = result {
                error!("Failed to adopt new WebView: {error:?}");
            } else {
                info!("adoptNewWebView() succeeded");
            }
        });
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let Some(window) = self.window_if_system_webview(&webview) else {
            return;
        };

        let Some(title) = title else {
            return;
        };

        if title.is_empty() {
            return;
        }

        window.window.set_title(&title);
    }

    fn notify_input_event_handled(
        &self,
        webview: WebView,
        event_id: InputEventId,
        result: InputEventResult,
    ) {
        if let Some(window) = self.window_for_webview_id(webview.id()) {
            window.handle_keyboard_event_result(event_id, result);
        }
    }

    fn show_notification(&self, webview: WebView, notification: Notification) {
        // For browserhtml, notifications from embedded webviews are routed through
        // the constellation and arrive as iframe events (embednotificationshow).
        // This delegate method is only called for non-embedded webviews, which
        // don't exist in browserhtml's architecture.
        log::debug!(
            "Notification from non-embedded webview {:?}: {:?}",
            webview.id(),
            notification.title
        );
    }

    fn show_embedder_control(&self, webview: WebView, embedder_control: EmbedderControl) {
        if self.window_if_system_webview(&webview).is_none() {
            return;
        };

        let EmbedderControl::InputMethod(ime) = embedder_control else {
            warn!("Not an input field, ignoring");
            return;
        };

        let script = format!(
            "window.servo?.openKeyboard({}, {}, {})",
            serde_json::to_string(&ime.input_method_type()).unwrap_or_else(|_| "null".to_string()),
            serde_json::to_string(&ime.text()).unwrap_or_else(|_| "null".to_string()),
            serde_json::to_string(&ime.insertion_point().unwrap_or(0))
                .unwrap_or_else(|_| "null".to_string())
        );
        webview.evaluate_javascript(&script, move |result| {
            if let Err(error) = result {
                error!("Failed to openKeyboard: {error:?}");
            } else {
                info!("openKeyboard() succeeded");
            }
        });
    }

    fn hide_embedder_control(&self, webview: WebView, control_id: EmbedderControlId) {
        if self.window_if_system_webview(&webview).is_none() {
            return;
        };

        let script = format!(
            "window.servo?.closeEmbedderControl({})",
            serde_json::to_string(&control_id).unwrap_or_else(|_| "null".to_string())
        );
        webview.evaluate_javascript(&script, move |result| {
            if let Err(error) = result {
                error!("Failed to closeEmbedderControl: {error:?}");
            } else {
                info!("closeEmbedderControl() succeeded");
            }
        });
    }
}

impl ServoDelegate for AppState {
    fn notify_devtools_server_started(&self, port: u16, _token: String) {
        info!("Devtools Server running on port {port}");
    }

    fn request_devtools_connection(&self, request: AllowOrDenyRequest) {
        request.allow();
    }

    fn notify_error(&self, error: ServoError) {
        error!("Saw Servo error: {error:?}!");
    }

    fn show_console_message(&self, level: ConsoleLogLevel, message: String) {
        println!("[{level:?}] {message}");
    }

    fn request_open_new_os_window(&self, url: Url, features: &str) {
        if let Some(window) = self.focused_window.borrow().as_ref() {
            let features = if features.is_empty() {
                None
            } else {
                Some(features.to_owned())
            };
            window.queue_command(UserInterfaceCommand::NewWindow(Some(url), features));
        }
    }

    fn request_close_current_os_window(&self) {
        let Some(id) = self.focused_window.borrow().as_ref().map(|w| w.id()) else {
            return;
        };
        self.close_window(id);
    }

    fn request_exit_application(&self) {
        self.exit_requested.set(true);
    }

    fn request_start_window_drag(&self, _webview_id: WebViewId) {
        let Some(id) = self.focused_window.borrow().as_ref().map(|w| w.id()) else {
            return;
        };
        if let Some(window) = self.window(id) {
            let _ = window.window.drag_window();
        }
    }

    fn request_start_window_resize(&self, _webview_id: WebViewId) {
        let Some(id) = self.focused_window.borrow().as_ref().map(|w| w.id()) else {
            return;
        };
        if let Some(window) = self.window(id) {
            let _ = window
                .window
                .drag_resize_window(winit::window::ResizeDirection::NorthWest);
        }
    }
}

/// A minimal ServoDelegate used to break the reference cycle between
/// AppState and Servo during shutdown. This allows Servo's Drop to run.
struct ShutdownDelegate;

impl ServoDelegate for ShutdownDelegate {}

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
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Self::Initial(waker) = self {
            let mut protocol_registry = ProtocolRegistry::default();
            let _ = protocol_registry.register("resource", ResourceProtocolHandler::default());

            let opts = Opts {
                multiprocess: true,
                config_dir: config_dir(),
                ..Default::default()
            };

            let preferences = Preferences {
                viewport_meta_enabled: true,
                devtools_server_enabled: true,
                devtools_server_port: 6222,
                shell_background_color_rgba: [0.0, 0.0, 0.0, 0.0],
                ..Default::default()
            };

            let servo = ServoBuilder::default()
                .opts(opts)
                .preferences(preferences)
                .protocol_registry(protocol_registry)
                .event_loop_waker(Box::new(waker.clone()))
                .build();

            vhost::start_vhost(8888);

            // Create the tray icon before creating the app state.
            // This may return None on systems that don't support tray icons
            // (e.g., GNOME without AppIndicator extension).
            #[cfg(feature = "status-tray")]
            let tray = Tray::new(waker.proxy());

            // Create the global hotkey manager.
            // This may return None if hotkeys cannot be registered
            // (e.g., permission denied on macOS, or hotkey already in use).
            #[cfg(feature = "global-hotkeys")]
            let hotkey_manager = HotkeyManager::new(waker.proxy());

            let app_state = Rc::new(AppState {
                servo: servo.clone(),
                windows: Default::default(),
                focused_window: Default::default(),
                #[cfg(feature = "status-tray")]
                tray,
                exit_requested: Cell::new(false),
                #[cfg(feature = "global-hotkeys")]
                hotkey_manager,
            });

            let state2 = Rc::clone(&app_state);
            servo.set_delegate(state2);
            servo.setup_logging();
            enable_experimental_prefs(&servo);

            let url = Url::parse(
                &std::env::args()
                    .nth(1)
                    .unwrap_or("http://system.localhost:8888/index.html".to_owned()),
            )
            .expect("Invalid url");

            // Create the initial window
            app_state.open_window(event_loop, url, None);

            *self = Self::Running(app_state);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: WakerEvent) {
        if let Self::Running(app_state) = self {
            match event {
                WakerEvent::Wake => {
                    app_state.servo.spin_event_loop();
                },
                #[cfg(feature = "status-tray")]
                WakerEvent::TrayMenu(menu_id) => {
                    app_state.handle_tray_menu_event(event_loop, &menu_id);
                },
                #[cfg(feature = "global-hotkeys")]
                WakerEvent::GlobalHotkey(id) => {
                    if let Some(ref hm) = app_state.hotkey_manager {
                        if hm.is_search_hotkey(id) {
                            app_state.open_search_window(event_loop);
                        }
                    }
                },
            }
            app_state.process_pending_commands(event_loop);

            // Check if quit was requested
            if app_state.exit_requested.get() {
                app_state.shutdown();
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Self::Running(app_state) = self else {
            return;
        };

        app_state.servo.spin_event_loop();

        let Some(browser_window) = app_state.window(window_id.into()) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                app_state.close_window(browser_window.id());
                // Exit if there are no more windows and no tray icon keeping the app alive
                #[cfg(feature = "status-tray")]
                let should_exit = app_state.windows.borrow().is_empty() && app_state.tray.is_none();
                #[cfg(not(feature = "status-tray"))]
                let should_exit = app_state.windows.borrow().is_empty();
                if should_exit {
                    app_state.shutdown();
                    event_loop.exit();
                }
            },
            WindowEvent::Focused(focused) => {
                if focused {
                    *app_state.focused_window.borrow_mut() = Some(browser_window.clone());
                }
            },
            _ => browser_window.window_event(event),
        }

        app_state.process_pending_commands(event_loop);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Self::Running(app_state) = self {
            app_state.shutdown();
        }
    }
}

#[derive(Clone)]
struct Waker(EventLoopProxy<WakerEvent>);

/// Events that can wake the event loop.
#[derive(Debug)]
pub enum WakerEvent {
    /// Servo has work to do.
    Wake,
    /// A tray menu item was clicked.
    #[cfg(feature = "status-tray")]
    TrayMenu(MenuId),
    /// A global hotkey was pressed.
    #[cfg(feature = "global-hotkeys")]
    GlobalHotkey(u32),
}

impl Waker {
    fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self(event_loop.create_proxy())
    }

    #[cfg(any(feature = "status-tray", feature = "global-hotkeys"))]
    fn proxy(&self) -> EventLoopProxy<WakerEvent> {
        self.0.clone()
    }
}

impl embedder_traits::EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn embedder_traits::EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }

    fn wake(&self) {
        if let Err(error) = self.0.send_event(WakerEvent::Wake) {
            warn!("Failed to wake event loop: {error}");
        }
    }
}
