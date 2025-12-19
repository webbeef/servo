// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

export { html, css };

export class MenuBase extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
  };

  constructor() {
    super();
    this.open = false;
  }

  handleItemClick(action) {
    this.dispatchEvent(
      new CustomEvent("menu-action", {
        bubbles: true,
        composed: true,
        detail: { action },
      }),
    );
    this.close();
  }

  close() {
    this.open = false;
    this.dispatchEvent(
      new CustomEvent("menu-closed", {
        bubbles: true,
        composed: true,
      }),
    );
  }
}
