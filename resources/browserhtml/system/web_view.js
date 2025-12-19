// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";
import "./webview_menu.js";
import "./url_bar_overlay.js";
import "./context_menu.js";
import "./select_control.js";
import "./color_picker.js";

export class WebView extends LitElement {
  constructor(src, title, attrs = {}) {
    super();

    this.src = src;
    this.title = title;
    this.favicon = "";
    this.canGoBack = false;
    this.canGoForward = false;
    this.themeColor = WebView.defaultThemeColor;
    this.active = false;

    this.iframe = undefined;
    this.attrs = attrs;
    this.webviewId = null; // Set by LayoutManager
    this.menuOpen = false;
    this.urlBarOpen = false;
    this.currentUrl = src || "";

    // Cached screenshot for overview mode
    this.screenshotUrl = null;

    // Context menu state
    this.contextMenu = null;

    // Select control state
    this.selectControl = null;

    // Color picker state
    this.colorPicker = null;

    // Load status for progress indicator
    this.loadStatus = "idle";
  }

  handleMenuAction(e) {
    const action = e.detail.action;
    switch (action) {
      case "split-horizontal":
        this.splitHorizontal();
        break;
      case "split-vertical":
        this.splitVertical();
        break;
      case "reduce-size":
        this.resizePanel(-10);
        break;
      case "increase-size":
        this.resizePanel(10);
        break;
      case "zoom-in":
        this.zoomIn();
        break;
      case "zoom-out":
        this.zoomOut();
        break;
      case "zoom-reset":
        this.zoomReset();
        break;
    }
  }

  handleMenuClosed() {
    this.menuOpen = false;
  }

  resizePanel(delta) {
    this.dispatchEvent(
      new CustomEvent("webview-resize-panel", {
        bubbles: true,
        detail: { webviewId: this.webviewId, delta },
      }),
    );
  }

  zoomIn() {
    this.ensureIframe();
    if (this.iframe) {
      const currentZoom = this.iframe.getPageZoom();
      console.log("Current zoom:", currentZoom);
      this.iframe.setPageZoom(currentZoom + 0.1);
    }
  }

  zoomOut() {
    this.ensureIframe();
    if (this.iframe) {
      const currentZoom = this.iframe.getPageZoom();
      console.log("Current zoom:", currentZoom);
      this.iframe.setPageZoom(currentZoom - 0.1);
    }
  }

  zoomReset() {
    this.ensureIframe();
    if (this.iframe) {
      this.iframe.setPageZoom(1.0);
    }
  }

  connectedCallback() {
    super.connectedCallback();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
  }

  static defaultThemeColor = "gray";

  static properties = {
    src: {},
    title: { state: true },
    favicon: { state: true },
    canGoBack: { state: true },
    canGoForward: { state: true },
    themeColor: { state: true },
    active: { state: true },
    menuOpen: { state: true },
    menuPosition: { state: true },
    urlBarOpen: { state: true },
    currentUrl: { state: true },
    contextMenu: { state: true },
    selectControl: { state: true },
    colorPicker: { state: true },
    loadStatus: { state: true },
  };

  ensureIframe() {
    if (!this.iframe) {
      this.iframe = this.shadowRoot.querySelector("iframe");
      // Update the screenshot when resizing the web-view
      const resizeObserver = new ResizeObserver((entries) => {
        this.captureScreenshot();
      });
      resizeObserver.observe(this);
    }
  }

  // Get the content iframe element (for screenshot capture, etc.)
  getContentIframe() {
    this.ensureIframe();
    return this.iframe;
  }

  ontitlechange(event) {
    if (event.detail) {
      console.log(`ontitlechange: ${event.detail}`);
      this.title = event.detail;
    }
  }

