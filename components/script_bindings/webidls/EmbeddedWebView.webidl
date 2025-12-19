/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Servo extension: Embedded WebView methods and events for HTMLIFrameElement.
// These methods only work when the iframe has the "embed" attribute set,
// which creates a top-level embedded webview instead of a nested browsing context.
// All methods throw if called on an iframe without the embed attribute.

interface mixin EmbeddedWebView {
  // Navigation
  [Throws] undefined load(USVString url);
  [Throws] undefined reload();

  // Zoom control
  [Throws] double getPageZoom();
  [Throws] undefined setPageZoom(double zoom);

  // History navigation
  [Throws] boolean canGoBack();
  [Throws] DOMString goBack();  // Returns TraversalId for tracking completion
  [Throws] boolean canGoForward();
  [Throws] DOMString goForward();  // Returns TraversalId for tracking completion

  // Screenshot
  [Throws] Promise<Blob> takeScreenshot(optional ScreenshotOptions options = {});

  // Embedder control responses (for select dropdowns, color pickers, etc.)
  // These are called by the parent shell in response to embedcontrolshow events
  [Throws] undefined respondToSelectControl(DOMString controlId, long selectedIndex);
  [Throws] undefined respondToColorPicker(DOMString controlId, DOMString? color);
  [Throws] undefined respondToContextMenu(DOMString controlId, DOMString? actionId);
  [Throws] undefined cancelEmbedderControl(DOMString controlId);

  // Simple dialog responses (for alert, confirm, prompt)
  // These are called by the parent shell in response to embeddialogshow events
  [Throws] undefined respondToAlert(DOMString controlId);
  [Throws] undefined respondToConfirm(DOMString controlId, boolean confirmed);
  [Throws] undefined respondToPrompt(DOMString controlId, DOMString? value);

  // Permission prompt response
  [Throws] undefined respondToPermissionPrompt(DOMString controlId, boolean allowed);
};



// Options for taking a screenshot of an embedded webview.
dictionary ScreenshotOptions {
  DOMString type = "image/png";  // "image/png", "image/jpeg", "image/webp"
  double quality = 0.92;         // 0.0 to 1.0, used for JPEG/WebP compression
};

// Position and size of an embedder control in device pixels.
dictionary EmbedderControlPosition {
  double x = 0;
  double y = 0;
  double width = 0;
  double height = 0;
};

// An option in a select element dropdown.
dictionary EmbedderSelectOption {
  unsigned long id = 0;
  DOMString label = "";
  boolean disabled = false;
  DOMString? group = null;  // Optgroup label if this option is in a group
};

// Parameters for select element control.
dictionary EmbedderSelectParameters {
  sequence<EmbedderSelectOption> options;
  long selectedIndex = -1;
};

// Parameters for color picker control.
dictionary EmbedderColorParameters {
  DOMString currentColor = "#000000";
};

// Parameters for file picker control.
dictionary EmbedderFileParameters {
  boolean multiple = false;           // Allow multiple file selection
  sequence<DOMString> acceptTypes;    // MIME types or extensions to accept (e.g., "image/*", ".pdf")
  DOMString? directory = null;        // Suggested directory path
};

// Parameters for input method control.
dictionary EmbedderInputMethodParameters {
  DOMString inputType = "text";       // "text", "password", "number", "email", "tel", "url", etc.
  DOMString? currentValue = null;     // Current input value
  DOMString? placeholder = null;      // Placeholder text
};

// A context menu item.
dictionary EmbedderContextMenuItem {
  DOMString id = "";
  DOMString label = "";
  boolean disabled = false;
  boolean checked = false;
  DOMString? icon = null;             // Optional icon URL
};

// Parameters for context menu control.
dictionary EmbedderContextMenuParameters {
  sequence<EmbedderContextMenuItem> items;
};

// Parameters for permission prompt control.
dictionary EmbedderPermissionParameters {
  DOMString feature = "";       // Permission feature: "geolocation", "camera", "microphone", etc.
  DOMString featureName = "";   // Human-readable name: "Location", "Camera", "Microphone", etc.
};

// Event detail for embedcontrolshow events.
// The controlType determines which parameters field is present.
dictionary EmbedderControlShowEventDetail {
  required DOMString controlType;  // "select", "color", "file", "inputmethod", "contextmenu", "permission"
  required DOMString controlId;    // Unique ID for response routing
  EmbedderControlPosition position;

  // Control-specific parameters (only one will be set based on controlType)
  EmbedderSelectParameters selectParameters;        // For controlType "select"
  EmbedderColorParameters colorParameters;          // For controlType "color"
  EmbedderFileParameters fileParameters;            // For controlType "file"
  EmbedderInputMethodParameters inputMethodParameters;  // For controlType "inputmethod"
  EmbedderContextMenuParameters contextMenuParameters;  // For controlType "contextmenu"
  EmbedderPermissionParameters permissionParameters;    // For controlType "permission"
};

// Event detail for embedcontrolhide events.
dictionary EmbedderControlHideEventDetail {
  required DOMString controlId;
};

// Event detail for embeddialogshow events (alert, confirm, prompt).
// The dialogType determines which fields are relevant.
dictionary EmbedderDialogShowEventDetail {
  required DOMString dialogType;   // "alert", "confirm", "prompt"
  required DOMString controlId;    // Unique ID for response routing
  required DOMString message;      // The message to display
  DOMString? defaultValue;         // Default value for prompt dialogs (null for alert/confirm)
};

// Event detail for embednotificationshow events.
dictionary EmbedderNotificationShowEventDetail {
  required DOMString title;        // Notification title
  DOMString body = "";             // Notification body text
  DOMString tag = "";              // Tag for deduplication/replacement
  DOMString? iconUrl = null;       // Icon URL
};
