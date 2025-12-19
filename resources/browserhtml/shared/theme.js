// SPDX-License-Identifier: AGPL-3.0-or-later

const THEME_PREF_KEY = "browserhtml.theme";
const DEFAULT_THEME = "default";

// Available themes - TODO: make that dynamic
const AVAILABLE_THEMES = [
  { id: "default", name: "Default", description: "Vibrant gradient theme" },
  { id: "lcars", name: "LCARS", description: "Star Trek sci-fi aesthetic" },
  { id: "meadow", name: "Meadow", description: "Peaceful nature-inspired" },
  { id: "winamp", name: "Winamp", description: "Classic media player vibes" },
];

/**
 * Get the list of available themes
 * @returns {Array<{id: string, name: string, description: string}>}
 */
export function getAvailableThemes() {
  return AVAILABLE_THEMES;
}

/**
 * Get the currently selected theme ID from preferences
 * @returns {string} Theme ID
 */
export function getTheme() {
  try {
    // Use navigator.servo preference API if available
    if (navigator.servo) {
      const theme = navigator.servo.getStringPreference(THEME_PREF_KEY);
      if (theme && theme !== "") {
        return theme;
      }
    }
  } catch (e) {
    // Preference not set or API not available
  }
  return DEFAULT_THEME;
}

/**
 * Set the theme and persist to preferences
 * @param {string} themeId - The theme ID to set
 */
export function setTheme(themeId) {
  // Validate theme exists
  const theme = AVAILABLE_THEMES.find((t) => t.id === themeId);
  if (!theme) {
    console.warn(`Theme "${themeId}" not found, using default`);
    themeId = DEFAULT_THEME;
  }

  // Persist to preferences via navigator.servo API
  // This will trigger preferencechanged event on navigator.embedder
  try {
    if (navigator.servo) {
      navigator.servo.setStringPreference(THEME_PREF_KEY, themeId);
    }
  } catch (e) {
    console.warn("Could not save theme preference:", e);
  }
}

/**
 * Apply the current theme by updating the theme stylesheet link
 */
function applyTheme() {
  const themeLink = document.querySelector('link[href*="theme.localhost"]');
  if (themeLink) {
    themeLink.href = `http://theme.localhost:8888/index.css?${Math.round(Math.random() * 1000000)}`;
  } else {
    console.error(`Failed to apply theme on ${location.href}`);
  }
}

/**
 * Initialize theme system - call this on page load
 * Sets up the current theme and listens for preference changes
 */
function initTheme() {
  // Listen for preference changes from the embedder
  if (navigator.embedder) {
    navigator.embedder.addEventListener("preferencechanged", (event) => {
      console.log(`[Theme PrefChanged-${location.href}] ${event.detail.name} : ${event.detail.value}`);
      if (event.detail.name === THEME_PREF_KEY) {
        applyTheme();
      }
    });
  }
}

initTheme();