  onfaviconchange(event) {
    const blob = event.detail;
    if (blob) {
      // Revoke old URL to free memory
      if (this.favicon && this.favicon.startsWith("blob:")) {
        URL.revokeObjectURL(this.favicon);
      }
      this.favicon = URL.createObjectURL(blob);

      // Notify parent (LayoutManager) about favicon change
      this.dispatchEvent(
        new CustomEvent("webview-favicon-change", {
          bubbles: true,
          detail: { webviewId: this.webviewId, favicon: this.favicon },
        }),
      );
    }
  }

  onthemecolorchange(event) {
    this.themeColor = event.detail;
  }

  onloadstatuschange(event) {
    console.log("[WebView] Load status changed:", event.detail);
    this.loadStatus = event.detail;

    // Auto-reset to idle after complete animation finishes
    if (event.detail === "complete") {
      setTimeout(() => {
        this.loadStatus = "idle";
      }, 500);
    }
  }

  oncontrolshow(event) {
    const detail = event.detail;
    console.log("[EmbedderControl] SHOW event received:", detail);

    if (detail.controlType === "select" && detail.selectParameters?.options) {
      const params = detail.selectParameters;

      // Show the select control
      this.selectControl = {
        controlId: detail.controlId,
        options: params.options,
        selectedIndex: params.selectedIndex,
        x: detail.position?.x || 0,
        y: detail.position?.y || 0,
      };
    } else if (detail.controlType === "color" && detail.colorParameters) {
      const params = detail.colorParameters;

      // Show the color picker
      this.colorPicker = {
        controlId: detail.controlId,
        currentColor: params.currentColor,
        x: detail.position?.x || 0,
        y: detail.position?.y || 0,
      };
    } else if (
      detail.controlType === "contextmenu" &&
      detail.contextMenuParameters?.items
    ) {
      const params = detail.contextMenuParameters;

      // Map action IDs to icons (matching title bar icons)
      const actionIcons = {
        GoBack: "arrow-left",
        GoForward: "arrow-right",
        Reload: "rotate-ccw",
      };

      // Enrich items with icons
      const items = params.items.map((item) => ({
        ...item,
        icon: actionIcons[item.id] || item.icon,
      }));

      // In mobile mode, show the radial menu instead of the regular context menu
      if (document.body.classList.contains("mobile-mode")) {
        // Extract navigation state from context menu items
        const navState = {
          canGoBack: params.items.some(
            (item) => item.id === "GoBack" && !item.disabled,
          ),
          canGoForward: params.items.some(
            (item) => item.id === "GoForward" && !item.disabled,
          ),
        };

        // Filter items to remove actions that are part of radial menu
        const filteredItems = items.filter(
          (item) =>
            item.id !== "GoBack" &&
            item.id !== "GoForward" &&
            item.id !== "Reload",
        );

        // Store pending context menu for later
        this.pendingContextMenu = {
          controlId: detail.controlId,
          items: filteredItems,
          x: detail.position?.x || 0,
          y: detail.position?.y || 0,
        };

        // Dispatch event to show radial menu at the touch position
        this.dispatchEvent(
          new CustomEvent("webview-show-radial-menu", {
            bubbles: true,
            composed: true,
            detail: {
              x: detail.position?.x || 0,
              y: detail.position?.y || 0,
              canGoBack: navState.canGoBack,
              canGoForward: navState.canGoForward,
              contextMenu: this.pendingContextMenu,
            },
          }),
        );

        // Don't respond yet - radial menu will handle it
        return;
      }

      // Show the context menu
      this.contextMenu = {
        controlId: detail.controlId,
        items,
        x: detail.position?.x || 0,
        y: detail.position?.y || 0,
      };
    } else if (
      detail.controlType === "permission" &&
      detail.permissionParameters
    ) {
      const params = detail.permissionParameters;

      // Show the permission prompt
      this.currentPermission = {
        controlId: detail.controlId,
        feature: params.feature,
        featureName: params.featureName,
      };
      this.requestUpdate();
    } else if (
      detail.controlType === "inputmethod" &&
      detail.inputMethodParameters
    ) {
      const params = detail.inputMethodParameters;

      // Bubble up to parent system window for virtual keyboard
      this.dispatchEvent(
        new CustomEvent("webview-inputmethod-show", {
          bubbles: true,
          composed: true,
          detail: {
            controlId: detail.controlId,
            inputType: params.inputType || "text",
            currentValue: params.currentValue || "",
            placeholder: params.placeholder || "",
            position: detail.position,
          },
        }),
      );
    }
  }

