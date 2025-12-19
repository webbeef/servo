// SPDX-License-Identifier: AGPL-3.0-or-later

import { WebView } from "./web_view.js";
import { LayoutManager } from "./layout_manager.js";
import { MobileLayoutManager } from "./mobile_layout_manager.js";
import "./system_menu.js";
import "./mobile_action_bar.js";
import "./mobile_notification_sheet.js";
import "./mobile_overview.js";
import "./mobile_radial_menu.js";

let NEW_FRAME_DEFAULT_URL = navigator.servo.getStringPreference(
  "browserhtml.start_url",
);

// Unique ID for this browser window (used for cross-window communication)
const windowId = `browser-${Date.now()}-${Math.random().toString(36).slice(2)}`;

navigator.embedder.addEventListener("servoerror", (event) => {
  console.error(`[Servo] ${event.type} ${event.detail}`);
});

// ============================================================================
// Global State and Event Handlers
// ============================================================================

let layoutManager = null;
let systemMenu = null;
let notificationPanel = null;
let mobileActionBar = null;
let mobileNotificationSheet = null;
let mobileOverview = null;
let mobileRadialMenu = null;
let isMobileMode = false;

// Detect if we should use mobile mode
function detectMobileMode() {
  // Check if mobile simulation is forced via preference
  const mobileSimulation = navigator.servo.getBoolPreference(
    "browserhtml.mobile_simulation",
  );
  if (mobileSimulation) {
    return true;
  }

  // maxTouchPoints is not yet supported on Servo
  // const isTouchDevice = navigator.maxTouchPoints > 0;
  const isNarrowScreen = window.matchMedia("(max-width: 768px)").matches;
  return isNarrowScreen;
}

// Create the appropriate layout manager based on device type
function createLayoutManager(rootElement, webViewBuilder) {
  isMobileMode = detectMobileMode();

  if (isMobileMode) {
    document.body.classList.add("mobile-mode");
    return new MobileLayoutManager(rootElement, webViewBuilder);
  } else {
    document.body.classList.remove("mobile-mode");
    return new LayoutManager(rootElement, webViewBuilder);
  }
}

navigator.embedder.addEventListener("preferencechanged", (event) => {
  console.log(
    `[System PrefChanged] ${event.detail.name} : ${event.detail.value}`,
  );
  if (event.detail.name == "browserhtml.start_url") {
    NEW_FRAME_DEFAULT_URL = event.detail.value;
  }
});

// Notification state
const MAX_NOTIFICATIONS = 50;
let notifications = [];

function updateNotificationBadge() {
  const badge = document.getElementById("notifications-badge");
  if (!badge) {
    return;
  }

  const count = notifications.length;
  if (count > 0) {
    badge.textContent =
      count > MAX_NOTIFICATIONS ? `${MAX_NOTIFICATIONS}+` : count;
    badge.classList.remove("hidden");
  } else {
    badge.classList.add("hidden");
  }
}

function addNotification(notification) {
  // Add timestamp if not present
  if (!notification.timestamp) {
    notification.timestamp = Date.now();
  }

  // Generate a unique ID if not present
  if (!notification.id) {
    notification.id = `notif-${Date.now()}-${Math.random()
      .toString(36)
      .slice(2)}`;
  }

  // Check for same-tag replacement
  if (notification.tag) {
    const existingIndex = notifications.findIndex(
      (n) => n.tag === notification.tag,
    );
    if (existingIndex !== -1) {
      // Replace existing notification with same tag
      notifications[existingIndex] = notification;
    } else {
      // Add to front
      notifications.unshift(notification);
    }
  } else {
    // No tag, just add to front
    notifications.unshift(notification);
  }

  // Enforce max limit (FIFO)
  if (notifications.length > MAX_NOTIFICATIONS) {
    notifications = notifications.slice(0, MAX_NOTIFICATIONS);
  }

  // Update panel and badge
  if (notificationPanel) {
    notificationPanel.notifications = [...notifications];
  }
  updateNotificationBadge();
}

