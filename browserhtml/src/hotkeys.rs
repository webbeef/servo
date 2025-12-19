// SPDX-License-Identifier: AGPL-3.0-or-later

//! Global keyboard shortcut management.

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use log::warn;
use winit::event_loop::EventLoopProxy;

use crate::WakerEvent;

/// Manages global keyboard shortcuts.
pub(crate) struct HotkeyManager {
    _manager: GlobalHotKeyManager,
    search_hotkey_id: u32,
}

impl HotkeyManager {
    /// Creates a new hotkey manager and registers global shortcuts.
    /// Returns None if the hotkey manager cannot be created or shortcuts cannot be registered.
    pub fn new(event_loop_proxy: EventLoopProxy<WakerEvent>) -> Option<Self> {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to create global hotkey manager: {e}");
                return None;
            },
        };

        // Register Cmd+Shift+Space (macOS) or Ctrl+Shift+Space (others)
        #[cfg(target_os = "macos")]
        let modifiers = Modifiers::META | Modifiers::SHIFT;
        #[cfg(not(target_os = "macos"))]
        let modifiers = Modifiers::CONTROL | Modifiers::SHIFT;

        let search_hotkey = HotKey::new(Some(modifiers), Code::Space);
        let search_hotkey_id = search_hotkey.id();

        if let Err(e) = manager.register(search_hotkey) {
            warn!(
                "Failed to register global hotkey (Ctrl/Cmd+Shift+Space): {e}. It may already be in use by another application."
            );
            return None;
        }

        // Set up event handler to forward hotkey events to the winit event loop.
        // Only forward key press events, not release events.
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                let _ = event_loop_proxy.send_event(WakerEvent::GlobalHotkey(event.id));
            }
        }));

        Some(Self {
            _manager: manager,
            search_hotkey_id,
        })
    }

    /// Returns true if the given hotkey ID corresponds to the search hotkey.
    pub fn is_search_hotkey(&self, id: u32) -> bool {
        id == self.search_hotkey_id
    }
}