  oncontrolhide(event) {
    console.log("[EmbedderControl] HIDE event received:", event.detail);

    // Close context menu if it's the one being hidden
    if (
      this.contextMenu &&
      this.contextMenu.controlId === event.detail.controlId
    ) {
      this.contextMenu = null;
    }

    // Close select control if it's the one being hidden
    if (
      this.selectControl &&
      this.selectControl.controlId === event.detail.controlId
    ) {
      this.selectControl = null;
    }

    // Close color picker if it's the one being hidden
    if (
      this.colorPicker &&
      this.colorPicker.controlId === event.detail.controlId
    ) {
      this.colorPicker = null;
    }

    // Close permission prompt if it's the one being hidden
    if (
      this.currentPermission &&
      this.currentPermission.controlId === event.detail.controlId
    ) {
      this.currentPermission = null;
      this.requestUpdate();
    }

    // Bubble up inputmethod hide event to parent system window
    // We always send the hide event as it's handled at the system level
    this.dispatchEvent(
      new CustomEvent("webview-inputmethod-hide", {
        bubbles: true,
        composed: true,
        detail: { controlId: event.detail.controlId },
      }),
    );
  }

  ondialogshow(event) {
    const detail = event.detail;
    console.log("[EmbedderDialog] SHOW event received:", detail);
    const dialogType = detail.dialogType;
    const controlId = detail.controlId;
    const message = detail.message;
    const defaultValue = detail.defaultValue;

    // Store the current dialog info for rendering
    this.currentDialog = {
      type: dialogType,
      controlId,
      message,
      defaultValue: defaultValue || "",
    };
    this.requestUpdate();
  }

  onnotificationshow(event) {
    const detail = event.detail;
    console.log("[Notification] SHOW event received:", detail);

    // Dispatch the notification to the parent (index.js) for global handling
    // Include the webviewId so the notification center can focus the source webview
    this.dispatchEvent(
      new CustomEvent("webview-notification", {
        bubbles: true,
        composed: true,
        detail: {
          webviewId: this.webviewId,
          title: detail.title,
          body: detail.body,
          tag: detail.tag,
          iconUrl: detail.iconUrl,
        },
      }),
    );
  }

  handleDialogConfirm(inputValue = null) {
    this.ensureIframe();
    const dialog = this.currentDialog;
    if (!dialog) {
      return;
    }

    console.log("[EmbedderDialog] User confirmed dialog:", dialog.type);

    switch (dialog.type) {
      case "alert":
        this.iframe.respondToAlert(dialog.controlId);
        break;
      case "confirm":
        this.iframe.respondToConfirm(dialog.controlId, true);
        break;
      case "prompt":
        this.iframe.respondToPrompt(dialog.controlId, inputValue);
        break;
    }

    this.currentDialog = null;
    this.requestUpdate();
  }

  handleDialogCancel() {
    this.ensureIframe();
    const dialog = this.currentDialog;
    if (!dialog) {
      return;
    }

    console.log("[EmbedderDialog] User cancelled dialog:", dialog.type);

    switch (dialog.type) {
      case "alert":
        // Alert only has OK, but handle cancel just in case
        this.iframe.respondToAlert(dialog.controlId);
        break;
      case "confirm":
        this.iframe.respondToConfirm(dialog.controlId, false);
        break;
      case "prompt":
        this.iframe.respondToPrompt(dialog.controlId, null);
        break;
    }

    this.currentDialog = null;
    this.requestUpdate();
  }

  handlePermissionAllow() {
    this.ensureIframe();
    const permission = this.currentPermission;
    if (!permission) {
      return;
    }

    console.log(
      "[EmbedderPermission] User allowed permission:",
      permission.feature,
    );
    this.iframe.respondToPermissionPrompt(permission.controlId, true);
    this.currentPermission = null;
    this.requestUpdate();
  }

