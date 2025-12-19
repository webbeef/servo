// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use servo::PrefValue;
use servo_config_macro::EmbedderPreferences;

/// BrowserHTML-specific preferences.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, EmbedderPreferences)]
#[namespace = "browserhtml"]
pub struct BrowserHtmlPreferences {
    /// The UI theme
    #[serde(default = "default_theme")]
    pub theme: String,
    /// The url of the new Web View page
    #[serde(default = "default_start_url")]
    pub start_url: String,
    /// The url of the homescreen
    #[serde(default = "default_homescreen_url")]
    pub homescreen_url: String,
    /// The alias of the search engine
    #[serde(default = "default_search_engine")]
    pub search_engine: String,
    /// Whether to show the virtual keyboard when an input element is focused.
    #[serde(default = "default_false")]
    pub ime_virtual_keyboard_enabled: bool,
    /// Force mobile UI mode regardless of screen size (for development)
    #[serde(default = "default_false")]
    pub mobile_simulation: bool,
}

fn default_start_url() -> String {
    "http://system.localhost:8888/new_view.html".to_string()
}

fn default_homescreen_url() -> String {
    "http://homescreen.localhost:8888/index.html".to_string()
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_search_engine() -> String {
    "duckduckgo".to_string()
}

fn default_false() -> bool {
    false
}

impl BrowserHtmlPreferences {
    /// Default preferences as a const for static initialization.
    pub const DEFAULT: Self = Self {
        theme: String::new(),
        start_url: String::new(),
        homescreen_url: String::new(),
        search_engine: String::new(),
        ime_virtual_keyboard_enabled: false,
        mobile_simulation: false,
    };
}

impl Default for BrowserHtmlPreferences {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            start_url: default_start_url(),
            homescreen_url: default_homescreen_url(),
            search_engine: default_search_engine(),
            ime_virtual_keyboard_enabled: false,
            mobile_simulation: false,
        }
    }
}

// Generate global storage and accessors for browserhtml preferences
servo::define_embedder_prefs!(BrowserHtmlPreferences);

static EXPERIMENTAL_PREFS: &[&str] = &[
    "dom_async_clipboard_enabled",
    "dom_fontface_enabled",
    "dom_intersection_observer_enabled",
    "dom_navigator_protocol_handlers_enabled",
    "dom_notification_enabled",
    "dom_offscreen_canvas_enabled",
    "dom_permissions_enabled",
    "dom_webgl2_enabled",
    "dom_webgpu_enabled",
    "layout_columns_enabled",
    "layout_container_queries_enabled",
    "layout_grid_enabled",
    "layout_variable_fonts_enabled",
];

pub(crate) fn enable_experimental_prefs(servo: &servo::Servo) {
    for pref in EXPERIMENTAL_PREFS {
        servo.set_preference(pref, PrefValue::Bool(true));
    }
}
