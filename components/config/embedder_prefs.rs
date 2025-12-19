/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Extensible preference system for embedders.
//!
//! This module provides a trait and helper functions that allow embedders to define
//! their own type-safe preferences using the `#[derive(EmbedderPreferences)]` macro.
//!
//! # Example
//!
//! ```rust,ignore
//! use servo_config_macro::EmbedderPreferences;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Debug, Deserialize, Serialize, EmbedderPreferences)]
//! #[namespace = "myembedder"]
//! pub struct MyPreferences {
//!     pub theme: String,
//!     pub enable_feature: bool,
//! }
//!
//! impl Default for MyPreferences {
//!     fn default() -> Self {
//!         Self {
//!             theme: "system".to_string(),
//!             enable_feature: false,
//!         }
//!     }
//! }
//!
//! // Generate global storage and accessors
//! servo_config::define_embedder_prefs!(MyPreferences);
//!
//! // Load from file (merges with defaults)
//! prefs::load(Path::new("~/.config/myembedder/prefs.json"));
//!
//! // Save to file
//! prefs::save(Path::new("~/.config/myembedder/prefs.json"));
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::{LazyLock, RwLock};
use std::{fs, io};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::pref_util::PrefValue;
use crate::prefs::OBSERVERS;

/// Type for callbacks that handle preference changes from JavaScript.
/// The callback receives the local preference name (without namespace) and the new value.
pub type EmbedderPrefSetterCallback = fn(&str, PrefValue);

/// Registry of callbacks for embedder preference setters, keyed by namespace.
static SETTER_CALLBACKS: LazyLock<RwLock<HashMap<&'static str, EmbedderPrefSetterCallback>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Register a callback to be invoked when preferences in the given namespace
/// are set via ServoInternals (JavaScript API).
///
/// This allows embedders to update their local preference storage when
/// preferences are changed from JavaScript.
pub fn register_setter_callback(namespace: &'static str, callback: EmbedderPrefSetterCallback) {
    SETTER_CALLBACKS
        .write()
        .unwrap()
        .insert(namespace, callback);
}

/// Set an embedder preference from JavaScript (via ServoInternals).
///
/// This function:
/// 1. Parses the namespaced name to extract namespace and local name
/// 2. Updates the embedder prefs registry
/// 3. Invokes the registered callback for the namespace (if any)
/// 4. Notifies all observers
///
/// Use this function when preferences are set from JavaScript.
/// For Rust-side changes, use `prefs::set()` instead which handles everything.
pub fn set_embedder_pref_from_script(full_name: &str, value: PrefValue) {
    // Parse namespace from "browserhtml.theme" -> ("browserhtml", "theme")
    let Some((namespace, local_name)) = full_name.split_once('.') else {
        // Not a namespaced preference, just update the registry
        crate::prefs::set_embedder_pref(full_name, value);
        return;
    };

    // Update the registry
    crate::prefs::set_embedder_pref(full_name, value.clone());

    // Invoke the callback for this namespace if registered
    if let Some(callback) = SETTER_CALLBACKS.read().unwrap().get(namespace) {
        callback(local_name, value.clone());
    }

    // Notify observers with the full namespaced name
    let static_name = Box::leak(full_name.to_string().into_boxed_str()) as &'static str;
    let changes = [(static_name, value)];
    for observer in &*OBSERVERS.read().unwrap() {
        observer.prefs_changed(&changes);
    }
}

/// Trait that embedder preference structs must implement.
///
/// This trait is automatically implemented by the `#[derive(EmbedderPreferences)]` macro.
/// It provides a common interface for accessing and modifying embedder preferences,
/// as well as computing diffs for observer notifications.
pub trait EmbedderPreferences: Send + Sync + Clone + 'static {
    /// The namespace prefix for this preference set (e.g., "browserhtml").
    ///
    /// All preference names will be prefixed with `"{namespace}."` when reported
    /// to observers. This prevents collisions with Servo's core preferences and
    /// other embedders' preferences.
    fn namespace() -> &'static str;

    /// Get a preference value by its local name (without namespace prefix).
    ///
    /// Returns `None` if the preference name is not recognized.
    fn get_value(&self, name: &str) -> Option<PrefValue>;

    /// Set a preference value by its local name (without namespace prefix).
    ///
    /// Returns `true` if the preference was found and set successfully,
    /// `false` otherwise.
    fn set_value(&mut self, name: &str, value: PrefValue) -> bool;

    /// Compute the diff between this preferences instance and another.
    ///
    /// Returns a list of `(local_name, new_value)` pairs for preferences
    /// that differ between `self` and `other`.
    fn diff(&self, other: &Self) -> Vec<(&'static str, PrefValue)>;

    /// List all preference names in this struct (local names, without namespace).
    fn pref_names() -> &'static [&'static str];
}