  handlePermissionDeny() {
    this.ensureIframe();
    const permission = this.currentPermission;
    if (!permission) {
      return;
    }

    console.log(
      "[EmbedderPermission] User denied permission:",
      permission.feature,
    );
    this.iframe.respondToPermissionPrompt(permission.controlId, false);
    this.currentPermission = null;
    this.requestUpdate();
  }

  handleContextMenuAction(e) {
    const { action, controlId } = e.detail;
    console.log(
      "[ContextMenu] Action selected:",
      action,
      "Control ID:",
      controlId,
    );

    this.ensureIframe();

    // Send the action back to the embedded webview for handling
    // The embedded webview will process the action (GoBack, Copy, Paste, etc.)
    this.iframe.respondToContextMenu(controlId, action);
    this.contextMenu = null;
  }

  handleContextMenuCancel(e) {
    const { controlId } = e.detail;
    console.log("[ContextMenu] Menu cancelled, Control ID:", controlId);

    this.ensureIframe();
    // Send null action to indicate cancellation
    this.iframe.respondToContextMenu(controlId, null);
    this.contextMenu = null;
  }

  handleContextMenuClosed() {
    this.contextMenu = null;
  }

  // Show the pending context menu (called from radial menu's context-menu action)
  showPendingContextMenu() {
    if (this.pendingContextMenu) {
      this.contextMenu = this.pendingContextMenu;
      this.pendingContextMenu = null;
    }
  }

  // Dismiss the pending context menu without showing it
  dismissPendingContextMenu() {
    if (this.pendingContextMenu) {
      this.ensureIframe();
      this.iframe.respondToContextMenu(this.pendingContextMenu.controlId, null);
      this.pendingContextMenu = null;
    }
  }

  handleSelectOption(e) {
    const { controlId, index } = e.detail;
    console.log(
      "[SelectControl] Option selected, index:",
      index,
      "Control ID:",
      controlId,
    );

    this.ensureIframe();
    this.iframe.respondToSelectControl(controlId, index);
    this.selectControl = null;
  }

  handleSelectCancel(e) {
    const { controlId } = e.detail;
    console.log("[SelectControl] Selection cancelled, Control ID:", controlId);

    this.ensureIframe();
    // Send -1 to indicate cancellation (no selection)
    this.iframe.respondToSelectControl(controlId, -1);
    this.selectControl = null;
  }

  handleSelectClosed() {
    this.selectControl = null;
  }

  handleColorConfirm(e) {
    const { controlId, color } = e.detail;
    console.log(
      "[ColorPicker] Color confirmed:",
      color,
      "Control ID:",
      controlId,
    );

    this.ensureIframe();
    this.iframe.respondToColorPicker(controlId, color);
    this.colorPicker = null;
  }

  handleColorCancel(e) {
    const { controlId } = e.detail;
    console.log("[ColorPicker] Color cancelled, Control ID:", controlId);

    this.ensureIframe();
    // Send null to indicate cancellation
    this.iframe.respondToColorPicker(controlId, null);
    this.colorPicker = null;
  }

  handleColorClosed() {
    this.colorPicker = null;
  }