function dismissNotification(notification) {
  const index = notifications.findIndex((n) => n.id === notification.id);
  if (index !== -1) {
    notifications.splice(index, 1);
    if (notificationPanel) {
      notificationPanel.notifications = [...notifications];
    }
    updateNotificationBadge();
  }
}

function clearAllNotifications() {
  notifications = [];
  if (notificationPanel) {
    notificationPanel.notifications = [];
  }
  updateNotificationBadge();
}

window.servo = {
  /**
   * Called by Servo when an embedded webview has opened a new embedded webview
   * via window.open() or target="_blank" link, and the constellation has already
   * created the webview. The iframe should adopt the pre-created webview.
   */
  adoptNewWebView: function (url, webviewId, browsingContextId, pipelineId) {
    console.log(
      "servo.adoptNewWebView:",
      url,
      webviewId,
      browsingContextId,
      pipelineId,
    );

    const attrs = {
      "adopt-webview-id": webviewId,
      "adopt-browsing-context-id": browsingContextId,
      "adopt-pipeline-id": pipelineId,
    };
    try {
      const webView = new WebView(url, "", attrs);
      layoutManager.addWebView(webView);
    } catch (e) {
      console.error(e);
    }
  },

  openKeyboard: function (inputType, currentValue, position) {
    console.log(`servo.openKeyboard: ${inputType} ${currentValue} ${position}`);
    openVirtualKeyboard({
      inputType,
      currentValue,
      placeholder: "",
      position,
    });
  },

  closeEmbedderControl: function (controlId) {
    console.log(`servo.closeEmbedderControl ${controlId}`);
    // TODO: proper control tracking (add controlId to openKeyboard).
    closeVirtualKeyboard();
  },
};

function createNewView() {
  const webView = new WebView(NEW_FRAME_DEFAULT_URL, "", {});
  layoutManager.addWebView(webView);
}

function switchToHomescreen() {
  if (layoutManager.homescreenWebviewId) {
    layoutManager.setActiveWebView(layoutManager.homescreenWebviewId);
  }
}

function openSettingsView() {
  const settingsView = new WebView(
    "http://settings.localhost:8888/index.html",
    "Settings",
    {},
  );
  layoutManager.addWebView(settingsView);
}

// Update the mobile tab overview with current tabs
function updateTabOverviewData() {
  if (!mobileOverview || !layoutManager) {
    return;
  }

  // Use getOverviewTabs() if available (excludes homescreen in mobile mode)
  let tabs;
  if (layoutManager.getOverviewTabs) {
    tabs = layoutManager.getOverviewTabs();
  } else {
    tabs = [];
    for (const [id, entry] of layoutManager.webviews) {
      tabs.push({
        id: id,
        title: entry.webview.title || "Untitled",
        favicon: entry.webview.favicon || "",
        screenshotUrl: entry.webview.screenshotUrl || null,
      });
    }
  }

  mobileOverview.tabs = tabs;
  mobileOverview.activeTabId = layoutManager.activeWebviewId;

  // Also update notification sheet tab count
  if (mobileNotificationSheet) {
    mobileNotificationSheet.tabCount = tabs.length;
  }
}

// Handle radial menu actions
function handleRadialMenuAction(detail) {
  const { action, originX, originY } = detail;

  switch (action) {
    case "new-view":
      createNewView();
      break;
    case "home":
      switchToHomescreen();
      break;
    case "back":
      layoutManager.goBack();
      break;
    case "forward":
      layoutManager.goForward();
      break;
    case "reload":
      layoutManager.reload();
      break;
    case "close-view":
      if (layoutManager.activeWebviewId) {
        layoutManager.removeWebView(layoutManager.activeWebviewId);
      }
      break;
    case "overview":
      layoutManager.showOverview();
      break;
    case "settings":
      openSettingsView();
      break;
    case "context-menu":
      // Show the full context menu from the radial menu
      const entry = layoutManager.getActiveEntry();
      if (entry?.webview?.showPendingContextMenu) {
        entry.webview.showPendingContextMenu();
      }
      break;
    default:
      console.log(`[RadialMenu] Unsupported action: ${action}`);
  }
}

