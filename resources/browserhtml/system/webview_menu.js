// SPDX-License-Identifier: AGPL-3.0-or-later

import { MenuBase, css, html } from "./menu_base.js";

export class WebViewMenu extends MenuBase {
  static properties = {
    ...MenuBase.properties,
    x: { type: Number },
    y: { type: Number },
  };

  constructor() {
    super();
    this.x = 0;
    this.y = 0;
    this.handleKeyDown = this.handleKeyDown.bind(this);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.removeEventListeners();
  }

  updated(changedProperties) {
    if (changedProperties.has("open")) {
      if (this.open) {
        document.addEventListener("keydown", this.handleKeyDown);
      } else {
        this.removeEventListeners();
      }
    }
  }

  removeEventListeners() {
    document.removeEventListener("keydown", this.handleKeyDown);
  }

  handleKeyDown(e) {
    if (e.key === "Escape") {
      this.close();
    }
  }

  handleBackdropClick(e) {
    // Only close if the click was directly on the backdrop
    if (e.target.classList.contains("backdrop")) {
      this.close();
    }
  }

  static styles = css`
    @import url(//system.localhost:8888/webview_menu.css);
  `;

  render() {
    // Position the menu so its right edge aligns with x, top at y
    const menuStyle = `right: calc(100% - ${this.x}px); top: ${this.y}px;`;

    return html`
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="menu" style="${menuStyle}">
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("split-horizontal")}
        >
          <lucide-icon name="square-split-horizontal"></lucide-icon>
          <span>Horizontal Split</span>
        </div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("split-vertical")}
        >
          <lucide-icon name="square-split-vertical"></lucide-icon>
          <span>Vertical Split</span>
        </div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("reduce-size")}
        >
          <lucide-icon name="chevrons-right-left"></lucide-icon>
          <span>Reduce Width</span>
        </div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("increase-size")}
        >
          <lucide-icon name="chevrons-left-right"></lucide-icon>
          <span>Increase Width</span>
        </div>
        <div class="menu-separator"></div>
        <div class="menu-item" @click=${() => this.handleItemClick("zoom-in")}>
          <lucide-icon name="zoom-in"></lucide-icon>
          <span>Zoom In</span>
        </div>
        <div class="menu-item" @click=${() => this.handleItemClick("zoom-out")}>
          <lucide-icon name="zoom-out"></lucide-icon>
          <span>Zoom Out</span>
        </div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("zoom-reset")}
        >
          <lucide-icon name="refresh-cw"></lucide-icon>
          <span>Reset Zoom</span>
        </div>
      </div>
    `;
  }
}

customElements.define("webview-menu", WebViewMenu);