  renderDialog() {
    if (!this.currentDialog) {
      return "";
    }

    const dialog = this.currentDialog;

    if (dialog.type === "alert") {
      return html`
        <div class="dialog-overlay" @click=${(e) => e.stopPropagation()}>
          <div class="dialog-box">
            <div class="dialog-message">${dialog.message}</div>
            <div class="dialog-buttons">
              <button
                class="dialog-button primary"
                @click=${() => this.handleDialogConfirm()}
              >
                OK
              </button>
            </div>
          </div>
        </div>
      `;
    } else if (dialog.type === "confirm") {
      return html`
        <div class="dialog-overlay" @click=${(e) => e.stopPropagation()}>
          <div class="dialog-box">
            <div class="dialog-message">${dialog.message}</div>
            <div class="dialog-buttons">
              <button
                class="dialog-button"
                @click=${() => this.handleDialogCancel()}
              >
                Cancel
              </button>
              <button
                class="dialog-button primary"
                @click=${() => this.handleDialogConfirm()}
              >
                OK
              </button>
            </div>
          </div>
        </div>
      `;
    } else if (dialog.type === "prompt") {
      return html`
        <div class="dialog-overlay" @click=${(e) => e.stopPropagation()}>
          <div class="dialog-box">
            <div class="dialog-message">${dialog.message}</div>
            <input
              type="text"
              class="dialog-input"
              .value=${dialog.defaultValue}
              @keydown=${(e) => {
                if (e.key === "Enter") {
                  this.handleDialogConfirm(e.target.value);
                } else if (e.key === "Escape") {
                  this.handleDialogCancel();
                }
              }}
              id="dialog-prompt-input"
            />
            <div class="dialog-buttons">
              <button
                class="dialog-button"
                @click=${() => this.handleDialogCancel()}
              >
                Cancel
              </button>
              <button
                class="dialog-button primary"
                @click=${() => {
                  const input = this.shadowRoot.querySelector(
                    "#dialog-prompt-input",
                  );
                  this.handleDialogConfirm(input?.value || "");
                }}
              >
                OK
              </button>
            </div>
          </div>
        </div>
      `;
    }

    return "";
  }

  renderColorPicker() {
    if (!this.colorPicker) {
      return "";
    }

    return html`<color-picker
      ?open=${this.colorPicker !== null}
      .currentColor=${this.colorPicker?.currentColor || "#000000"}
      .x=${this.colorPicker?.x || 0}
      .y=${this.colorPicker?.y || 0}
      .controlId=${this.colorPicker?.controlId || ""}
      @color-confirm=${this.handleColorConfirm}
      @color-cancel=${this.handleColorCancel}
    >
    </color-picker>`;
  }

  renderPermissionPrompt() {
    if (!this.currentPermission) {
      return "";
    }

    const permission = this.currentPermission;

    // Permission-specific icons
    const permissionIcons = {
      geolocation: "map-pin",
      camera: "camera",
      microphone: "mic",
      notifications: "bell",
      bluetooth: "bluetooth",
    };

    const icon = permissionIcons[permission.feature] || "shield";

    return html`
      <div class="dialog-overlay" @click=${(e) => e.stopPropagation()}>
        <div class="dialog-box permission-prompt">
          <div class="permission-icon">
            <lucide-icon name="${icon}"></lucide-icon>
          </div>
          <div class="permission-title">Permission Request</div>
          <div class="dialog-message">
            This site wants to use your
            <strong>${permission.featureName}</strong>.
          </div>
          <div class="dialog-buttons">
            <button
              class="dialog-button"
              @click=${() => this.handlePermissionDeny()}
            >
              Block
            </button>
            <button
              class="dialog-button primary"
              @click=${() => this.handlePermissionAllow()}
            >
              Allow
            </button>
          </div>
        </div>
      </div>
    `;
  }

  renderUrlBarOverlay() {
    if (!this.urlBarOpen) {
      return "";
    }

    return html`<url-bar-overlay
      ?open=${this.urlBarOpen}
      .url=${this.currentUrl}
      .onNavigate=${(url) => this.handleNavigate(url)}
      .onSelectWebView=${(windowId, webviewId) =>
        this.handleSelectWebView(windowId, webviewId)}
      @close=${this.closeUrlBar}
    ></url-bar-overlay>`;
  }

  onurlchange(event) {
    this.ensureIframe();
    this.canGoBack = this.iframe.canGoBack();
    this.canGoForward = this.iframe.canGoForward();
    this.currentUrl = event.detail;

    // Dispatch navigation state change event for action bar updates
    this.dispatchEvent(
      new CustomEvent("navigation-state-changed", {
        bubbles: true,
        composed: true,
        detail: {
          webviewId: this.webviewId,
          canGoBack: this.canGoBack,
          canGoForward: this.canGoForward,
          url: this.currentUrl,
        },
      })
    );

    // Capture screenshot for overview mode after a short delay
    // to allow the page to render
    this.captureScreenshot();
  }

