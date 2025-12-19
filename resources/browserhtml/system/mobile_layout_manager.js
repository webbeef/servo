// SPDX-License-Identifier: AGPL-3.0-or-later

import { EdgeGestureHandler } from "./edge_gesture_handler.js";

export class MobileLayoutManager {
  constructor(rootElement, webViewBuilder) {
    this.root = rootElement;
    this.webViewBuilder = webViewBuilder;

    // Linear array of webviews (carousel model)
    this.webviews = new Map(); // webviewId -> { webview, index }
    this.webviewOrder = []; // Array of webviewIds in order
    this.activeIndex = -1;
    this.activeWebviewId = null;
    this.nextId = 1;

    // Homescreen webview (special, cannot be closed, excluded from overview/swipe)
    this.homescreenWebviewId = null;

    // Overview mode state
    this.overviewMode = false;
    this.overviewElement = null;

    // Gesture handler
    this.gestureHandler = new EdgeGestureHandler(this);

    // Action bar component
    this.actionBar = null;

    // Setup event listeners
    this.setupEventListeners();
  }

  setupEventListeners() {
    document.addEventListener("webview-close", (e) => {
      this.removeWebView(e.detail.webviewId);
    });

    document.addEventListener("webview-focus", (e) => {
      this.setActiveWebView(e.detail.webviewId);
    });

    // Listen for navigation state changes from webviews
    document.addEventListener("navigation-state-changed", (e) => {
      // Only update if it's from the active webview
      if (e.detail.webviewId === this.activeWebviewId && this.actionBar) {
        this.actionBar.updateState();
      }
    });
  }

  generateId() {
    return `wv-${this.nextId++}`;
  }

  // Set the homescreen webview
  setHomescreen(webviewId) {
    this.homescreenWebviewId = webviewId;
  }

  // Check if a webview is the homescreen
  isHomescreen(webviewId) {
    return webviewId === this.homescreenWebviewId;
  }

  // Get the currently active webview entry
  getActiveEntry() {
    if (this.activeWebviewId) {
      return this.webviews.get(this.activeWebviewId);
    }
    return null;
  }

  // Add a new webview to the carousel
  addWebView(webview) {
    const id = this.generateId();
    webview.webviewId = id;

    // Mark webview as mobile mode for styling
    webview.classList.add("mobile-mode");

    // Add to the end of the order
    const index = this.webviewOrder.length;
    this.webviewOrder.push(id);
    this.webviews.set(id, { webview, index });

    // Create container for the webview
    const container = document.createElement("div");
    container.className = "mobile-webview-container";
    container.dataset.webviewId = id;
    container.appendChild(webview);
    this.root.appendChild(container);

    // Make this the active webview
    this.setActiveWebView(id);

    return webview;
  }

  // Remove a webview from the carousel
  removeWebView(webviewId) {
    // Prevent closing the homescreen
    if (this.isHomescreen(webviewId)) {
      console.warn("[MobileLayoutManager] Cannot close homescreen");
      return;
    }

    const entry = this.webviews.get(webviewId);
    if (!entry) {
      return;
    }

    const { webview, index } = entry;

    // Remove from DOM
    const container = this.root.querySelector(
      `.mobile-webview-container[data-webview-id="${webviewId}"]`
    );
    if (container) {
      container.remove();
    }

    // Remove from tracking
    this.webviews.delete(webviewId);
    this.webviewOrder = this.webviewOrder.filter((id) => id !== webviewId);

    // Update indices for remaining webviews
    this.webviewOrder.forEach((id, newIndex) => {
      const e = this.webviews.get(id);
      if (e) {
        e.index = newIndex;
      }
    });

    // If we removed the active webview, activate another one
    if (this.activeWebviewId === webviewId) {
      if (this.webviewOrder.length > 0) {
        // Activate the webview at the same position or the last one
        const newIndex = Math.min(index, this.webviewOrder.length - 1);
        this.setActiveWebView(this.webviewOrder[newIndex]);
      } else {
        this.activeWebviewId = null;
        this.activeIndex = -1;
      }
    }
  }

  // Set the active webview by ID
  setActiveWebView(webviewId) {
    // Remove active state from previous and capture its screenshot
    if (this.activeWebviewId && this.webviews.has(this.activeWebviewId)) {
      const prevEntry = this.webviews.get(this.activeWebviewId);
      prevEntry.webview.active = false;

      // Capture screenshot of the tab we're switching away from
      if (prevEntry.webview.captureScreenshot) {
        prevEntry.webview.captureScreenshot();
      }

      const prevContainer = this.root.querySelector(
        `.mobile-webview-container[data-webview-id="${this.activeWebviewId}"]`
      );
      if (prevContainer) {
        prevContainer.classList.remove("active");
      }
    }

    this.activeWebviewId = webviewId;

    // Set active state on new
    if (webviewId && this.webviews.has(webviewId)) {
      const entry = this.webviews.get(webviewId);
      entry.webview.active = true;
      this.activeIndex = entry.index;
      const container = this.root.querySelector(
        `.mobile-webview-container[data-webview-id="${webviewId}"]`
      );
      if (container) {
        container.classList.add("active");
      }
    }
  }

  // Switch to webview at given index
  switchTo(index) {
    if (index < 0 || index >= this.webviewOrder.length) {
      return;
    }
    const webviewId = this.webviewOrder[index];
    this.setActiveWebView(webviewId);
  }

