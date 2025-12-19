// SPDX-License-Identifier: AGPL-3.0-or-later

// import { SearchController } from "//shared.localhost:8888/search/controller.js";

// DOM elements
const bookmarksGrid = document.getElementById("bookmarks-grid");
const resultsArea = document.getElementById("results-area");
const resultsList = document.getElementById("results-list");
const searchInput = document.getElementById("search-input");
const widgetsArea = document.getElementById("widgets-area");

// Load widgets from JSON
async function loadWidgets() {
  try {
    const response = await fetch("resources/widgets.json");
    const widgets = await response.json();
    renderWidgets(widgets);
  } catch (e) {
    console.log("[Homescreen] No widgets configured");
  }
}

// Render widgets as iframes
function renderWidgets(widgets) {
  for (const widget of widgets) {
    const iframe = document.createElement("iframe");
    iframe.className = "widget-frame";
    iframe.src = widget.url;
    iframe.style.width = widget.width || "100%";
    iframe.style.height = widget.height || "100px";
    widgetsArea.appendChild(iframe);
  }
}

// Load bookmarks from JSON
async function loadBookmarks() {
  try {
    const response = await fetch("resources/bookmarks.json");
    const bookmarks = await response.json();
    renderBookmarks(bookmarks);
  } catch (e) {
    console.error("[Homescreen] Failed to load bookmarks:", e);
  }
}

// Render bookmarks to the grid
function renderBookmarks(bookmarks) {
  bookmarksGrid.innerHTML = "";

  for (const bookmark of bookmarks) {
    const item = document.createElement("div");
    item.className = "bookmark-item";
    item.innerHTML = `
      <div class="bookmark-icon">
        <img src="resources/${bookmark.icon}" alt="" onerror="this.style.display='none'" />
      </div>
      <span class="bookmark-title">${bookmark.title}</span>
    `;
    // TODO: replace with <a target="_blank">
    item.addEventListener("click", () => {
      navigate(bookmark.url);
    });
    bookmarksGrid.appendChild(item);
  }
}

// Reset search UI to initial state
function resetSearchState() {
  searchInput.value = "";
  searchInput.blur();
  document.body.classList.remove("searching");
  controller.clear();
}

// Navigate to a URL
function navigate(url) {
  resetSearchState();
  window.open(url, "_blank");
}

class Controller {
  constructor() {
    this.inner = null;
  }

  async ensureController() {
    if (this.inner) {
      return;
    }

    let mod = await import("//shared.localhost:8888/search/controller.js");
    this.inner = new mod.SearchController({
      onNavigate: navigate,
      onResultsChanged: renderResults,
      debounceDelay: 150,
    });
  }

  async handleResultClick(result) {
    await this.ensureController();
    this.inner.handleResultClick(result);
  }

  async handleSubmit(data) {
    await this.ensureController();
    this.inner.handleSubmit(data);
  }

  async query(query) {
    await this.ensureController();
    this.inner.query(query);
  }

  async clear() {
    await this.ensureController();
    this.inner.clear();
  }
}

// Initialize search controller
const controller = new Controller();

// Render search results (adapted from new_view.js)
function renderResults(results, groups) {
  resultsList.innerHTML = "";

  if (results.length === 0) {
    return;
  }

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
  const query = searchInput.value.trim();
  if (query) {
    document.body.classList.add("searching");
    controller.query(query);
  } else {
    document.body.classList.remove("searching");
    controller.clear();
  }
});

searchInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    controller.handleSubmit(searchInput.value);
  }
});

// Initialize
loadWidgets();
loadBookmarks();
