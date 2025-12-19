// SPDX-License-Identifier: AGPL-3.0-or-later

import { MenuBase, html, css } from "./menu_base.js";

export class SystemMenu extends MenuBase {
  constructor() {
    super();
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
    if (e.target.classList.contains("backdrop")) {
      this.close();
    }
  }

  static styles = css`
    @import url(//system.localhost:8888/system_menu.css);
  `;

  render() {
    return html`
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="menu">
        <div class="menu-item" @click=${() => this.handleItemClick("new-tab")}>
          <lucide-icon name="plus"></lucide-icon>
          <span>New View</span>
        </div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("new-search")}
        >
          <lucide-icon name="search"></lucide-icon>
          <span>Floating Search</span>
        </div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("new-window")}
        >
          <lucide-icon name="app-window"></lucide-icon>
          <span>New Window</span>
        </div>
        <div class="menu-separator"></div>
        <div class="menu-item" @click=${() => this.handleItemClick("overview")}>
          <lucide-icon name="layout-grid"></lucide-icon>
          <span>Overview</span>
        </div>
        <div class="menu-item" @click=${() => this.handleItemClick("settings")}>
          <lucide-icon name="settings"></lucide-icon>
          <span>Settings</span>
        </div>
        <div class="menu-separator"></div>
        <div
          class="menu-item"
          @click=${() => this.handleItemClick("reload-ui")}
        >
          <lucide-icon name="refresh-cw"></lucide-icon>
          <span>Reload UI</span>
        </div>
        <div class="menu-item" @click=${() => this.handleItemClick("quit")}>
          <lucide-icon name="power"></lucide-icon>
          <span>Quit</span>
        </div>
      </div>
    `;
  }
}

customElements.define("system-menu", SystemMenu);
