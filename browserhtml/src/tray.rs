// SPDX-License-Identifier: AGPL-3.0-or-later

//! System tray icon and menu management.

#[cfg(target_os = "linux")]
use std::sync::mpsc;

use image::ImageReader;
use log::warn;
#[cfg(not(target_os = "linux"))]
use tray_icon::TrayIcon;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use winit::event_loop::EventLoopProxy;

use crate::WakerEvent;

/// Identifiers for tray menu items.
#[derive(Clone, Debug)]
pub(crate) struct TrayMenuIds {
    pub show_all: MenuId,
    pub hide_all: MenuId,
    pub search: MenuId,
    pub settings: MenuId,
    pub quit: MenuId,
}

/// Manages the system tray icon and its associated menu.
pub(crate) struct Tray {
    /// On non-Linux platforms, we hold the TrayIcon directly.
    /// On Linux, the TrayIcon lives in a separate GTK thread.
    #[cfg(not(target_os = "linux"))]
    _icon: TrayIcon,
    #[cfg(target_os = "linux")]
    _gtk_thread: std::thread::JoinHandle<()>,
    menu_ids: TrayMenuIds,
}

impl Tray {
    /// Creates a new system tray icon with menu.
    /// Returns None if the tray icon cannot be created (e.g., on GNOME without
    /// the AppIndicator extension installed).
    #[cfg(not(target_os = "linux"))]
    pub fn new(event_loop_proxy: EventLoopProxy<WakerEvent>) -> Option<Self> {
        let (menu_ids, menu) = create_menu();
        let icon = load_icon()?;

        let tray_icon = match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("Browser.HTML")
            .build()
        {
            Ok(icon) => icon,
            Err(e) => {
                warn!(
                    "Failed to create tray icon: {e}. The app will exit when all windows are closed."
                );
                return None;
            },
        };

        // Set up menu event forwarding to the winit event loop
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = event_loop_proxy.send_event(WakerEvent::TrayMenu(event.id));
        }));

        Some(Self {
            _icon: tray_icon,
            menu_ids,
        })
    }

    /// Creates a new system tray icon with menu on Linux.
    /// On Linux, GTK must run in its own thread since winit doesn't use GTK.
    #[cfg(target_os = "linux")]
    pub fn new(event_loop_proxy: EventLoopProxy<WakerEvent>) -> Option<Self> {
        // Create menu IDs first so we can return them
        let (menu_ids, _) = create_menu();
        let menu_ids_clone = menu_ids.clone();

        // Use a channel to communicate success/failure from the GTK thread
        let (tx, rx) = mpsc::channel();

        let gtk_thread = std::thread::spawn(move || {
            if gtk::init().is_err() {
                let _ = tx.send(false);
                return;
            }

            let (_, menu) = create_menu_with_ids(&menu_ids_clone);
            let Some(icon) = load_icon() else {
                let _ = tx.send(false);
                return;
            };

            let tray_icon = match TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_icon(icon)
                .with_tooltip("Browser.HTML")
                .build()
            {
                Ok(icon) => icon,
                Err(e) => {
                    warn!(
                        "Failed to create tray icon: {e}. The app will exit when all windows are closed."
                    );
                    let _ = tx.send(false);
                    return;
                },
            };

            // Set up menu event forwarding to the winit event loop
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let _ = event_loop_proxy.send_event(WakerEvent::TrayMenu(event.id));
            }));

            // Signal success
            let _ = tx.send(true);

            // Keep the tray icon alive by preventing it from being dropped
            let _tray_icon = tray_icon;

            // Run the GTK main loop
            gtk::main();
        });

        // Wait for the GTK thread to report success or failure
        match rx.recv() {
            Ok(true) => Some(Self {
                _gtk_thread: gtk_thread,
                menu_ids,
            }),
            _ => {
                warn!(
                    "Failed to initialize tray icon on Linux. The app will exit when all windows are closed."
                );
                None
            },
        }
    }

    /// Returns the menu IDs for matching against menu events.
    pub fn menu_ids(&self) -> &TrayMenuIds {
        &self.menu_ids
    }
}

/// Create the menu and return the menu IDs along with the menu.
fn create_menu() -> (TrayMenuIds, Menu) {
    let show_all = MenuItem::new("Show all windows", true, None);
    let hide_all = MenuItem::new("Hide all windows", true, None);
    let search = MenuItem::new("Search", true, None);
    let settings = MenuItem::new("Settings", true, None);
    let quit = MenuItem::new("Quit", true, None);

    let menu_ids = TrayMenuIds {
        show_all: show_all.id().clone(),
        hide_all: hide_all.id().clone(),
        search: search.id().clone(),
        settings: settings.id().clone(),
        quit: quit.id().clone(),
    };

    let menu = Menu::new();
    #[cfg(not(target_os = "linux"))]
    {
        let _ = menu.append(&show_all);
        let _ = menu.append(&hide_all);
    }
    let _ = menu.append(&search);
    let _ = menu.append(&settings);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    (menu_ids, menu)
}

/// Create a menu using pre-existing menu IDs (for Linux where IDs must match).
#[cfg(target_os = "linux")]
fn create_menu_with_ids(ids: &TrayMenuIds) -> (TrayMenuIds, Menu) {
    #[cfg(not(target_os = "linux"))]
    {
        let show_all = MenuItem::with_id(ids.show_all.clone(), "Show all windows", true, None);
        let hide_all = MenuItem::with_id(ids.hide_all.clone(), "Hide all windows", true, None);
    }
    let search = MenuItem::with_id(ids.search.clone(), "Search", true, None);
    let settings = MenuItem::with_id(ids.settings.clone(), "Settings", true, None);
    let quit = MenuItem::with_id(ids.quit.clone(), "Quit", true, None);

    let menu = Menu::new();
    #[cfg(not(target_os = "linux"))]
    {
        let _ = menu.append(&show_all);
        let _ = menu.append(&hide_all);
    }
    let _ = menu.append(&search);
    let _ = menu.append(&settings);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    (ids.clone(), menu)
}

/// Load the tray icon from embedded bytes.
fn load_icon() -> Option<Icon> {
    let file_path = crate::resources::resources_dir_path()
        .join("browserhtml")
        .join("system")
        .join("logo.png");

    let img = ImageReader::open(file_path).ok()?.decode().ok()?;
    let rgba = img.to_rgba8();

    Icon::from_rgba(rgba.to_vec(), img.width(), img.height()).ok()
}
