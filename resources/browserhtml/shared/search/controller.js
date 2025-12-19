// SPDX-License-Identifier: AGPL-3.0-or-later

import { ResultsAggregator } from "//system.localhost:8888/results_aggregator.js";
import { TopSitesProvider } from "//system.localhost:8888/providers/topsites.js";
import { FendProvider } from "//system.localhost:8888/providers/fend.js";
import { OpenViewsProvider } from "//system.localhost:8888/providers/openviews.js";
import SearchEngines from "//shared.localhost:8888/search/engines.js";
import { isUrl, normalizeUrl, debounce, groupResults } from "./utils.js";

/**
 * SearchController - Handles search logic with customizable callbacks
 *
 * Uses composition pattern: consumers provide callbacks for navigation
 * and result display, while the controller handles the search logic.
 */
export class SearchController {
  /**
   * Create a new SearchController
   * @param {Object} options - Configuration options
   * @param {Function} options.onNavigate - Callback when navigating to a URL: (url) => void
   * @param {Function} options.onSelectWebView - Callback when selecting a webview: (windowId, webviewId) => void
   * @param {Function} options.onResultsChanged - Callback when results change: (results, groups) => void
   * @param {number} options.debounceDelay - Debounce delay in ms (default: 150)
   */
  constructor({
    onNavigate = null,
    onSelectWebView = null,
    onResultsChanged = null,
    debounceDelay = 150,
  } = {}) {
    this.onNavigate = onNavigate;
    this.onSelectWebView = onSelectWebView;
    this.onResultsChanged = onResultsChanged;

    // Initialize providers
    this.aggregator = new ResultsAggregator([
      new OpenViewsProvider(),
      new FendProvider(),
      new TopSitesProvider(),
    ]);

    // Initialize search engines
    this.searchEngines = new SearchEngines();
    this.searchEngines.ensureReady();

    // Current results
    this.results = [];
    this.groups = [];

    // Create debounced query function
    this.debouncedQuery = debounce(
      (query) => this.executeQuery(query),
      debounceDelay,
    );
  }

  /**
   * Query with debouncing - use this for input events
   * @param {string} query - The search query
   */
  query(query) {
    this.debouncedQuery(query.trim());
  }

  /**
   * Query immediately without debouncing
   * @param {string} query - The search query
   */
  async queryImmediate(query) {
    await this.executeQuery(query.trim());
  }

  /**
   * Execute the query and update results
   * @private
   */
  async executeQuery(query) {
    this.results = await this.aggregator.query(query);
    this.groups = groupResults(this.results);

    if (this.onResultsChanged) {
      this.onResultsChanged(this.results, this.groups);
    }
  }

  /**
   * Handle form submission - navigate to URL or first result
   * @param {string} query - The current query text
   * @param {Function} getFirstLinkUrl - Optional function to get first link URL from DOM
   */
  handleSubmit(query, getFirstLinkUrl = null) {
    const trimmed = query.trim();
    if (!trimmed) {
      return;
    }

    let url;

    // Check if it looks like a URL
    if (isUrl(trimmed)) {
      url = normalizeUrl(trimmed);
    } else {
      // Try to get first link result
      let firstLinkUrl = null;

      // Check internal results first
      const firstLink = this.results.find((r) => r.kind === "link");
      if (firstLink) {
        firstLinkUrl = firstLink.value.url;
      }

      // Allow consumer to override (e.g., from DOM)
      if (!firstLinkUrl && getFirstLinkUrl) {
        firstLinkUrl = getFirstLinkUrl();
      }

      if (firstLinkUrl) {
        url = firstLinkUrl;
      } else {
        // Default to search
        url = this.searchEngines.queryUrl(trimmed);
      }
    }

    this.navigate(url);
  }

  /**
   * Handle clicking on a result
   * @param {Object} result - The result object
   */
  handleResultClick(result) {
    if (result.kind === "link") {
      this.navigate(result.value.url);
    } else if (result.kind === "webview") {
      this.selectWebView(result.value.windowId, result.value.webviewId);
    }
  }

  /**
   * Navigate to a URL
   * @private
   */
  navigate(url) {
    if (this.onNavigate) {
      this.onNavigate(url);
    }
  }

  /**
   * Select a webview
   * @private
   */
  selectWebView(windowId, webviewId) {
    if (this.onSelectWebView) {
      this.onSelectWebView(windowId, webviewId);
    }
  }

  /**
   * Clear current results
   */
  clear() {
    this.results = [];
    this.groups = [];
    if (this.onResultsChanged) {
      this.onResultsChanged(this.results, this.groups);
    }
  }
}

// Re-export utilities for convenience
export { isUrl, normalizeUrl, groupResults } from "./utils.js";