// Global keyboard shortcuts

// New tab
Mousetrap.bindGlobal("mod+t", (e) => {
  createNewView();
  return false;
});

// New OS Window
Mousetrap.bindGlobal("mod+n", (e) => {
  console.log("Requesting new OS window");
  navigator.embedder.openNewOSWindow("http://system.localhost:8888/index.html");
  return false;
});

Mousetrap.bindGlobal("mod+f", (e) => {
  console.log("Requesting new floating search window");
  navigator.embedder.openNewOSWindow(
    "http://system.localhost:8888/search.html",
    "notitle,ontop",
  );
  return false;
});

// Close current tab
Mousetrap.bindGlobal("mod+w", (e) => {
  if (layoutManager.activeWebviewId) {
    layoutManager.removeWebView(layoutManager.activeWebviewId);
  }
  return false;
});

// Close current OS Window
Mousetrap.bindGlobal("mod+shift+w", (e) => {
  console.log("Closing current OS window");
  navigator.embedder.closeCurrentOSWindow();
  return false;
});

// Exit application
Mousetrap.bindGlobal("mod+q", (e) => {
  console.log("Bye bye");
  navigator.embedder.exit();
  return false;
});

// Whole system UI reload
Mousetrap.bindGlobal("mod+shift+r", (e) => {
  window.location.reload();
  return false;
});

// Navigate to next panel (Cmd+] or Ctrl+Tab)
Mousetrap.bindGlobal(["mod+]", "ctrl+tab"], (e) => {
  layoutManager.nextPanel();
  return false;
});

// Navigate to previous panel (Cmd+[ or Ctrl+Shift+Tab)
Mousetrap.bindGlobal(["mod+[", "ctrl+shift+tab"], (e) => {
  layoutManager.prevPanel();
  return false;
});

// Toggle overview mode (Cmd+E or Ctrl+E)
Mousetrap.bindGlobal("mod+e", (e) => {
  layoutManager.toggleOverview();
  return false;
});

// Exit overview mode or close system menu with Escape
Mousetrap.bindGlobal("escape", (e) => {
  if (mobileRadialMenu && mobileRadialMenu.open) {
    mobileRadialMenu.hide();
    return false;
  }
  if (mobileOverview && mobileOverview.open) {
    mobileOverview.open = false;
    return false;
  }
  if (mobileNotificationSheet && mobileNotificationSheet.open) {
    mobileNotificationSheet.open = false;
    return false;
  }
  if (mobileActionBar && mobileActionBar.open) {
    mobileActionBar.hide();
    return false;
  }
  if (notificationPanel && notificationPanel.open) {
    notificationPanel.open = false;
    return false;
  }
  if (systemMenu && systemMenu.open) {
    systemMenu.open = false;
    return false;
  }
  if (layoutManager.overviewMode) {
    layoutManager.hideOverview();
    return false;
  }
});

// Open URL bar on active webview (Cmd+L or Ctrl+L)
Mousetrap.bindGlobal("mod+l", (e) => {
  if (layoutManager.activeWebviewId) {
    const entry = layoutManager.webviews.get(layoutManager.activeWebviewId);
    if (entry) {
      entry.webview.openUrlBar(e);
    }
  }
  return false;
});

