// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

export class MobileNotificationSheet extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    notifications: { type: Array },
    tabCount: { type: Number },
  };

  static styles = css`
    @import url(//system.localhost:8888/mobile_notification_sheet.css);
  `;

  constructor() {
    super();
    this.open = false;
    this.notifications = [];
    this.tabCount = 1;

    // Swipe state for individual notifications
    this.swipeState = null;
  }

  formatTime(timestamp) {
    if (!timestamp) {
      return "";
    }

    const now = Date.now();
    const diff = now - timestamp;
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);

    if (minutes < 1) {
      return "Just now";
    }
    if (minutes < 60) {
      return `${minutes}m ago`;
    }
    if (hours < 24) {
      return `${hours}h ago`;
    }
    return new Date(timestamp).toLocaleDateString();
  }

  handleOverlayClick(e) {
    if (e.target === e.currentTarget) {
      this.close();
    }
  }

  close() {
    this.open = false;
    this.dispatchEvent(new CustomEvent("sheet-closed", { bubbles: true }));
  }

  handleNotificationClick(notification) {
    this.dispatchEvent(
      new CustomEvent("notification-click", {
        bubbles: true,
        detail: { notification },
      })
    );
  }

  handleDismiss(e, notification) {
    e.stopPropagation();
    this.dispatchEvent(
      new CustomEvent("notification-dismiss", {
        bubbles: true,
        detail: { notification },
      })
    );
  }

  handleClearAll() {
    this.dispatchEvent(
      new CustomEvent("notification-clear-all", { bubbles: true })
    );
  }

  // Touch handlers for swipe-to-dismiss
  handleTouchStart(e, notification) {
    const touch = e.touches[0];
    this.swipeState = {
      notification,
      startX: touch.clientX,
      currentX: touch.clientX,
      element: e.currentTarget,
    };
    e.currentTarget.classList.add("swiping");
  }

  handleTouchMove(e) {
    if (!this.swipeState) {
      return;
    }

    const touch = e.touches[0];
    this.swipeState.currentX = touch.clientX;
    const deltaX = this.swipeState.currentX - this.swipeState.startX;

    // Only allow left swipe (dismiss)
    if (deltaX < 0) {
      this.swipeState.element.style.transform = `translateX(${deltaX}px)`;
      this.swipeState.element.style.opacity = Math.max(0, 1 + deltaX / 200);
    }
  }

  handleTouchEnd(e) {
    if (!this.swipeState) {
      return;
    }

    const deltaX = this.swipeState.currentX - this.swipeState.startX;
    const element = this.swipeState.element;
    const notification = this.swipeState.notification;

    element.classList.remove("swiping");

    if (deltaX < -100) {
      // Dismiss threshold reached
      element.style.transform = "translateX(-100%)";
      element.style.opacity = "0";
      setTimeout(() => {
        this.handleDismiss(new Event("click"), notification);
      }, 200);
    } else {
      // Snap back
      element.style.transform = "";
      element.style.opacity = "";
    }

    this.swipeState = null;
  }

  render() {
    return html`
      <div class="overlay" @click=${this.handleOverlayClick}></div>
      <div class="sheet">
        <div class="sheet-header">
          <div class="status-info">
            <lucide-icon name="layout-grid"></lucide-icon>
            <span class="tab-count">${this.tabCount} tabs</span>
          </div>
          <button
            class="clear-all-button"
            @click=${this.handleClearAll}
            ?disabled=${this.notifications.length === 0}
          >
            Clear All
          </button>
        </div>

        <div class="notifications-list">
          ${this.notifications.length === 0
            ? html`
                <div class="empty-state">
                  <lucide-icon name="bell-off"></lucide-icon>
                  <span class="empty-state-text">No notifications</span>
                </div>
              `
            : this.notifications.map(
                (notification) => html`
                  <div
                    class="notification-item"
                    @click=${() => this.handleNotificationClick(notification)}
                    @touchstart=${(e) => this.handleTouchStart(e, notification)}
                    @touchmove=${this.handleTouchMove}
                    @touchend=${this.handleTouchEnd}
                  >
                    <div class="notification-icon">
                      ${notification.iconUrl
                        ? html`<img src="${notification.iconUrl}" alt="" />`
                        : html`<lucide-icon name="bell"></lucide-icon>`}
                    </div>
                    <div class="notification-content">
                      <div class="notification-title">
                        ${notification.title || "Notification"}
                      </div>
                      ${notification.body
                        ? html`<div class="notification-body">
                            ${notification.body}
                          </div>`
                        : ""}
                      <div class="notification-time">
                        ${this.formatTime(notification.timestamp)}
                      </div>
                    </div>
                    <button
                      class="dismiss-button"
                      @click=${(e) => this.handleDismiss(e, notification)}
                    >
                      <lucide-icon name="x"></lucide-icon>
                    </button>
                  </div>
                `
              )}
        </div>
      </div>
    `;
  }
}

customElements.define("mobile-notification-sheet", MobileNotificationSheet);