  // Navigate to next webview in carousel (skips homescreen)
  nextWebView() {
    const regularViews = this.webviewOrder.filter((id) => !this.isHomescreen(id));
    if (regularViews.length <= 1) {
      return;
    }

    const currentIndex = regularViews.indexOf(this.activeWebviewId);
    if (currentIndex === -1) {
      // Currently on homescreen, switch to first regular view
      this.setActiveWebView(regularViews[0]);
    } else {
      const nextIndex = (currentIndex + 1) % regularViews.length;
      this.setActiveWebView(regularViews[nextIndex]);
    }
  }

  // Navigate to previous webview in carousel (skips homescreen)
  prevWebView() {
    const regularViews = this.webviewOrder.filter((id) => !this.isHomescreen(id));
    if (regularViews.length <= 1) {
      return;
    }

    const currentIndex = regularViews.indexOf(this.activeWebviewId);
    if (currentIndex === -1) {
      // Currently on homescreen, switch to last regular view
      this.setActiveWebView(regularViews[regularViews.length - 1]);
    } else {
      const prevIndex =
        (currentIndex - 1 + regularViews.length) % regularViews.length;
      this.setActiveWebView(regularViews[prevIndex]);
    }
  }

  // Get adjacent webview ID for peek preview (skips homescreen)
  getAdjacentWebViewId(direction) {
    const regularViews = this.webviewOrder.filter((id) => !this.isHomescreen(id));
    if (regularViews.length <= 1) {
      return null;
    }

    const currentIndex = regularViews.indexOf(this.activeWebviewId);
    if (currentIndex === -1) {
      // On homescreen, no peek
      return null;
    }

    if (direction === "next") {
      const nextIndex = (currentIndex + 1) % regularViews.length;
      return regularViews[nextIndex];
    } else {
      const prevIndex =
        (currentIndex - 1 + regularViews.length) % regularViews.length;
      return regularViews[prevIndex];
    }
  }

  // Get webview info for peek preview
  getWebViewInfo(webviewId) {
    const entry = this.webviews.get(webviewId);
    if (!entry) {
      return null;
    }
    return {
      id: webviewId,
      title: entry.webview.title || "Untitled",
      favicon: entry.webview.favicon || "",
      screenshotUrl: entry.webview.screenshotUrl || null,
    };
  }

  // Show action bar (bottom slide-up)
  // Not allowed when on homescreen
  showActionBar() {
    if (this.isHomescreen(this.activeWebviewId)) {
      return;
    }
    if (this.actionBar) {
      this.actionBar.show();
    }
  }

  // Hide action bar
  hideActionBar() {
    if (this.actionBar) {
      this.actionBar.hide();
    }
  }

  // Toggle action bar
  toggleActionBar() {
    if (this.actionBar) {
      this.actionBar.toggle();
    }
  }

  // Set the action bar component reference
  setActionBar(actionBar) {
    this.actionBar = actionBar;
  }

  // Perform navigation action on active webview
  goBack() {
    const entry = this.getActiveEntry();
    if (entry) {
      entry.webview.goBack();
    }
  }

  goForward() {
    const entry = this.getActiveEntry();
    if (entry) {
      entry.webview.goForward();
    }
  }

  reload() {
    const entry = this.getActiveEntry();
    if (entry) {
      entry.webview.doReload();
    }
  }

  // Navigate to URL in active webview
  navigateTo(url) {
    const entry = this.getActiveEntry();
    if (entry) {
      entry.webview.ensureIframe();
      entry.webview.iframe.load(url);
    }
  }

  // Get current URL of active webview
  getCurrentUrl() {
    const entry = this.getActiveEntry();
    if (entry) {
      return entry.webview.currentUrl || "";
    }
    return "";
  }

  // Get navigation state of active webview
  getNavigationState() {
    const entry = this.getActiveEntry();
    if (entry) {
      return {
        canGoBack: entry.webview.canGoBack || false,
        canGoForward: entry.webview.canGoForward || false,
      };
    }
    return { canGoBack: false, canGoForward: false };
  }

  // Get tab count (excludes homescreen)
  getTabCount() {
    return this.webviewOrder.filter((id) => !this.isHomescreen(id)).length;
  }

  // Get tabs for overview (excludes homescreen)
  getOverviewTabs() {
    return this.webviewOrder
      .filter((id) => !this.isHomescreen(id))
      .map((id) => {
        const entry = this.webviews.get(id);
        return {
          id,
          title: entry.webview.title || "Untitled",
          favicon: entry.webview.favicon || "",
          screenshotUrl: entry.webview.screenshotUrl || null,
        };
      });
  }

  // Capture screenshots for all webviews (for tab overview)
  captureAllScreenshots() {
    for (const [id, entry] of this.webviews) {
      if (entry.webview.captureScreenshot) {
        entry.webview.captureScreenshot();
      }
    }
  }

  // ============================================================================
  // Overview Mode (for compatibility, minimal implementation for MVP)
  // ============================================================================

  toggleOverview() {
    if (this.overviewMode) {
      this.hideOverview();
    } else {
      this.showOverview();
    }
  }

  showOverview() {
    this.overviewMode = true;
    // TODO: Implement mobile tab overview grid
    console.log("[MobileLayoutManager] Overview mode enabled (not yet implemented)");
  }

  hideOverview() {
    this.overviewMode = false;
    console.log("[MobileLayoutManager] Overview mode disabled");
  }

  // Navigation methods for compatibility with desktop shortcuts
  nextPanel() {
    this.nextWebView();
  }

  prevPanel() {
    this.prevWebView();
  }

  scrollToPanel(index) {
    this.switchTo(index);
  }
}