async function openVirtualKeyboard(detail) {
  // Check if virtual keyboard is enabled.
  if (
    !navigator.servo.getBoolPreference(
      "browserhtml.ime_virtual_keyboard_enabled",
    )
  ) {
    return;
  }

  // Load the keyboard if needed.
  let keyboardFrame = document.getElementById("keyboard-frame");
  if (!keyboardFrame) {
    keyboardFrame = document.createElement("iframe");
    keyboardFrame.setAttribute("embed", true);
    keyboardFrame.setAttribute("hidefocus", true);
    keyboardFrame.setAttribute("id", "keyboard-frame");
    keyboardFrame.setAttribute("src", "//keyboard.localhost:8888/index.html");
    document.getElementById("footer").append(keyboardFrame);
    // Wait for the frame to be loaded.
    let loaded = new Promise((resolve) => {
      keyboardFrame.addEventListener("embedloadstatuschange", (event) => {
        if (event.detail == "complete") {
          resolve();
        }
      });
    });
    await loaded;
  }

  console.log("[VirtualKeyboard] Show event received:", detail);
  const footer = document.getElementById("footer");
  footer.classList.add("visible");
  document.body.classList.add("keyboard-open");

  // Send input context to keyboard iframe

  if (keyboardFrame.contentWindow) {
    keyboardFrame.contentWindow.postMessage(
      {
        type: "show",
        inputType: detail.inputType,
        currentValue: detail.currentValue,
        placeholder: detail.placeholder,
      },
      "*",
    );
  }
}

function closeVirtualKeyboard() {
  const footer = document.getElementById("footer");
  footer.classList.remove("visible");
  document.body.classList.remove("keyboard-open");

  // Notify keyboard iframe
  const keyboardFrame = document.getElementById("keyboard-frame");
  if (keyboardFrame.contentWindow) {
    keyboardFrame.contentWindow.postMessage({ type: "hide" }, "*");
  }
}

