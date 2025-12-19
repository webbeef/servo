// SPDX-License-Identifier: AGPL-3.0-or-later

import { SearchController } from "//shared.localhost:8888/search/controller.js";

const container = document.getElementById("container");
const searchInput = document.getElementById("search-input");
const resultsList = document.getElementById("results-list");

// BroadcastChannel for communicating with browser window
const searchChannel = new BroadcastChannel("servo-search");
let pendingAction = null;

// Listen for acknowledgments
searchChannel.onmessage = (e) => {
  if (
    e.data.type === "ack" &&
    pendingAction &&
    e.data.id === pendingAction.id
  ) {
    clearTimeout(pendingAction.timeout);
    pendingAction = null;
  }
};

// Navigate to a URL
function navigateTo(url) {
  window.location.href = url;
}

// Select a web-view in a browser window
function selectWebView(windowId, webviewId) {
  const id = Date.now();

  searchChannel.postMessage({
    type: "selectWebView",
    id: id,
    targetWindowId: windowId,
    webviewId: webviewId,
  });

  pendingAction = {
    id: id,
    timeout: setTimeout(() => {
      pendingAction = null;
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
