// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

export class PanelOverview extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    panels: { type: Array },
    rootWidth: { type: Number },
    rootHeight: { type: Number },
  };

  constructor() {
    super();
    this.open = false;
    this.panels = [];
    this.rootWidth = 0;
    this.rootHeight = 0;
    this.boundKeyHandler = this.handleKeyDown.bind(this);
  }

  connectedCallback() {
    super.connectedCallback();
    document.addEventListener("keydown", this.boundKeyHandler);
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    document.removeEventListener("keydown", this.boundKeyHandler);
  }

  handleKeyDown(e) {
    if (this.open && e.key === "Escape") {
      this.close();
    }
  }

  handleContainerClick(e) {
    // Close if clicking on the background container (not a thumbnail)
    if (e.target === e.currentTarget) {
      this.close();
    }
  }

  handleThumbnailClick(e, thumb) {
    e.stopPropagation();
    this.dispatchEvent(
      new CustomEvent("overview-select", {
        bubbles: true,
        composed: true,
        detail: {
          webviewId: thumb.webviewId,
          panelIndex: thumb.panelIndex,
        },
      }),
    );
    this.close();
  }

  close() {
    this.open = false;
    this.dispatchEvent(
      new CustomEvent("overview-close", {
        bubbles: true,
        composed: true,
      }),
    );
  }

  // Calculate scale factor to fit all panels
  calculateScale() {
    if (!this.panels.length || !this.rootWidth || !this.rootHeight) {
      return 1;
    }

    const gap = 16;
    const availableWidth = this.rootWidth * 0.9;
    const availableHeight = this.rootHeight * 0.8;

    let totalUnscaledWidth = 0;
    let maxUnscaledHeight = 0;
    for (const panel of this.panels) {
      totalUnscaledWidth += panel.width;
      maxUnscaledHeight = Math.max(maxUnscaledHeight, panel.height);
    }

    const numPanels = this.panels.length;
    const scaleForWidth =
      (availableWidth - (numPanels - 1) * gap) / totalUnscaledWidth;
    const scaleForHeight = availableHeight / maxUnscaledHeight;
    return Math.min(scaleForWidth, scaleForHeight);
  }

  renderThumbnail(thumb, scale) {
    const style = `
      left: ${thumb.x * scale}px;
      top: ${thumb.y * scale}px;
      width: ${thumb.width * scale}px;
      height: ${thumb.height * scale}px;
    `;

    const bgColor = thumb.themeColor || "var(--bg-header)";
    const labelStyle = `
      background-color: ${bgColor};
      color: contrast-color(${bgColor});
    `;

    return html`
      <div
        class="thumbnail ${thumb.active ? "active" : ""}"
        style=${style}
        @click=${(e) => this.handleThumbnailClick(e, thumb)}
      >
        ${thumb.screenshotUrl
          ? html`<img
              src=${thumb.screenshotUrl}
              alt=${thumb.title || "Webview"}
            />`
          : ""}
        <div class="label" style=${labelStyle}>
          ${thumb.title || "Untitled"}
        </div>
      </div>
    `;
  }

  renderPanel(panel, scale) {
    const wrapperStyle = `
      width: ${panel.width * scale}px;
      height: ${panel.height * scale}px;
    `;
    const panelStyle = `
      width: ${panel.width * scale}px;
      height: ${panel.height * scale}px;
    `;

    return html`
      <div class="panel-wrapper" style=${wrapperStyle}>
        <div class="panel" style=${panelStyle}>
          ${panel.thumbnails.map((thumb) => this.renderThumbnail(thumb, scale))}
        </div>
      </div>
    `;
  }

  render() {
    const scale = this.calculateScale();

    return html`
      <link rel="stylesheet" href="overview.css" />
      <div class="container" @click=${this.handleContainerClick}>
        <div class="panels">
          ${this.panels.map((panel) => this.renderPanel(panel, scale))}
        </div>
      </div>
    `;
  }
}

customElements.define("panel-overview", PanelOverview);