// Initialize on DOM ready
document.addEventListener("DOMContentLoaded", () => {
  layoutManager = createLayoutManager(document.getElementById("root"), () => {
    return new WebView(NEW_FRAME_DEFAULT_URL, "", {});
  });

  // Setup mobile action bar if in mobile mode
  if (isMobileMode) {
    // Create homescreen webview first
    const homescreenUrl = navigator.servo.getStringPreference(
      "browserhtml.homescreen_url",
    );
    const homescreen = new WebView(homescreenUrl, "Home", {});
    layoutManager.addWebView(homescreen);
    layoutManager.setHomescreen(homescreen.webviewId);

    // Create mobile action bar
    mobileActionBar = document.createElement("mobile-action-bar");
    document.body.appendChild(mobileActionBar);
    mobileActionBar.setLayoutManager(layoutManager);
    layoutManager.setActionBar(mobileActionBar);

    // Handle new tab action from action bar
    mobileActionBar.addEventListener("action-new-tab", () => {
      createNewView();
    });

    // Handle home action from action bar
    mobileActionBar.addEventListener("action-home", () => {
      switchToHomescreen();
    });

    // Create mobile notification sheet
    mobileNotificationSheet = document.createElement(
      "mobile-notification-sheet",
    );
    document.body.appendChild(mobileNotificationSheet);
    mobileNotificationSheet.tabCount = layoutManager.getTabCount();

    mobileNotificationSheet.addEventListener("notification-click", (e) => {
      const notification = e.detail.notification;
      if (notification.webviewId && layoutManager.webviews) {
        for (const [id, entry] of layoutManager.webviews) {
          if (id.toString() === notification.webviewId) {
            layoutManager.setActiveWebView(id);
            break;
          }
        }
      }
      dismissNotification(notification);
      mobileNotificationSheet.open = false;
    });

    mobileNotificationSheet.addEventListener("notification-dismiss", (e) => {
      dismissNotification(e.detail.notification);
      mobileNotificationSheet.notifications = [...notifications];
    });

    mobileNotificationSheet.addEventListener("notification-clear-all", () => {
      clearAllNotifications();
      mobileNotificationSheet.notifications = [];
    });

    mobileNotificationSheet.addEventListener("sheet-closed", () => {
      mobileNotificationSheet.open = false;
    });

    // Create mobile tab overview
    mobileOverview = document.createElement("mobile-overview");
    document.body.appendChild(mobileOverview);

    mobileOverview.addEventListener("tab-select", (e) => {
      layoutManager.setActiveWebView(e.detail.tabId);
    });

    mobileOverview.addEventListener("tab-close", (e) => {
      layoutManager.removeWebView(e.detail.tabId);
      updateTabOverviewData();
    });

    mobileOverview.addEventListener("tab-new", () => {
      createNewView();
      mobileOverview.open = false;
    });

    mobileOverview.addEventListener("tab-home", () => {
      switchToHomescreen();
      mobileOverview.open = false;
    });

    mobileOverview.addEventListener("overview-close", () => {
      mobileOverview.open = false;
    });

    // Create mobile radial menu
    mobileRadialMenu = document.createElement("mobile-radial-menu");
    document.body.appendChild(mobileRadialMenu);

    // Radial menu is now triggered via contextmenu event from webviews,
    // not via the gesture handler's long-press detection.

    mobileRadialMenu.addEventListener("radial-action", (e) => {
      handleRadialMenuAction(e.detail);
    });

    // Handle radial menu dismiss (close without action)
    mobileRadialMenu.addEventListener("radial-dismiss", () => {
      // Dismiss pending context menu on active webview
      const entry = layoutManager.getActiveEntry();
      if (entry?.webview?.dismissPendingContextMenu) {
        entry.webview.dismissPendingContextMenu();
      }
    });

    // Override layout manager's showOverview to use mobile tab overview
    layoutManager.showOverview = function () {
      // Capture screenshots for all tabs before showing overview
      layoutManager.captureAllScreenshots();
      updateTabOverviewData();
      mobileOverview.open = true;
    };

    layoutManager.hideOverview = function () {
      mobileOverview.open = false;
    };
  }

  // Desktop-only header handlers
  const headerSpan = document.querySelector("header span");
  if (headerSpan) {
    headerSpan.onmousedown = (e) => {
      e.preventDefault();
      navigator.embedder.startWindowDrag();
    };
  }

  const headerResize = document.querySelector("header .resize");
  if (headerResize) {
    headerResize.onmousedown = (e) => {
      e.preventDefault();
      navigator.embedder.startWindowResize();
    };
  }

  const headerClose = document.querySelector("header .close");
  if (headerClose) {
    headerClose.onmousedown = (e) => {
      e.preventDefault();
      navigator.embedder.closeCurrentOSWindow();
    };
  }

  const plusIcon = document.getElementById("plus-icon");
  if (plusIcon) {
    plusIcon.onclick = () => {
      createNewView();
    };
  }

  // Setup system menu (desktop only)
  if (!isMobileMode) {
    systemMenu = document.createElement("system-menu");
    document.body.appendChild(systemMenu);

    const menuIcon = document.getElementById("menu-icon");
    if (menuIcon) {
      menuIcon.onclick = () => {
        systemMenu.open = !systemMenu.open;
      };
    }

    systemMenu.addEventListener("menu-action", (e) => {
      switch (e.detail.action) {
        case "new-tab":
          createNewView();
          break;
        case "new-window":
          navigator.embedder.openNewOSWindow(
            "http://system.localhost:8888/index.html",
          );
          break;
        case "new-search":
          navigator.embedder.openNewOSWindow(
            "http://system.localhost:8888/search.html",
            "notitle,ontop",
          );
          break;
        case "overview":
          layoutManager.toggleOverview();
          break;
        case "settings":
          openSettingsView();
          break;
        case "reload-ui":
          window.location.reload();
          break;
        case "quit":
          navigator.embedder.exit();
          break;
      }
    });
  }

  // Setup notification panel
  notificationPanel = document.getElementById("notification-panel");

  const notificationsIconContainer = document.getElementById(
    "notifications-icon-container",
  );
  if (notificationsIconContainer) {
    notificationsIconContainer.onclick = () => {
      notificationPanel.open = !notificationPanel.open;
    };
  }

  // Listen for mobile notification show event (from edge gesture)
  document.addEventListener("mobile-show-notifications", () => {
    if (isMobileMode && mobileNotificationSheet) {
      mobileNotificationSheet.notifications = [...notifications];
      mobileNotificationSheet.tabCount = layoutManager.getTabCount();
      mobileNotificationSheet.open = true;
    } else if (notificationPanel) {
      notificationPanel.open = true;
    }
  });

  notificationPanel.addEventListener("notification-click", (e) => {
    const notification = e.detail.notification;

    // Focus the source webview if possible
    if (notification.webviewId && layoutManager.webviews) {
      // Try to find the webview by its ID
      for (const [id, entry] of layoutManager.webviews) {
        if (id.toString() === notification.webviewId) {
          layoutManager.setActiveWebView(id);
          layoutManager.scrollToPanel(entry.panelIndex);
          break;
        }
      }
    }

    // Dismiss the notification (no actions supported for now)
    dismissNotification(notification);

    // Close the panel
    notificationPanel.open = false;
  });

  notificationPanel.addEventListener("notification-dismiss", (e) => {
    dismissNotification(e.detail.notification);
  });

  notificationPanel.addEventListener("notification-clear-all", () => {
    clearAllNotifications();
  });

  notificationPanel.addEventListener("panel-closed", () => {
    notificationPanel.open = false;
  });

  // Listen for notifications from webviews
  document
    .getElementById("root")
    .addEventListener("webview-notification", (e) => {
      console.log("[Notification] Received from webview:", e.detail);
      addNotification({
        webviewId: e.detail.webviewId?.toString(),
        title: e.detail.title,
        body: e.detail.body,
        tag: e.detail.tag,
        iconUrl: e.detail.iconUrl,
      });
    });

  // Listen for virtual keyboard show/hide events from webviews
  document
    .getElementById("root")
    .addEventListener("webview-inputmethod-show", (event) => {
      openVirtualKeyboard(event.detail);
    });

  document
    .getElementById("root")
    .addEventListener("webview-inputmethod-hide", () => {
      console.log("[Keyboard] Hide event received");
      closeVirtualKeyboard();
    });

  // Listen for radial menu show events from webviews (mobile mode)
  document
    .getElementById("root")
    .addEventListener("webview-show-radial-menu", (e) => {
      if (isMobileMode && mobileRadialMenu) {
        console.log("[RadialMenu] Show event received from webview:", e.detail);
        mobileRadialMenu.canGoBack = e.detail.canGoBack;
        mobileRadialMenu.canGoForward = e.detail.canGoForward;
        mobileRadialMenu.isHomescreen =
          layoutManager.activeWebviewId === layoutManager.homescreenWebviewId;
        mobileRadialMenu.show(e.detail.x, e.detail.y, e.detail.contextMenu);
      }
    });

  const params = new URLSearchParams(window.location.search);
  const openValue = params.get("open");
  if (openValue) {
    const webView = new WebView(openValue, "", {});
    layoutManager.addWebView(webView);
  } else if (!isMobileMode) {
    // In mobile mode, homescreen is already created as the initial view
    createNewView();
  }

  // BroadcastChannel for receiving URLs from search window
  const searchChannel = new BroadcastChannel("servo-search");
  searchChannel.onmessage = (e) => {
    if (e.data.type === "discover") {
      // Respond to discovery with our window ID
      searchChannel.postMessage({ type: "available", windowId: windowId });
    } else if (
      e.data.type === "openUrl" &&
      e.data.targetWindowId === windowId
    ) {
      // This message is targeted to us - handle it
      searchChannel.postMessage({ type: "ack", id: e.data.id });
      const webView = new WebView(e.data.url, "", {});
      layoutManager.addWebView(webView);
    } else if (e.data.type === "listWebViews") {
      // Respond with list of our web-views
      const webviews = [];
      for (const [webviewId, entry] of layoutManager.webviews) {
        webviews.push({
          webviewId: webviewId,
          title: entry.webview.title || "",
          url: entry.webview.url || "",
        });
      }
      searchChannel.postMessage({
        type: "webviewList",
        windowId: windowId,
        webviews: webviews,
      });
    } else if (
      e.data.type === "selectWebView" &&
      e.data.targetWindowId === windowId
    ) {
      // Select and focus the specified web-view
      searchChannel.postMessage({ type: "ack", id: e.data.id });
      layoutManager.setActiveWebView(e.data.webviewId);
      const entry = layoutManager.webviews.get(e.data.webviewId);
      if (entry) {
        layoutManager.scrollToPanel(entry.panelIndex);
      }
    }
  };
});
