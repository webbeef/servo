// SPDX-License-Identifier: AGPL-3.0-or-later

import { html, css, LitElement } from "./third_party/lit/lit-all.min.js";

class LucideIcon extends LitElement {
  constructor() {
    super();
  }

  static get properties() {
    return {
      name: { type: String },
    };
  }

  static get styles() {
    return css`
      :host(lucide-icon) div {
        width: 1em;
        margin-left: 0.125em;
        margin-right: 0.125em;
        display: flex;
        align-items: center;
        justify-content: center;
      }
    `;
  }

  render() {
    return html`<div>
      <link
        rel="stylesheet"
        href="//shared.localhost:8888/third_party/lucide/lucide.css"
      />
      <i class="icon-${this.name}"></i>
    </div>`;
  }
}

customElements.define("lucide-icon", LucideIcon);
