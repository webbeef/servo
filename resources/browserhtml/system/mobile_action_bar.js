// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

import { SearchController } from "//shared.localhost:8888/search/controller.js";

export class MobileActionBar extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    url: { type: String },
    canGoBack: { type: Boolean },
    canGoForward: { type: Boolean },
    viewCount: { type: Number },
    results: { type: Array },
    groups: { type: Array },
  };

  static styles = css`
    @import url(//system.localhost:8888/mobile_action_bar.css);
  `;

  constructor() {
    super();
    this.open = false;
    this.url = "";
    this.canGoBack = false;
    this.canGoForward = false;
    this.viewCount = 1;
    this.layoutManager = null;
    this.results = [];
    this.groups = [];

    this.initSearchController();
  }

  initSearchController() {
    this.searchController = new SearchController({
      onNavigate: (url) => this.navigateTo(url),
      onSelectWebView: (windowId, webviewId) => this.selectWebView(windowId, webviewId),
      onResultsChanged: (results, groups) => {
        this.results = results;
        this.groups = groups;
      },
      debounceDelay: 150,
    });
  }

  setLayoutManager(lm) {
    this.layoutManager = lm;
  }

  show() {
    this.updateState();
    this.open = true;
  }

  hide() {
    this.open = false;
    this.results = [];
    this.groups = [];
    this.searchController.clear();
  }

  toggle() {
    if (this.open) {
      this.hide();
    } else {
      this.show();
    }
  }

  updateState() {
    if (this.layoutManager) {
      this.url = this.layoutManager.getCurrentUrl();
      const navState = this.layoutManager.getNavigationState();
      this.canGoBack = navState.canGoBack;
      this.canGoForward = navState.canGoForward;
      this.viewCount = this.layoutManager.getTabCount();
    }
  }

  handleOverlayClick(e) {
    if (e.target === e.currentTarget) {
      this.hide();
    }
  }

  handleInputKeydown(e) {
    if (e.key === "Enter") {
      this.handleNavigate();
    } else if (e.key === "Escape") {
      this.hide();
    }
  }

  handleInputChange(e) {
    const query = e.target.value.trim();
    if (query) {
      this.searchController.query(query);
    } else {
      this.results = [];
      this.groups = [];
      this.searchController.clear();
    }
  }

  handleNavigate() {
    const input = this.shadowRoot?.querySelector(".url-input");
    if (!input || !this.layoutManager) {
      return;
    }

    const value = input.value.trim();
    if (!value) {
      return;
    }

    this.searchController.handleSubmit(value);
  }

  navigateTo(url) {
    if (this.layoutManager) {
      this.layoutManager.navigateTo(url);
      const input = this.shadowRoot?.querySelector(".url-input");
      if (input) {
        input.blur();
      }
      this.hide();
    }
  }

  selectWebView(windowId, webviewId) {
    if (this.layoutManager) {
      this.layoutManager.selectWebView(windowId, webviewId);
      this.hide();
    }
  }

  handleResultClick(result) {
    this.searchController.handleResultClick(result);
  }

  handleHome() {
    this.dispatchEvent(
      new CustomEvent("action-home", { bubbles: true, composed: true })
    );
    this.hide();
  }

  handleBack() {
    if (this.layoutManager && this.canGoBack) {
      this.layoutManager.goBack();
    }
  }

  handleForward() {
    if (this.layoutManager && this.canGoForward) {
      this.layoutManager.goForward();
    }
  }

  handleReload() {
    if (this.layoutManager) {
      this.layoutManager.reload();
    }
    this.hide();
  }

  handleViews() {
    if (this.layoutManager) {
      this.layoutManager.toggleOverview();
    }
    this.hide();
  }

  handleMore() {
    // Dispatch event for more menu
    this.dispatchEvent(
      new CustomEvent("action-more", { bubbles: true, composed: true })
    );
  }

  renderResults() {
    if (this.groups.length === 0) {
      return html``;
    }

    return html`
      <div class="results-area">
        ${this.groups.map(
          (group) => html`
            <div class="result-group">
              <div class="result-group-icon">
                ${group.providerIcon
                  ? html`<lucide-icon name="${group.providerIcon}"></lucide-icon>`
                  : ""}
              </div>
              <div class="result-group-items">
                ${group.items.map((result) => this.renderResult(result))}
              </div>
            </div>
          `
        )}
      </div>
    `;
  }

  renderResult(result) {
    if (result.kind === "link" || result.kind === "webview") {
      return html`
        <div
          class="result-item"
          data-kind="${result.kind}"
          @click=${() => this.handleResultClick(result)}
        >
          <a href="#" class="result-link" @click=${(e) => e.preventDefault()}>
            ${result.value.title}
          </a>
        </div>
      `;
    } else if (result.kind === "text") {
      return html`
        <div class="result-item" data-kind="text">
          <span class="result-text">${result.value}</span>
        </div>
      `;
    }
    return html``;
  }

  render() {
    return html`
      <div class="overlay" @click=${this.handleOverlayClick}></div>
      <div class="action-bar">
        ${this.renderResults()}

        <div class="url-input-container">
          <lucide-icon name="search"></lucide-icon>
          <input
            type="text"
            class="url-input"
            placeholder="Search or enter URL…"
            .value=${this.url}
            @input=${this.handleInputChange}
            @keydown=${this.handleInputKeydown}
          />
        </div>

        <div class="quick-actions">
          <button class="action-button" @click=${this.handleHome}>
            <lucide-icon name="house"></lucide-icon>
          </button>

          <button
            class="action-button"
            @click=${this.handleBack}
            ?disabled=${!this.canGoBack}
          >
            <lucide-icon name="arrow-left"></lucide-icon>
          </button>

          <button
            class="action-button"
            @click=${this.handleForward}
            ?disabled=${!this.canGoForward}
          >
            <lucide-icon name="arrow-right"></lucide-icon>
          </button>

          <button class="action-button" @click=${this.handleReload}>
            <lucide-icon name="rotate-ccw"></lucide-icon>
          </button>

          <button class="action-button views-button" @click=${this.handleViews}>
            <lucide-icon name="layout-grid"></lucide-icon>
            ${this.viewCount > 1
              ? html`<span class="view-count">${this.viewCount}</span>`
              : ""}
          </button>
        </div>
      </div>
    `;
  }
}

customElements.define("mobile-action-bar", MobileActionBar);
