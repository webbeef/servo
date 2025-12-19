// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

export class MobileOverview extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    tabs: { type: Array },
    activeTabId: { type: String },
  };

  static styles = css`
    @import url(//system.localhost:8888/mobile_overview.css);
  `;

  constructor() {
    super();
    this.open = false;
    this.tabs = [];
    this.activeTabId = null;

    // Touch state for swipe-to-close
    this.swipeState = null;
  }

  handleOverlayClick(e) {
    if (e.target.classList.contains("overlay")) {
      this.close();
    }
  }

  close() {
    this.open = false;
    this.dispatchEvent(new CustomEvent("overview-close", { bubbles: true }));
  }

  handleTabClick(tab) {
    this.dispatchEvent(
      new CustomEvent("tab-select", {
        bubbles: true,
        detail: { tabId: tab.id },
      })
    );
    this.close();
  }

  handleCloseTab(e, tab) {
    e.stopPropagation();

    // Animate the card closing
    const card = e.currentTarget.closest(".tab-card");
    card.classList.add("closing");

    setTimeout(() => {
      this.dispatchEvent(
        new CustomEvent("tab-close", {
          bubbles: true,
          detail: { tabId: tab.id },
        })
      );
    }, 300);
  }

  handleNewTab() {
    this.dispatchEvent(new CustomEvent("tab-new", { bubbles: true }));
    this.close();
  }

  handleHome() {
    this.dispatchEvent(new CustomEvent("tab-home", { bubbles: true }));
    this.close();
  }

  handleDone() {
    this.close();
  }

  // Touch handlers for swipe-up-to-close on cards
  handleTouchStart(e, tab) {
    const touch = e.touches[0];
    this.swipeState = {
      tab,
      startY: touch.clientY,
      currentY: touch.clientY,
      element: e.currentTarget,
    };
  }

  handleTouchMove(e) {
    if (!this.swipeState) {
      return;
    }

    const touch = e.touches[0];
    this.swipeState.currentY = touch.clientY;
    const deltaY = this.swipeState.currentY - this.swipeState.startY;

    // Only allow upward swipe (close)
    if (deltaY < 0) {
      this.swipeState.element.style.transform = `translateY(${deltaY}px)`;
      this.swipeState.element.style.opacity = Math.max(0, 1 + deltaY / 150);
    }
  }

  handleTouchEnd(e) {
    if (!this.swipeState) {
      return;
    }

    const deltaY = this.swipeState.currentY - this.swipeState.startY;
    const element = this.swipeState.element;
    const tab = this.swipeState.tab;

    if (deltaY < -80) {
      // Close threshold reached
      element.classList.add("closing");
      setTimeout(() => {
        this.dispatchEvent(
          new CustomEvent("tab-close", {
            bubbles: true,
            detail: { tabId: tab.id },
          })
        );
      }, 300);
    } else {
      // Snap back
      element.style.transform = "";
      element.style.opacity = "";
    }

    this.swipeState = null;
  }

  render() {
    let tabText = this.tabs.length > 1 ? `${this.tabs.length} Views` : `1 View`;

    return html`
      <div class="overlay" @click=${this.handleOverlayClick}></div>
      <div class="container">
        <div class="header">
          <span class="header-title">${tabText}</span>
          <div class="header-actions">
            <button class="header-button" @click=${this.handleHome}>
              <lucide-icon name="house"></lucide-icon>
            </button>
            <button class="header-button" @click=${this.handleDone}>
              <lucide-icon name="check"></lucide-icon>
            </button>
          </div>
        </div>

        <div class="grid">
          ${this.tabs.map(
            (tab) => html`
              <div
                class="tab-card ${tab.id === this.activeTabId ? "active" : ""}"
                @click=${() => this.handleTabClick(tab)}
                @touchstart=${(e) => this.handleTouchStart(e, tab)}
                @touchmove=${this.handleTouchMove}
                @touchend=${this.handleTouchEnd}
              >
                ${tab.screenshotUrl
                  ? html`<img
                      class="tab-screenshot"
                      src="${tab.screenshotUrl}"
                      alt=""
                    />`
                  : html`<div class="tab-screenshot-placeholder">
                      <lucide-icon name="globe"></lucide-icon>
                    </div>`}
                <div class="tab-info">
                  <img class="tab-favicon" src="${tab.favicon || ""}" alt="" />
                  <span class="tab-title">${tab.title || "Untitled"}</span>
                </div>
                <button
                  class="close-button"
                  @click=${(e) => this.handleCloseTab(e, tab)}
                >
                  <lucide-icon name="x"></lucide-icon>
                </button>
              </div>
            `
          )}

          <div class="home-card" @click=${this.handleHome}>
            <lucide-icon name="house"></lucide-icon>
            <span>Home</span>
          </div>
        </div>
      </div>
    `;
  }
}

customElements.define("mobile-overview", MobileOverview);
