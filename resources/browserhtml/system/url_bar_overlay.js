// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";
import { SearchController } from "//shared.localhost:8888/search/controller.js";

export class UrlBarOverlay extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    url: { type: String },
    groups: { state: true },
    onNavigate: { type: Function },
    onSelectWebView: { type: Function },
  };

  static styles = css`
    @import url(//system.localhost:8888/url_bar_overlay.css);
  `;

  constructor() {
    super();
    this.open = false;
    this.url = "";
    this.groups = [];
    this.onNavigate = null;
    this.onSelectWebView = null;

    this.controller = new SearchController({
      onNavigate: (url) => this.navigateTo(url),
      onSelectWebView: (windowId, webviewId) =>
        this.selectWebView(windowId, webviewId),
      onResultsChanged: (results, groups) => {
        this.groups = groups;
      },
    });
  }

  updated(changedProperties) {
    if (changedProperties.has("open") && this.open) {
      // Focus and select the input when opening
      this.updateComplete.then(() => {
        const input = this.shadowRoot.querySelector(".search-input");
        if (input) {
          input.focus();
          input.select();
        }
      });
      // Initial query with current URL
      this.controller.queryImmediate(this.url);
    }
  }

  handleBackdropClick() {
    this.close();
  }

  handleKeydown(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      this.close();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const input = this.shadowRoot.querySelector(".search-input");
      this.controller.handleSubmit(input?.value || "");
    }
  }

  handleInput(e) {
    this.controller.query(e.target.value);
  }

  handleResultClick(e, result) {
    e.preventDefault();
    this.controller.handleResultClick(result);
  }

  navigateTo(url) {
    if (this.onNavigate) {
      this.onNavigate(url);
    }
    this.close();
  }

  selectWebView(windowId, webviewId) {
    if (this.onSelectWebView) {
      this.onSelectWebView(windowId, webviewId);
    }
    this.close();
  }

  close() {
    this.open = false;
    this.groups = [];
    this.controller.clear();
    this.dispatchEvent(new CustomEvent("close"));
  }

  render() {
    return html`
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="overlay" @keydown=${this.handleKeydown}>
        <div class="search-header">
          <img src="//system.localhost:8888/logo.png" alt="Logo" class="logo" />
          <input
            type="text"
            class="search-input"
            .value=${this.url}
            placeholder="Search or enter URL…"
            @input=${this.handleInput}
          />
        </div>
        <div class="results-container">
          <div class="results-list">
            ${this.groups.map(
              (group) => html`
                <div class="result-group">
                  <div class="result-group-icon">
                    ${group.providerIcon
                      ? html`<lucide-icon
                          name="${group.providerIcon}"
                        ></lucide-icon>`
                      : null}
                  </div>
                  <div class="result-group-items">
                    ${group.items.map(
                      (result) => html`
                        <div
                          class="result-item"
                          data-kind=${result.kind}
                          @click=${(e) => this.handleResultClick(e, result)}
                        >
                          ${result.kind === "link" || result.kind === "webview"
                            ? html`<span class="result-link"
                                >${result.value.title}</span
                              >`
                            : html`<span class="result-text"
                                >${result.value}</span
                              >`}
                        </div>
                      `
                    )}
                  </div>
                </div>
              `
            )}
          </div>
        </div>
      </div>
    `;
  }
}

customElements.define("url-bar-overlay", UrlBarOverlay);