  // Capture a screenshot and cache it for overview mode
  captureScreenshot() {
    // Debounce: clear any pending capture
    if (this.screenshotTimeout) {
      clearTimeout(this.screenshotTimeout);
    }

    // Delay capture to allow page to render
    this.screenshotTimeout = setTimeout(() => {
      this.getContentIframe()
        .takeScreenshot()
        .then((blob) => {
          if (blob) {
            // Revoke old URL to free memory
            if (this.screenshotUrl) {
              URL.revokeObjectURL(this.screenshotUrl);
            }
            this.screenshotUrl = URL.createObjectURL(blob);
          }
        })
        .catch(() => {
          // Ignore screenshot errors (e.g., for off-screen webviews)
        });
    }, 500);
  }

  onfocus() {
    this.dispatchEvent(
      new CustomEvent("webview-focus", {
        bubbles: true,
        detail: { webviewId: this.webviewId },
      }),
    );
  }

  doReload() {
    this.ensureIframe();
    this.iframe.reload();
  }

  goBack() {
    this.ensureIframe();
    this.themeColor = WebView.defaultThemeColor;
    this.iframe.goBack();
  }

  goForward() {
    this.ensureIframe();
    this.themeColor = WebView.defaultThemeColor;
    this.iframe.goForward();
  }

  close() {
    this.dispatchEvent(
      new CustomEvent("webview-close", {
        bubbles: true,
        detail: { webviewId: this.webviewId },
      }),
    );
  }

  split(direction) {
    this.dispatchEvent(
      new CustomEvent("webview-split", {
        bubbles: true,
        detail: { webviewId: this.webviewId, direction },
      }),
    );
  }

  splitHorizontal() {
    this.split("horizontal");
    this.menuOpen = false;
  }

  splitVertical() {
    this.split("vertical");
    this.menuOpen = false;
  }

  toggleMenu(e) {
    e.stopPropagation();
    if (!this.menuOpen) {
      // Get the position of the menu button relative to the web-view
      const buttonRect = e.target.getBoundingClientRect();
      const hostRect = this.getBoundingClientRect();
      this.menuPosition = {
        x: buttonRect.right - hostRect.left,
        y: buttonRect.bottom - hostRect.top,
      };
    }
    this.menuOpen = !this.menuOpen;
  }

  closeMenu() {
    this.menuOpen = false;
  }

  openUrlBar(e) {
    e.stopPropagation();
    if (this.active) {
      this.urlBarOpen = true;
    }
  }

  closeUrlBar() {
    this.urlBarOpen = false;
  }

  handleNavigate(url) {
    this.ensureIframe();
    this.iframe.load(url);
    this.urlBarOpen = false;
  }

  handleSelectWebView(windowId, webviewId) {
    // Use BroadcastChannel to tell the target window to select the webview
    const channel = new BroadcastChannel("servo-search");
    channel.postMessage({
      type: "selectWebView",
      id: Date.now(),
      targetWindowId: windowId,
      webviewId: webviewId,
    });
    channel.close();
    this.urlBarOpen = false;
  }

