// SPDX-License-Identifier: AGPL-3.0-or-later

import { MenuBase, css, html } from "./menu_base.js";

export class SelectControl extends MenuBase {
  static properties = {
    ...MenuBase.properties,
    options: { type: Array },
    selectedIndex: { type: Number },
    x: { type: Number },
    y: { type: Number },
    controlId: { type: String },
  };

  constructor() {
    super();
    this.options = [];
    this.selectedIndex = -1;
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
        requestAnimationFrame(() => {
          document.addEventListener("keydown", this.handleKeyDown);
        });
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
    if (e.target.classList.contains("backdrop")) {
      this.cancel();
    }
  }

  cancel() {
    this.dispatchEvent(
      new CustomEvent("select-cancel", {
        bubbles: true,
        composed: true,
        detail: { controlId: this.controlId },
      }),
    );
    this.close();
  }

  handleOptionClick(option, index) {
    if (option.disabled) {
      return;
    }
    this.dispatchEvent(
      new CustomEvent("select-option", {
        bubbles: true,
        composed: true,
        detail: {
          controlId: this.controlId,
          optionId: option.id,
          index: index,
        },
      }),
    );
    this.close();
  }

  static styles = css`
    @import url(//system.localhost:8888/select_control.css);
  `;

  render() {
    // Group options by their group label
    let currentGroup = null;
    const renderedItems = [];

    this.options.forEach((option, index) => {
      // Check if we need to render a group header
      if (option.group && option.group !== currentGroup) {
        currentGroup = option.group;
        renderedItems.push(html`
          <div class="option-group">${option.group}</div>
        `);
      } else if (!option.group && currentGroup !== null) {
        currentGroup = null;
      }

      const isSelected = index === this.selectedIndex;
      renderedItems.push(html`
        <div
          class="menu-item ${option.disabled ? "disabled" : ""} ${isSelected
            ? "selected"
            : ""}"
          @click=${() => this.handleOptionClick(option, index)}
        >
          <span class="icon-slot">
            ${isSelected ? html`<lucide-icon name="check"></lucide-icon>` : ""}
          </span>
          <span>${option.label}</span>
        </div>
      `);
    });

    return html`
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="menu" style="left: ${this.x}px; top: ${this.y}px;">
        ${renderedItems}
      </div>
    `;
  }
}

customElements.define("select-control", SelectControl);
