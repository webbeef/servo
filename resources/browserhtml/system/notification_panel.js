// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

export class NotificationPanel extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    notifications: { type: Array, state: true },
  };

  static styles = css`
    @import url("//system.localhost:8888/notification_panel.css");
  `;

  constructor() {
    super();
    this.open = false;
    this.notifications = [];
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
      this.close();
    }
  }

  close() {
    this.open = false;
    this.dispatchEvent(
      new CustomEvent("panel-closed", {
        bubbles: true,
        composed: true,
      })
    );
  }

  handleBackdropClick(e) {
    if (e.target.classList.contains("backdrop")) {
      this.close();
    }
  }

  handleNotificationClick(notification, e) {
    // Don't handle click if dismiss button was clicked
    if (e.target.closest(".notification-dismiss")) {
      return;
    }

    this.dispatchEvent(
      new CustomEvent("notification-click", {
        bubbles: true,
        composed: true,
        detail: { notification },
      })
    );
  }

  handleDismiss(notification, e) {
    e.stopPropagation();
    this.dispatchEvent(
      new CustomEvent("notification-dismiss", {
        bubbles: true,
        composed: true,
        detail: { notification },
      })
    );
  }

  handleClearAll() {
    this.dispatchEvent(
      new CustomEvent("notification-clear-all", {
        bubbles: true,
        composed: true,
      })
    );
  }

  formatTimeAgo(timestamp) {
    if (!timestamp) {
      return "";
    }

    const now = Date.now();
    const diff = now - timestamp;

    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (days > 0) {
      return `${days}d ago`;
    }
    if (hours > 0) {
      return `${hours}h ago`;
    }
    if (minutes > 0) {
      return `${minutes}m ago`;
    }
    return "Just now";
  }

  renderNotificationIcon(notification) {
    if (notification.iconUrl) {
      return html`<img src="${notification.iconUrl}" alt="" />`;
    }
    return html`<lucide-icon name="bell"></lucide-icon>`;
  }

  renderNotification(notification) {
    return html`
      <div
        class="notification-item"
        @click=${(e) => this.handleNotificationClick(notification, e)}
      >
        <div class="notification-header">
          <div class="notification-icon">
            ${this.renderNotificationIcon(notification)}
          </div>
          <div class="notification-content">
            <div class="notification-title">${notification.title}</div>
            <div class="notification-body">${notification.body}</div>
            <div class="notification-meta">
              <span class="notification-time"
                >${this.formatTimeAgo(notification.timestamp)}</span
              >
            </div>
          </div>
        </div>
        <button
          class="notification-dismiss"
          @click=${(e) => this.handleDismiss(notification, e)}
          title="Dismiss"
        >
          <lucide-icon name="x" size="14"></lucide-icon>
        </button>
      </div>
    `;
  }

  renderEmptyState() {
    return html`
      <div class="empty-state">
        <lucide-icon name="bell-off"></lucide-icon>
        <div class="empty-state-text">No notifications</div>
      </div>
    `;
  }

  render() {
    return html`
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="panel">
        <div class="header">
          <span class="header-title">Notifications</span>
          <div class="header-actions">
            ${this.notifications.length > 0
              ? html`<button class="clear-btn" @click=${this.handleClearAll}>
                  Clear all
                </button>`
              : ""}
            <button class="close-btn" @click=${() => this.close()}>
              <lucide-icon name="x" size="16"></lucide-icon>
            </button>
          </div>
        </div>
        <div class="notification-list">
          ${this.notifications.length > 0
            ? this.notifications.map((n) => this.renderNotification(n))
            : this.renderEmptyState()}
        </div>
      </div>
    `;
  }
}

customElements.define("notification-panel", NotificationPanel);
