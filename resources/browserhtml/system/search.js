// SPDX-License-Identifier: AGPL-3.0-or-later

import { SearchController } from "//shared.localhost:8888/search/controller.js";

const container = document.getElementById("container");
const searchInput = document.getElementById("search-input");
const resultsList = document.getElementById("results-list");

// BroadcastChannel for communicating with main browser window
const searchChannel = new BroadcastChannel("servo-search");
let discoveredWindows = [];
let pendingNavigation = null;

// Listen for responses from browser windows
searchChannel.onmessage = (e) => {
  if (e.data.type === "available") {
    // Collect discovered browser windows
    discoveredWindows.push(e.data.windowId);
  } else if (
    e.data.type === "ack" &&
    pendingNavigation &&
    e.data.id === pendingNavigation.id
  ) {
    // Browser window handled it, close search window
    clearTimeout(pendingNavigation.timeout);
    pendingNavigation = null;
    // navigator.embedder.closeCurrentOSWindow();
  }
};

// Navigate to a URL - open in existing browser window or create new one
function navigateTo(url) {
  discoveredWindows = [];

  // Phase 1: Discover available browser windows
  searchChannel.postMessage({ type: "discover" });

  // Phase 2: After brief discovery period, send to first responder or open new window
  setTimeout(() => {
    if (discoveredWindows.length > 0) {
      // Send to first discovered window
      const targetWindowId = discoveredWindows[0];
      const id = Date.now();

      searchChannel.postMessage({
        type: "openUrl",
        url: url,
        id: id,
        targetWindowId: targetWindowId,
      });

      // Set up fallback timeout
      pendingNavigation = {
        id: id,
        timeout: setTimeout(() => {
          // Target window didn't ack - open new window as fallback
          pendingNavigation = null;
          navigator.embedder.openNewOSWindow(
            `//system.localhost:8888/index.html?open=${encodeURIComponent(url)}`,
          );
          // navigator.embedder.closeCurrentOSWindow();
        }, 100),
      };
    } else {
      // No browser windows found - open new window
      navigator.embedder.openNewOSWindow(
        `//system.localhost:8888/index.html?open=${encodeURIComponent(url)}`,
      );
      // navigator.embedder.closeCurrentOSWindow();
    }
  }, 50); // 50ms discovery window
}

// Select a web-view in an existing browser window
function selectWebView(windowId, webviewId) {
  const id = Date.now();

  // Send selectWebView message to the specific window
  searchChannel.postMessage({
    type: "selectWebView",
    id: id,
    targetWindowId: windowId,
    webviewId: webviewId,
  });

  // Set up fallback timeout (in case window closed)
  pendingNavigation = {
    id: id,
    timeout: setTimeout(() => {
      pendingNavigation = null;
      // Window didn't respond - just close search
      // navigator.embedder.closeCurrentOSWindow();
    }, 100),
  };
}

// Initialize search controller
const controller = new SearchController({
  onNavigate: navigateTo,
  onSelectWebView: selectWebView,
  onResultsChanged: renderResults,
});

// Render results to the DOM
function renderResults(results, groups) {
  resultsList.innerHTML = "";

  if (results.length === 0) {
    container.classList.remove("has-results");
    return;
  }

  container.classList.add("has-results");

  for (const group of groups) {
    const groupDiv = document.createElement("div");
    groupDiv.className = "result-group";

    // Icon column
    const iconDiv = document.createElement("div");
    iconDiv.className = "result-group-icon";
    if (group.providerIcon) {
      const icon = document.createElement("lucide-icon");
      icon.setAttribute("name", group.providerIcon);
      iconDiv.appendChild(icon);
    }
    groupDiv.appendChild(iconDiv);

    // Items column
    const itemsDiv = document.createElement("div");
    itemsDiv.className = "result-group-items";

    for (const result of group.items) {
      const itemDiv = document.createElement("div");
      itemDiv.className = "result-item";
      itemDiv.dataset.kind = result.kind;

      if (result.kind === "link" || result.kind === "webview") {
        const link = document.createElement("a");
        link.href = result.kind === "link" ? result.value.url : "#";
        link.className = "result-link";
        link.textContent = result.value.title;
        link.addEventListener("click", (e) => {
          e.preventDefault();
          controller.handleResultClick(result);
        });
        itemDiv.appendChild(link);
      } else if (result.kind === "text") {
        const text = document.createElement("span");
        text.className = "result-text";
        text.textContent = result.value;
        itemDiv.appendChild(text);
      }

      itemsDiv.appendChild(itemDiv);
    }

    groupDiv.appendChild(itemsDiv);
    resultsList.appendChild(groupDiv);
  }
}

// Event listeners
searchInput.addEventListener("input", () => {
  controller.query(searchInput.value);
});

searchInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    controller.handleSubmit(searchInput.value);
  }
});

// Close window on Escape from anywhere
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    navigator.embedder.closeCurrentOSWindow();
  }
});

document.querySelector("h1").addEventListener("mousedown", () => {
  navigator.embedder.startWindowDrag();
});

searchInput.focus();