/// Error type for preference persistence operations.
#[derive(Debug)]
pub enum PrefsPersistError {
    /// I/O error reading or writing the file.
    Io(io::Error),
    /// JSON serialization/deserialization error.
    Json(serde_json::Error),
}

impl std::fmt::Display for PrefsPersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrefsPersistError::Io(e) => write!(f, "I/O error: {}", e),
            PrefsPersistError::Json(e) => write!(f, "JSON error: {}", e),
        }
    }
}

impl std::error::Error for PrefsPersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PrefsPersistError::Io(e) => Some(e),
            PrefsPersistError::Json(e) => Some(e),
        }
    }
}

impl From<io::Error> for PrefsPersistError {
    fn from(e: io::Error) -> Self {
        PrefsPersistError::Io(e)
    }
}

impl From<serde_json::Error> for PrefsPersistError {
    fn from(e: serde_json::Error) -> Self {
        PrefsPersistError::Json(e)
    }
}

/// Load embedder preferences from a JSON file.
///
/// Returns the default value if a i/o or json error occurs.
pub fn load_from_path<T: DeserializeOwned + Default>(path: &Path) -> T {
    if !path.exists() {
        return T::default();
    }
    let Ok(contents) = fs::read_to_string(path) else {
        return T::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

/// Save embedder preferences to a JSON file.
///
/// Creates parent directories if they don't exist.
pub fn save_to_path<T: Serialize>(prefs: &T, path: &Path) -> Result<(), PrefsPersistError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let contents = serde_json::to_string_pretty(prefs)?;
    fs::write(path, contents)?;
    Ok(())
}

/// Notify observers of embedder preference changes with namespaced keys.
///
/// This function computes the diff between the old and new preference values,
/// prepends the namespace to each changed preference name, updates the embedder
/// preference registry, and notifies all registered observers.
///
/// # Example
///
/// ```rust,ignore
/// let old_prefs = prefs::get().clone();
/// let mut new_prefs = old_prefs.clone();
/// new_prefs.theme = "dark".to_string();
/// notify_embedder_pref_changes(&old_prefs, &new_prefs);
/// // Observers receive: [("browserhtml.theme", PrefValue::Str("dark"))]
/// ```
pub fn notify_embedder_pref_changes<T: EmbedderPreferences>(old: &T, new: &T) {
    let changes = new.diff(old);
    if changes.is_empty() {
        return;
    }

    let namespace = T::namespace();
    // Create namespaced changes: "browserhtml.theme" instead of "theme"
    let namespaced: Vec<_> = changes
        .into_iter()
        .map(|(name, value)| {
            let full_name = Box::leak(format!("{}.{}", namespace, name).into_boxed_str());
            // Update the embedder prefs registry so ServoInternals can access these
            crate::prefs::set_embedder_pref(full_name, value.clone());
            (full_name as &'static str, value)
        })
        .collect();

    for observer in &*OBSERVERS.read().unwrap() {
        observer.prefs_changed(&namespaced);
    }
}

/// Sync all embedder preferences to the registry.
///
/// This should be called after loading preferences to ensure the registry
/// is up-to-date with all current embedder preference values.
pub fn sync_to_registry<T: EmbedderPreferences>(prefs: &T) {
    let namespace = T::namespace();
    for name in T::pref_names() {
        if let Some(value) = prefs.get_value(name) {
            let full_name = format!("{}.{}", namespace, name);
            crate::prefs::set_embedder_pref(&full_name, value);
        }
    }
}

/// Macro to define embedder preference storage with accessor functions.
///
/// This macro generates:
/// - `get()` - Read access to current preferences
/// - `set(prefs)` - Update preferences, notify observers, and auto-save if a path was set
/// - `load(path)` - Load preferences from a JSON file and remember path for auto-save
/// - `save(path)` - Save current preferences to a JSON file
/// - A callback for JavaScript-initiated preference changes
///
/// # Example
///
/// ```rust,ignore
/// // In your embedder's prefs module:
/// servo_config::define_embedder_prefs!(MyPreferences);
///
/// // Load from file at startup (also enables auto-save on changes)
/// if let Err(e) = prefs::load(Path::new("prefs.json")) {
///     eprintln!("Failed to load preferences: {}", e);
/// }
///
/// // Read preferences
/// let theme = prefs::get().theme.clone();
///
/// // Update preferences (notifies observers and auto-saves to the loaded path)
/// let mut new_prefs = (*prefs::get()).clone();
/// new_prefs.theme = "dark".to_string();
/// prefs::set(new_prefs);
/// ```
#[macro_export]
macro_rules! define_embedder_prefs {
    ($prefs_type:ty) => {
        use std::path::{Path, PathBuf};
        use std::sync::{RwLock, RwLockReadGuard};

        static PREFS: RwLock<$prefs_type> = RwLock::new(<$prefs_type>::DEFAULT);
        static PREFS_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

        /// Get the current set of embedder preferences.
        pub fn get() -> RwLockReadGuard<'static, $prefs_type> {
            PREFS.read().unwrap()
        }

        /// Set the embedder preferences, notify observers of changes, and auto-save.
        ///
        /// If preferences were previously loaded with `load()`, this will automatically
        /// save the updated preferences to the same file.
        pub fn set(prefs: $prefs_type) {
            let old = PREFS.read().unwrap().clone();
            *PREFS.write().unwrap() = prefs.clone();
            $crate::embedder_prefs::notify_embedder_pref_changes(&old, &prefs);

            // Auto-save if we have a path
            if let Some(ref path) = *PREFS_PATH.read().unwrap() {
                if let Err(e) = $crate::embedder_prefs::save_to_path(&prefs, path) {
                    log::warn!("Failed to auto-save preferences: {}", e);
                }
            }
        }

        /// Internal function to set a single preference value from JavaScript.
        /// Called by the registered setter callback.
        fn set_single_pref_from_script(local_name: &str, value: $crate::pref_util::PrefValue) {
            use $crate::embedder_prefs::EmbedderPreferences;

            let mut prefs = PREFS.write().unwrap();
            if prefs.set_value(local_name, value) {
                // Auto-save if we have a path
                if let Some(ref path) = *PREFS_PATH.read().unwrap() {
                    if let Err(e) = $crate::embedder_prefs::save_to_path(&*prefs, path) {
                        log::warn!("Failed to auto-save preferences: {}", e);
                    }
                }
            }
        }

        /// Load embedder preferences from a JSON file.
        ///
        /// If the file exists and is valid, updates the global preferences
        /// and notifies observers of any changes. If the file doesn't exist,
        /// the current preferences are left unchanged (but the directory is created).
        ///
        /// The path is remembered for auto-save: subsequent calls to `set()`
        /// will automatically save to this path.
        ///
        /// After loading (or using defaults), all preferences are synced to the
        /// embedder prefs registry so they can be accessed via ServoInternals.
        ///
        /// Also registers a callback for JavaScript-initiated preference changes.
        pub fn load(path: &Path) -> Result<(), $crate::embedder_prefs::PrefsPersistError> {
            use $crate::embedder_prefs::EmbedderPreferences;

            // Create the config directory if it doesn't exist
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }

            // Remember the path for auto-save
            *PREFS_PATH.write().unwrap() = Some(path.to_path_buf());

            let prefs = $crate::embedder_prefs::load_from_path::<$prefs_type>(path);
            // Use internal set to avoid double-save
            let old = PREFS.read().unwrap().clone();
            *PREFS.write().unwrap() = prefs.clone();
            $crate::embedder_prefs::notify_embedder_pref_changes(&old, &prefs);

            // Sync all current preferences to the registry (including defaults)
            // so they're accessible via ServoInternals
            $crate::embedder_prefs::sync_to_registry(&*PREFS.read().unwrap());

            // Register callback for JavaScript-initiated preference changes
            $crate::embedder_prefs::register_setter_callback(
                <$prefs_type as EmbedderPreferences>::namespace(),
                set_single_pref_from_script,
            );

            Ok(())
        }

        /// Save the current embedder preferences to a JSON file.
        ///
        /// Creates parent directories if they don't exist.
        pub fn save(path: &Path) -> Result<(), $crate::embedder_prefs::PrefsPersistError> {
            let prefs = PREFS.read().unwrap();
            $crate::embedder_prefs::save_to_path(&*prefs, path)
        }
    };
}
