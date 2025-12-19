// SPDX-License-Identifier: AGPL-3.0-or-later

import { MenuBase, css, html } from "./menu_base.js";

export class ContextMenu extends MenuBase {
  static properties = {
    ...MenuBase.properties,
    items: { type: Array },
    x: { type: Number },
    y: { type: Number },
    controlId: { type: String },
  };

  constructor() {
    super();
    this.items = [];
    this.x = 0;
    this.y = 0;
    this.controlId = "";
    this.handleKeyDown = this.handleKeyDown.bind(this);
  }

  connectedCallback() {
    super.connectedCallback();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.removeEventListeners();
  }

  updated(changedProperties) {
    if (changedProperties.has("open")) {
      if (this.open) {
        // Add keyboard listener when menu opens
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
      this.cancel();
    }
  }

  handleBackdropClick(e) {
    // Only cancel if the click was directly on the backdrop, not on the menu
    if (e.target.classList.contains("backdrop")) {
      this.cancel();
    }
  }

  cancel() {
    this.dispatchEvent(
      new CustomEvent("menu-cancel", {
        bubbles: true,
        composed: true,
        detail: { controlId: this.controlId },
      }),
    );
    this.close();
  }

  handleItemClick(item) {
    if (item.disabled) {
      return;
    }
    this.dispatchEvent(
      new CustomEvent("menu-action", {
        bubbles: true,
        composed: true,
        detail: { action: item.id, controlId: this.controlId },
      }),
    );
    this.close();
  }

  static styles = css`
    @import url(//system.localhost:8888/context_menu.css);
  `;

  render() {
    return html`
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="menu" style="left: ${this.x}px; top: ${this.y}px;">
        ${this.items.map(
          (item) => html`
            <div
              class="menu-item ${item.disabled ? "disabled" : ""}"
              @click=${() => this.handleItemClick(item)}
            >
              <span class="icon-slot">
                ${item.icon
                  ? html`<lucide-icon name="${item.icon}"></lucide-icon>`
                  : ""}
              </span>
              <span>${item.label}</span>
              ${item.checked
                ? html`<lucide-icon name="check"></lucide-icon>`
                : ""}
            </div>
          `,
        )}
      </div>
    `;
  }
}

customElements.define("context-menu", ContextMenu);
