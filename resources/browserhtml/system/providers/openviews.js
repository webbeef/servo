// SPDX-License-Identifier: AGPL-3.0-or-later

const DISCOVERY_TIMEOUT = 50; // ms to wait for responses
const MAX_RESULTS = 10;

export class OpenViewsProvider {
  constructor() {
    this.name = "Open Tabs";
    this.icon = "app-window";
    this.channel = new BroadcastChannel("servo-search");
    this.pendingQuery = null;
    this.webviews = [];

    // Listen for responses from browser windows
    this.channel.onmessage = (e) => {
      if (e.data.type === "webviewList" && this.pendingQuery) {
        // Collect web-views from responding windows
        for (const wv of e.data.webviews) {
          this.webviews.push({
            ...wv,
            windowId: e.data.windowId,
          });
        }
      }
    };
  }

  async query(text) {
    if (!text || text.trim() === "") {
      return [];
    }

    const query = text.toLowerCase().trim();

    // Reset state for new query
    this.webviews = [];
    this.pendingQuery = query;

    // Request web-view list from all browser windows
    this.channel.postMessage({ type: "listWebViews" });

    // Wait for responses
    await new Promise((resolve) => setTimeout(resolve, DISCOVERY_TIMEOUT));

    this.pendingQuery = null;

    // Filter and score results
    const results = [];

    for (const wv of this.webviews) {
      const title = (wv.title || "").toLowerCase();
      const url = (wv.url || "").toLowerCase();

      // Check for matches in title or URL
      const titleMatch = title.includes(query);
      const urlMatch = url.includes(query);

      if (titleMatch || urlMatch) {
        // Score: title match is worth more than URL match
        const score = (titleMatch ? 0.6 : 0) + (urlMatch ? 0.3 : 0);

        results.push({
          score: score,
          kind: "webview",
          value: {
            title: wv.title || wv.url || "Untitled",
            url: wv.url,
            webviewId: wv.webviewId,
            windowId: wv.windowId,
          },
        });
      }
    }

    // Sort by score and limit
    results.sort((a, b) => b.score - a.score);
    return results.slice(0, MAX_RESULTS);
  }
}
