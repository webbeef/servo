// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

export class MobileRadialMenu extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    x: { type: Number },
    y: { type: Number },
    canGoBack: { type: Boolean },
    canGoForward: { type: Boolean },
    contextMenu: { type: Object },
    isHomescreen: { type: Boolean },
  };

  // 8 zones clockwise from top
  static zones = [
    { id: 0, action: "home", icon: "house", angle: -90 },
    { id: 1, action: "overview", icon: "layout-grid", angle: -45 },
    { id: 2, action: "forward", icon: "arrow-right", angle: 0 },
    { id: 3, action: "context-menu", icon: "ellipsis-vertical", angle: 45 },
    { id: 4, action: "close-view", icon: "x", angle: 90 },
    { id: 5, action: "settings", icon: "settings", angle: 135 },
    { id: 6, action: "back", icon: "arrow-left", angle: 180 },
    { id: 7, action: "reload", icon: "rotate-ccw", angle: -135 },
  ];

  static styles = css`
    @import url(//system.localhost:8888/mobile_radial_menu.css);
  `;

  constructor() {
    super();
    this.open = false;
    this.x = 0;
    this.y = 0;
    this.canGoBack = false;
    this.canGoForward = false;
    this.contextMenu = null;
    this.isHomescreen = false;
  }

  show(x, y, contextMenu = null) {
    // Adjust position to keep menu on screen
    const menuRadius = 100;
    const padding = 20;

    this.x = Math.max(
      menuRadius + padding,
      Math.min(window.innerWidth - menuRadius - padding, x)
    );
    this.y = Math.max(
      menuRadius + padding,
      Math.min(window.innerHeight - menuRadius - padding, y)
    );
    this.contextMenu = contextMenu;
    this.open = true;
  }

  hide() {
    this.open = false;
    // Dispatch dismiss event so pending context menu can be closed
    this.dispatchEvent(
      new CustomEvent("radial-dismiss", {
        bubbles: true,
        composed: true,
      })
    );
    this.contextMenu = null;
  }

  handleOverlayTap(e) {
    // Tapping on overlay (outside menu) closes the menu
    e.preventDefault();
    this.hide();
  }

  handleZoneTap(zone, e) {
    console.log(`[RadialMenu] handleZoneTap ${zone.action}`);
    e.preventDefault();
    e.stopPropagation();

    // Check if action is disabled
    if (this.isZoneDisabled(zone)) {
      return;
    }

    // For context-menu action, close without dispatching dismiss
    // (the context menu will be shown instead)
    if (zone.action === "context-menu") {
      this.dispatchEvent(
        new CustomEvent("radial-action", {
          bubbles: true,
          composed: true,
          detail: {
            action: zone.action,
            contextMenu: this.contextMenu,
          },
        })
      );
      this.open = false;
      this.contextMenu = null;
      return;
    }

    // Dispatch action
    this.dispatchEvent(
      new CustomEvent("radial-action", {
        bubbles: true,
        composed: true,
        detail: {
          action: zone.action,
          originX: this.x,
          originY: this.y,
        },
      })
    );

    this.hide();
  }

  isZoneDisabled(zone) {
    if (zone.action === "back") {
      return !this.canGoBack;
    }
    if (zone.action === "forward") {
      return !this.canGoForward;
    }
    if (zone.action === "context-menu") {
      return !this.contextMenu || this.contextMenu.items.length === 0;
    }
    // Disable close-view, reload and home actions when on homescreen
    if (
      zone.action === "close-view" ||
      zone.action === "home" ||
      zone.action === "reload"
    ) {
      return this.isHomescreen;
    }
    return false;
  }

  render() {
    const menuStyle = `left: ${this.x}px; top: ${this.y}px;`;

    return html`
      <div class="overlay" @click=${this.handleOverlayTap}></div>
      <div class="menu" style=${menuStyle}>
        <div class="center">
          <div class="center-dot"></div>
        </div>
        ${MobileRadialMenu.zones.map(
          (zone) => html`
            <div
              class="zone ${this.isZoneDisabled(zone) ? "disabled" : ""}"
              data-zone=${zone.id}
              @click=${(e) => this.handleZoneTap(zone, e)}
            >
              <lucide-icon name=${zone.icon}></lucide-icon>
            </div>
          `
        )}
      </div>
    `;
  }
}

customElements.define("mobile-radial-menu", MobileRadialMenu);