  render() {
    // Only render adopt-* attributes if they have values
    const adoptAttrs = {};
    if (this.attrs["adopt-webview-id"]) {
      adoptAttrs["adopt-webview-id"] = this.attrs["adopt-webview-id"];
    }
    if (this.attrs["adopt-browsing-context-id"]) {
      adoptAttrs["adopt-browsing-context-id"] =
        this.attrs["adopt-browsing-context-id"];
    }
    if (this.attrs["adopt-pipeline-id"]) {
      adoptAttrs["adopt-pipeline-id"] = this.attrs["adopt-pipeline-id"];
    }
    this.attrs = {};

    return html`
      <link rel="stylesheet" href="web_view.css" />
      <div
        class="wrapper ${this.active ? "active" : ""}"
        @click=${this.onfocus}
      >
        <div
          class="bar ${this.active ? "" : "hidden"}"
          style="background-color: ${this
            .themeColor}; color: contrast-color(${this.themeColor});"
        >
          <div class="load-progress load-${this.loadStatus}"></div>
          <img src="${this.favicon}" class="icon" />
          <lucide-icon
            name="arrow-left"
            class="icon enabled-${this.canGoBack}"
            color="contrast-color(${this.themeColor})"
            @click="${this.goBack}"
          ></lucide-icon>
          <lucide-icon
            name="arrow-right"
            class="icon enabled-${this.canGoForward}"
            color="contrast-color(${this.themeColor})"
            @click="${this.goForward}"
          ></lucide-icon>
          <lucide-icon
            name="rotate-ccw"
            class="icon"
            color="contrast-color(${this.themeColor})"
            @click="${this.doReload}"
          ></lucide-icon>
          <span class="title" @click=${this.openUrlBar}>${this.title}</span>
          <div class="menu-container">
            <lucide-icon
              @click=${this.toggleMenu}
              name="ellipsis-vertical"
              class="icon"
              color="contrast-color(${this.themeColor})"
            ></lucide-icon>
          </div>
          <lucide-icon
            @click=${this.close}
            name="x"
            class="icon"
            color="contrast-color(${this.themeColor})"
          ></lucide-icon>
        </div>
        <webview-menu
          ?open=${this.menuOpen}
          .x=${this.menuPosition?.x || 0}
          .y=${this.menuPosition?.y || 0}
          @menu-action=${this.handleMenuAction}
          @menu-closed=${this.handleMenuClosed}
        ></webview-menu>
        <context-menu
          ?open=${this.contextMenu !== null}
          .items=${this.contextMenu?.items || []}
          .x=${this.contextMenu?.x || 0}
          .y=${this.contextMenu?.y || 0}
          .controlId=${this.contextMenu?.controlId || ""}
          @menu-action=${this.handleContextMenuAction}
          @menu-cancel=${this.handleContextMenuCancel}
          @menu-closed=${this.handleContextMenuClosed}
        ></context-menu>
        ${this.renderUrlBarOverlay()}
        <div class="iframe-container">
          <iframe
            embed
            adopt-webview-id=${adoptAttrs["adopt-webview-id"]}
            adopt-browsing-context-id=${adoptAttrs["adopt-browsing-context-id"]}
            adopt-pipeline-id=${adoptAttrs["adopt-pipeline-id"]}
            src="${this.src}"
            @embedtitlechange=${this.ontitlechange}
            @embedfaviconchange=${this.onfaviconchange}
            @embedthemecolorchange=${this.onthemecolorchange}
            @embedurlchange=${this.onurlchange}
            @embedinputreceived=${this.onfocus}
            @embedcontrolshow=${this.oncontrolshow}
            @embedcontrolhide=${this.oncontrolhide}
            @embeddialogshow=${this.ondialogshow}
            @embednotificationshow=${this.onnotificationshow}
            @embedloadstatuschange=${this.onloadstatuschange}
          ></iframe>
          ${this.renderDialog()} ${this.renderPermissionPrompt()}
          <select-control
            ?open=${this.selectControl !== null}
            .options=${this.selectControl?.options || []}
            .selectedIndex=${this.selectControl?.selectedIndex ?? -1}
            .x=${this.selectControl?.x || 0}
            .y=${this.selectControl?.y || 0}
            .controlId=${this.selectControl?.controlId || ""}
            @select-option=${this.handleSelectOption}
            @select-cancel=${this.handleSelectCancel}
            @menu-closed=${this.handleSelectClosed}
          ></select-control>

          ${this.renderColorPicker()}
        </div>
      </div>
    `;
  }
}

customElements.define("web-view", WebView);
