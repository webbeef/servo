// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  getAvailableThemes,
  getTheme,
  setTheme,
} from "//shared.localhost:8888/theme.js";

import SearchEngines from "//shared.localhost:8888/search/engines.js";

// Bi-directionial binding between a preference and an element's value.
class PreferenceBindings {
  constructor() {
    navigator.embedder.addEventListener(
      "preferencechanged",
      this.onPrefChange.bind(this),
    );

    this.bindings = {};
  }

  add(prefName, element) {
    this.bindings[prefName] = element;
    element.addEventListener("input", (event) => {
      console.log(`[PreferenceBinding] ${event.type} for ${prefName}, value is ${element.value}`);
      navigator.servo.setStringPreference(prefName, element.value);
    });
    console.log(`[PreferenceBinding] ${prefName} is ${navigator.servo.getStringPreference(prefName)}`)
    element.value = navigator.servo.getStringPreference(prefName);
  }

  addBool(prefName, element) {
    this.bindings[prefName] = element;
    element.addEventListener("change", (event) => {
      console.log(`[PreferenceBinding] ${event.type} for ${prefName}, checked is ${element.checked}`);
      navigator.servo.setBoolPreference(prefName, element.checked);
    });
    console.log(`[PreferenceBinding] ${prefName} is ${navigator.servo.getBoolPreference(prefName)}`)
    element.checked = navigator.servo.getBoolPreference(prefName);
  }

  addInt(prefName, element) {
    this.bindings[prefName] = element;
    element.addEventListener("input", (event) => {
      const value = parseInt(element.value, 10);
      if (!isNaN(value)) {
        console.log(`[PreferenceBinding] ${event.type} for ${prefName}, value is ${value}`);
        navigator.servo.setIntPreference(prefName, value);
      }
    });
    console.log(`[PreferenceBinding] ${prefName} is ${navigator.servo.getIntPreference(prefName)}`)
    element.value = navigator.servo.getIntPreference(prefName);
  }

  onPrefChange(event) {
    let element = this.bindings[event.detail.name];
    if (element) {
      if (element.type === "checkbox") {
        element.checked = event.detail.value;
      } else {
        element.value = event.detail.value;
      }
    }
  }
}

// Setup sidebar navigation
function setupNavigation() {
  const navItems = document.querySelectorAll(".nav-item");

  navItems.forEach((item) => {
    item.addEventListener("click", (event) => {
      event.preventDefault();
      const categoryId = item.dataset.category;

      // Update active nav item
      navItems.forEach((nav) => nav.classList.remove("active"));
      item.classList.add("active");

      // Scroll to category and open its details
      const category = document.getElementById(categoryId);
      if (category) {
        category.scrollIntoView({ behavior: "smooth", block: "start" });
        const details = category.querySelector("details");
        if (details) {
          details.open = true;
        }
      }
    });
  });
}

// Mobile drill-down navigation
function setupMobileNavigation() {
  const navItems = document.querySelectorAll(".nav-item");
  const backButton = document.querySelector(".back-button");
  const mobileTitle = document.querySelector(".mobile-title");
  const categories = document.querySelectorAll(".settings-category");

  // Check if we're in mobile view
  function isMobile() {
    return window.matchMedia("(max-width: 600px)").matches;
  }

  // Show detail view for a category
  function showDetail(categoryId) {
    if (!isMobile()) {
      return;
    }
  
    document.body.classList.add("show-detail");

    // Mark the active category
    categories.forEach((cat) => {
      cat.classList.remove("active");
      if (cat.id === categoryId) {
        cat.classList.add("active");
        // Open the details element
        const details = cat.querySelector("details");
        if (details) details.open = true;
      }
    });

    // Update title
    const activeNav = document.querySelector(`[data-category="${categoryId}"]`);
    if (activeNav && mobileTitle) {
      mobileTitle.textContent = activeNav.querySelector("span").textContent;
    }
  }

  // Return to list view
  function showList() {
    document.body.classList.remove("show-detail");
    categories.forEach((cat) => cat.classList.remove("active"));
    if (mobileTitle) mobileTitle.textContent = "Settings";
  }

  // Nav item click handler (enhanced for mobile)
  navItems.forEach((item) => {
    item.addEventListener("click", () => {
      if (isMobile()) {
        showDetail(item.dataset.category);
      }
    });
  });

  // Back button handler
  if (backButton) {
    backButton.addEventListener("click", showList);
  }

  // Handle resize: reset state when switching between mobile/desktop
  window.addEventListener("resize", () => {
    if (!isMobile()) {
      document.body.classList.remove("show-detail");
      categories.forEach((cat) => cat.classList.remove("active"));
    }
  });
}

// Render theme cards
function renderThemeCards() {
  const grid = document.getElementById("theme-grid");
  if (!grid) {
    return;
  }

  const currentTheme = getTheme();
  const themes = getAvailableThemes();

  grid.innerHTML = themes
    .map(
      (theme) => `
          <div class="theme-card ${theme.id === currentTheme ? "selected" : ""}"
               data-theme="${theme.id}">
            <div class="theme-preview preview-${theme.id}">
              <div class="theme-preview-header"></div>
              <div class="theme-preview-main">
                <div class="theme-preview-webview"></div>
              </div>
            </div>
            <div class="theme-info">
              <div class="theme-name">${theme.name}</div>
              <div class="theme-description">${theme.description}</div>
            </div>
          </div>
        `,
    )
    .join("");

  // Add click handlers
  grid.querySelectorAll(".theme-card").forEach((card) => {
    card.addEventListener("click", () => {
      const themeId = card.dataset.theme;
      setTheme(themeId);

      // Update selected state
      grid
        .querySelectorAll(".theme-card")
        .forEach((c) => c.classList.remove("selected"));
      card.classList.add("selected");
    });
  });
}

async function setupSearchEngines() {
  let engines = new SearchEngines();
  await engines.ensureReady();

  let list = document.getElementById("search-engine-list");
  engines.engines.forEach((engine) => {
    console.log(`Adding search engine: ${engine.name}`);
    let option = document.createElement("option");
    option.setAttribute("value", engine.id);
    if (engine.id == engines.currentEngine.id) {
      option.setAttribute("selected", true);
    }
    option.textContent = engine.name;
    list.append(option);
  });
}

// Initialize
setupNavigation();
setupMobileNavigation();
renderThemeCards();
setupSearchEngines();

const bindings = new PreferenceBindings();

// Register bindings: [<element id>, <preference name>]
[
  ["new-view-url", "browserhtml.start_url"],
  ["homescreen-url", "browserhtml.homescreen_url"],
  ["search-engine-list", "browserhtml.search_engine"],
  ["user-agent", "user_agent"],
].forEach((item) => {
  bindings.add(item[1], document.getElementById(item[0]));
});

// Register boolean bindings: [<element id>, <preference name>]
[
  ["virtual-keyboard-toggle", "browserhtml.ime_virtual_keyboard_enabled"],
  ["mobile-simulation-toggle", "browserhtml.mobile_simulation"],
  ["devtools-toggle", "devtools_server_enabled"],
].forEach((item) => {
  bindings.addBool(item[1], document.getElementById(item[0]));
});

// Register integer bindings: [<element id>, <preference name>]
[
  ["devtools-port", "devtools_server_port"],
].forEach((item) => {
  bindings.addInt(item[1], document.getElementById(item[0]));
});
