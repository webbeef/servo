// SPDX-License-Identifier: AGPL-3.0-or-later

// ============================================================================
// LayoutManager: Manages the split tree and CSS Grid layout
// ============================================================================

import "./overview.js"; // Registers the panel-overview custom element

export class LayoutManager {
  constructor(rootElement, webViewBuilder) {
    this.root = rootElement;
    // Top-level is an array of "panels" (each panel is a split tree)
    // New tabs add new panels, splits divide within a panel
    this.panels = []; // Array of { tree, element } where element is a panel container
    this.webviews = new Map(); // webviewId -> { webview, panelIndex }
    this.nextId = 1;
    this.activeWebviewId = null;

    // Sidebar icons container
    this.viewsIconsContainer = document.getElementById("views-icons");

    // Create floating preview element for sidebar hover
    this.sidebarPreview = this.createSidebarPreview();

    // Overview mode state
    this.overviewMode = false;
    this.overviewElement = null;

    // Listen for webview events
    document.addEventListener("webview-split", (e) => {
      this.splitWebView(e.detail.webviewId, e.detail.direction);
    });

    document.addEventListener("webview-close", (e) => {
      this.removeWebView(e.detail.webviewId);
    });

    document.addEventListener("webview-focus", (e) => {
      this.setActiveWebView(e.detail.webviewId);
    });

    document.addEventListener("webview-resize-panel", (e) => {
      this.resizePanel(e.detail.webviewId, e.detail.delta);
    });

    document.addEventListener("webview-favicon-change", (e) => {
      this.updateSidebarIcon(e.detail.webviewId, e.detail.favicon);
    });

    this.webViewBuilder = webViewBuilder;
  }

  generateId() {
    return `wv-${this.nextId++}`;
  }

  // Create a new panel (container for a split tree) - used by Cmd+T
  createPanel() {
    const panel = document.createElement("div");
    panel.className = "panel";
    this.root.appendChild(panel);
    return panel;
  }

  // Apply panel width based on widthPercent
  applyPanelWidth(panelIndex) {
    const panel = this.panels[panelIndex];
    if (!panel) {
      return;
    }
    const widthPercent = panel.widthPercent || 100;
    // Subtract sidebar width from viewport-based calculation
    panel.element.style.width = `calc(${widthPercent}vw - var(--sidebar-width) - 1em)`;
    panel.element.style.minWidth = `calc(${widthPercent}vw - var(--sidebar-width) - 1em)`;
  }

  // Resize a panel by delta percent (e.g., +10 or -10)
  resizePanel(webviewId, delta) {
    const entry = this.webviews.get(webviewId);
    if (!entry) {
      return;
    }
    const panel = this.panels[entry.panelIndex];
    if (!panel) {
      return;
    }
    const currentWidth = panel.widthPercent || 100;
    const newWidth = Math.max(30, Math.min(200, currentWidth + delta));
    panel.widthPercent = newWidth;
    this.applyPanelWidth(entry.panelIndex);
  }

  // Add a new webview as a new panel (Cmd+T behavior)
  addWebView(webview) {
    const id = this.generateId();
    webview.webviewId = id;

    // Create a new panel for this webview
    const panelElement = this.createPanel();
    const panelIndex = this.panels.length;

    this.panels.push({
      tree: { type: "leaf", webviewId: id },
      element: panelElement,
    });

    this.webviews.set(id, { webview, panelIndex });
    panelElement.appendChild(webview);

    // Create sidebar icon for this webview
    this.createSidebarIcon(id);

    this.applyPanelLayout(panelIndex);
    this.setActiveWebView(id);
    this.scrollToPanel(panelIndex);
    return webview;
  }

  // Scroll to a specific panel
  scrollToPanel(panelIndex) {
    const panel = this.panels[panelIndex];
    if (panel && panel.element) {
      panel.element.scrollIntoView({ behavior: "smooth", inline: "start" });
    }
  }

  // Navigate to the next panel
  nextPanel() {
    if (this.panels.length <= 1) {
      return;
    }
    const entry = this.webviews.get(this.activeWebviewId);
    if (!entry) {
      return;
    }
    const nextIndex = (entry.panelIndex + 1) % this.panels.length;
    const leaf = this.findFirstLeaf(this.panels[nextIndex].tree);
    if (leaf) {
      this.setActiveWebView(leaf.webviewId);
      this.scrollToPanel(nextIndex);
    }
  }

  // Navigate to the previous panel
  prevPanel() {
    if (this.panels.length <= 1) {
      return;
    }
    const entry = this.webviews.get(this.activeWebviewId);
    if (!entry) {
      return;
    }
    const prevIndex =
      (entry.panelIndex - 1 + this.panels.length) % this.panels.length;
    const leaf = this.findFirstLeaf(this.panels[prevIndex].tree);
    if (leaf) {
      this.setActiveWebView(leaf.webviewId);
      this.scrollToPanel(prevIndex);
    }
  }

  // Split within the same panel (split button behavior)
  splitWebView(webviewId, direction) {
    const entry = this.webviews.get(webviewId);
    if (!entry) {
      return;
    }

    const { panelIndex } = entry;
    const panel = this.panels[panelIndex];

    const newId = this.generateId();
    const newWebview = this.webViewBuilder();
    newWebview.webviewId = newId;

    this.webviews.set(newId, { webview: newWebview, panelIndex });
    panel.element.appendChild(newWebview);

    // Create sidebar icon for this webview
    this.createSidebarIcon(newId);

    // Replace the leaf with a split node
    // ratio is the fraction of space given to the first child (0.5 = 50/50)
    panel.tree = this.replaceLeaf(panel.tree, webviewId, {
      type: "split",
      direction,
      ratio: 0.5,
      children: [
        { type: "leaf", webviewId },
        { type: "leaf", webviewId: newId },
      ],
    });

    this.applyPanelLayout(panelIndex);
    this.setActiveWebView(newId);
  }

  removeWebView(webviewId) {
    const entry = this.webviews.get(webviewId);
    if (!entry) {
      return;
    }

    const { webview, panelIndex } = entry;
    webview.remove();
    this.webviews.delete(webviewId);

    // Remove sidebar icon for this webview
    this.removeSidebarIcon(webviewId);

    const panel = this.panels[panelIndex];
    panel.tree = this.removeLeaf(panel.tree, webviewId);

    // If panel is empty, remove it
    if (!panel.tree) {
      panel.element.remove();
      this.panels.splice(panelIndex, 1);

      // Update panel indices for remaining webviews
      for (const [id, entry] of this.webviews) {
        if (entry.panelIndex > panelIndex) {
          entry.panelIndex--;
        }
      }

      // No panels left, bail out.
      if (this.panels.length === 0) {
        return;
      }
    } else {
      this.applyPanelLayout(panelIndex);
    }

    // Update active webview if needed
    if (this.activeWebviewId === webviewId) {
      const firstLeaf = this.findFirstLeafInAnyPanel();
      if (firstLeaf) {
        this.setActiveWebView(firstLeaf);
      }
    }
  }

  findFirstLeafInAnyPanel() {
    for (const panel of this.panels) {
      const leaf = this.findFirstLeaf(panel.tree);
      if (leaf) {
        return leaf.webviewId;
      }
    }
    return null;
  }

  setActiveWebView(webviewId) {
    // Remove active state from previous
    if (this.activeWebviewId && this.webviews.has(this.activeWebviewId)) {
      this.webviews.get(this.activeWebviewId).webview.active = false;
    }

    // Update sidebar icon active states
    const prevIcon = this.viewsIconsContainer?.querySelector(
      `[data-webview-id="${this.activeWebviewId}"]`,
    );
    if (prevIcon) {
      prevIcon.classList.remove("active");
    }

    this.activeWebviewId = webviewId;

    // Set active state on new
    if (webviewId && this.webviews.has(webviewId)) {
      this.webviews.get(webviewId).webview.active = true;
    }

    const newIcon = this.viewsIconsContainer?.querySelector(
      `[data-webview-id="${webviewId}"]`,
    );
    if (newIcon) {
      newIcon.classList.add("active");
    }
  }

  // ============================================================================
  // Sidebar Icon Management
  // ============================================================================

  // Create a sidebar icon for a webview
  createSidebarIcon(webviewId) {
    if (!this.viewsIconsContainer) {
      return;
    }

    const icon = document.createElement("div");
    icon.className = "view-icon";
    icon.dataset.webviewId = webviewId;

    // Create an img element for the favicon
    const img = document.createElement("img");
    img.className = "view-icon-img";
    img.src = ""; // Will be set when favicon loads
    icon.appendChild(img);

    // Click handler to activate and scroll to the webview
    icon.addEventListener("click", () => {
      const entry = this.webviews.get(webviewId);
      if (entry) {
        this.setActiveWebView(webviewId);
        this.scrollToPanel(entry.panelIndex);
      }
    });

    // Hover handlers for floating preview
    icon.addEventListener("mouseenter", () => {
      this.showSidebarPreview(webviewId, icon);
    });

    icon.addEventListener("mouseleave", () => {
      this.hideSidebarPreview();
    });

    this.viewsIconsContainer.appendChild(icon);
  }

  // Update the sidebar icon favicon
  updateSidebarIcon(webviewId, favicon) {
    if (!this.viewsIconsContainer) {
      return;
    }

    const icon = this.viewsIconsContainer.querySelector(
      `[data-webview-id="${webviewId}"]`,
    );
    if (icon) {
      const img = icon.querySelector(".view-icon-img");
      if (img) {
        img.src = favicon || "";
      }
    }
  }

  // Remove a sidebar icon
  removeSidebarIcon(webviewId) {
    if (!this.viewsIconsContainer) {
      return;
    }

    const icon = this.viewsIconsContainer.querySelector(
      `[data-webview-id="${webviewId}"]`,
    );
    if (icon) {
      icon.remove();
    }
  }

  // Create the floating preview element
  createSidebarPreview() {
    const preview = document.createElement("div");
    preview.className = "sidebar-preview";
    preview.innerHTML = `
      <div class="sidebar-preview-title"></div>
      <img class="sidebar-preview-screenshot" />
    `;
    document.body.appendChild(preview);
    return preview;
  }

  // Show the sidebar preview for a webview
  showSidebarPreview(webviewId, iconElement) {
    const entry = this.webviews.get(webviewId);
    if (!entry) {
      return;
    }

    const { webview } = entry;
    const title = this.sidebarPreview.querySelector(".sidebar-preview-title");
    const screenshot = this.sidebarPreview.querySelector(
      ".sidebar-preview-screenshot",
    );

    title.textContent = webview.title || "Untitled";

    if (webview.screenshotUrl) {
      screenshot.src = webview.screenshotUrl;
      screenshot.style.display = "block";
    } else {
      screenshot.style.display = "none";
    }

    // Position the preview to the right of the icon
    const iconRect = iconElement.getBoundingClientRect();
    this.sidebarPreview.style.left = `${iconRect.right + 8}px`;
    this.sidebarPreview.style.top = `${iconRect.top}px`;

    this.sidebarPreview.classList.add("visible");
  }

  // Hide the sidebar preview
  hideSidebarPreview() {
    this.sidebarPreview.classList.remove("visible");
  }

  // Replace a leaf node with a new node (used for splitting)
  replaceLeaf(node, webviewId, replacement) {
    if (!node) {
      return null;
    }

    if (node.type === "leaf") {
      return node.webviewId === webviewId ? replacement : node;
    }

    return {
      ...node,
      children: node.children.map((child) =>
        this.replaceLeaf(child, webviewId, replacement),
      ),
    };
  }

  // Remove a leaf node and collapse single-child splits
  removeLeaf(node, webviewId) {
    if (!node) {
      return null;
    }

    if (node.type === "leaf") {
      return node.webviewId === webviewId ? null : node;
    }

    // Process children
    const newChildren = node.children
      .map((child) => this.removeLeaf(child, webviewId))
      .filter((child) => child !== null);

    // If no children left, this node is gone
    if (newChildren.length === 0) {
      return null;
    }

    // If only one child, collapse the split
    if (newChildren.length === 1) {
      return newChildren[0];
    }

    return { ...node, children: newChildren };
  }

  findFirstLeaf(node) {
    if (!node) {
      return null;
    }
    if (node.type === "leaf") {
      return node;
    }
    return this.findFirstLeaf(node.children[0]);
  }

  // Apply grid layout within a single panel
  applyPanelLayout(panelIndex) {
    const panel = this.panels[panelIndex];
    if (!panel || !panel.tree) {
      return;
    }

    // Remove existing resize handles
    panel.element.querySelectorAll(".resize-handle").forEach((h) => h.remove());

    // Collect all split points to create grid tracks
    const colPoints = new Set([0, 100]);
    const rowPoints = new Set([0, 100]);
    this.collectSplitPoints(panel.tree, 0, 100, 0, 100, colPoints, rowPoints);

    // Sort points to create ordered grid lines
    const colLines = [...colPoints].sort((a, b) => a - b);
    const rowLines = [...rowPoints].sort((a, b) => a - b);

    // Generate grid template from split points (using fr units based on percentages)
    const colTemplate = [];
    for (let i = 0; i < colLines.length - 1; i++) {
      colTemplate.push(`${colLines[i + 1] - colLines[i]}fr`);
    }
    const rowTemplate = [];
    for (let i = 0; i < rowLines.length - 1; i++) {
      rowTemplate.push(`${rowLines[i + 1] - rowLines[i]}fr`);
    }

    panel.element.style.gridTemplateColumns = colTemplate.join(" ");
    panel.element.style.gridTemplateRows = rowTemplate.join(" ");

    // Compute grid positions for each webview
    const positions = this.computeGridPositions(
      panel.tree,
      0,
      100,
      0,
      100,
      colLines,
      rowLines,
    );

    for (const pos of positions) {
      const entry = this.webviews.get(pos.webviewId);
      if (entry) {
        const wv = entry.webview;
        wv.style.gridColumn = pos.gridColumn;
        wv.style.gridRow = pos.gridRow;
      }
    }

    // Add resize handles for splits
    this.addResizeHandles(panel, panelIndex, colLines, rowLines);
  }

  // Collect all split points in the tree
  collectSplitPoints(node, left, right, top, bottom, colPoints, rowPoints) {
    if (node.type === "leaf") {
      return;
    }

    const ratio = node.ratio || 0.5;

    if (node.direction === "horizontal") {
      const mid = left + (right - left) * ratio;
      colPoints.add(mid);
      this.collectSplitPoints(
        node.children[0],
        left,
        mid,
        top,
        bottom,
        colPoints,
        rowPoints,
      );
      this.collectSplitPoints(
        node.children[1],
        mid,
        right,
        top,
        bottom,
        colPoints,
        rowPoints,
      );
    } else {
      const mid = top + (bottom - top) * ratio;
      rowPoints.add(mid);
      this.collectSplitPoints(
        node.children[0],
        left,
        right,
        top,
        mid,
        colPoints,
        rowPoints,
      );
      this.collectSplitPoints(
        node.children[1],
        left,
        right,
        mid,
        bottom,
        colPoints,
        rowPoints,
      );
    }
  }

  // Compute grid column/row for each webview
  computeGridPositions(node, left, right, top, bottom, colLines, rowLines) {
    if (node.type === "leaf") {
      // Find which grid lines correspond to our bounds
      const colStart = colLines.indexOf(left) + 1;
      const colEnd = colLines.indexOf(right) + 1;
      const rowStart = rowLines.indexOf(top) + 1;
      const rowEnd = rowLines.indexOf(bottom) + 1;

      return [
        {
          webviewId: node.webviewId,
          gridColumn: `${colStart} / ${colEnd}`,
          gridRow: `${rowStart} / ${rowEnd}`,
        },
      ];
    }

    const ratio = node.ratio || 0.5;

    if (node.direction === "horizontal") {
      const mid = left + (right - left) * ratio;
      return [
        ...this.computeGridPositions(
          node.children[0],
          left,
          mid,
          top,
          bottom,
          colLines,
          rowLines,
        ),
        ...this.computeGridPositions(
          node.children[1],
          mid,
          right,
          top,
          bottom,
          colLines,
          rowLines,
        ),
      ];
    } else {
      const mid = top + (bottom - top) * ratio;
      return [
        ...this.computeGridPositions(
          node.children[0],
          left,
          right,
          top,
          mid,
          colLines,
          rowLines,
        ),
        ...this.computeGridPositions(
          node.children[1],
          left,
          right,
          mid,
          bottom,
          colLines,
          rowLines,
        ),
      ];
    }
  }

  // Find split node that contains a webview
  findParentSplit(node, webviewId, parent = null) {
    if (!node) {
      return null;
    }
    if (node.type === "leaf") {
      return node.webviewId === webviewId ? parent : null;
    }
    for (const child of node.children) {
      const result = this.findParentSplit(child, webviewId, node);
      if (result) {
        return result;
      }
    }
    return null;
  }

  // Add resize handles between split children
  addResizeHandles(panel, panelIndex, colLines, rowLines) {
    const handles = this.collectSplitHandles(panel.tree, 0, 100, 0, 100);

    for (const handle of handles) {
      const handleEl = document.createElement("div");
      handleEl.className = `resize-handle resize-handle-${handle.direction}`;
      // Use absolute positioning for handles to overlay the gap
      handleEl.style.position = "absolute";

      if (handle.direction === "horizontal") {
        // Vertical bar at horizontal split point
        const leftPercent = handle.position;
        const topPercent = handle.top;
        const heightPercent = handle.bottom - handle.top;
        handleEl.style.left = `calc(${leftPercent}% - 0.25em)`;
        handleEl.style.top = `${topPercent}%`;
        handleEl.style.width = "0.5em";
        handleEl.style.height = `${heightPercent}%`;
      } else {
        // Horizontal bar at vertical split point
        const topPercent = handle.position;
        const leftPercent = handle.left;
        const widthPercent = handle.right - handle.left;
        handleEl.style.left = `${leftPercent}%`;
        handleEl.style.top = `calc(${topPercent}% - 0.25em)`;
        handleEl.style.width = `${widthPercent}%`;
        handleEl.style.height = "0.5em";
      }

      handleEl.addEventListener("mousedown", (e) => {
        e.preventDefault();
        this.startResize(e, handle.splitNode, handle.direction, panelIndex);
      });

      panel.element.appendChild(handleEl);
    }
  }

  // Collect information about where to place resize handles
  collectSplitHandles(node, left = 0, right = 100, top = 0, bottom = 100) {
    if (node.type === "leaf") {
      return [];
    }

    const handles = [];
    const ratio = node.ratio || 0.5;

    if (node.direction === "horizontal") {
      const mid = left + (right - left) * ratio;
      handles.push({
        splitNode: node,
        direction: "horizontal",
        position: mid,
        top,
        bottom,
      });
      handles.push(
        ...this.collectSplitHandles(node.children[0], left, mid, top, bottom),
      );
      handles.push(
        ...this.collectSplitHandles(node.children[1], mid, right, top, bottom),
      );
    } else {
      const mid = top + (bottom - top) * ratio;
      handles.push({
        splitNode: node,
        direction: "vertical",
        position: mid,
        left,
        right,
      });
      handles.push(
        ...this.collectSplitHandles(node.children[0], left, right, top, mid),
      );
      handles.push(
        ...this.collectSplitHandles(node.children[1], left, right, mid, bottom),
      );
    }

    return handles;
  }

  // Start resize operation
  startResize(e, splitNode, direction, panelIndex) {
    const panel = this.panels[panelIndex];
    const panelRect = panel.element.getBoundingClientRect();
    const startX = e.clientX;
    const startY = e.clientY;
    const startRatio = splitNode.ratio || 0.5;

    // Disable pointer events on all webviews during resize
    document.body.classList.add("resizing");

    const onMouseMove = (e) => {
      e.preventDefault();
      e.stopPropagation();

      let delta;
      if (direction === "horizontal") {
        delta = (e.clientX - startX) / panelRect.width;
      } else {
        delta = (e.clientY - startY) / panelRect.height;
      }
      splitNode.ratio = Math.max(0.1, Math.min(0.9, startRatio + delta));
      this.applyPanelLayout(panelIndex);
    };

    const onMouseUp = (e) => {
      e.preventDefault();
      e.stopPropagation();

      document.body.removeEventListener("mousemove", onMouseMove, true);
      document.body.removeEventListener("mouseup", onMouseUp, true);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";

      // Re-enable pointer events on webviews
      document.body.classList.remove("resizing");
    };

    // Use capture phase on body to intercept events before they reach webviews
    document.body.addEventListener("mousemove", onMouseMove, true);
    document.body.addEventListener("mouseup", onMouseUp, true);
    document.body.style.cursor =
      direction === "horizontal" ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";
  }

  // ============================================================================
  // Overview Mode
  // ============================================================================

  // Toggle overview mode on/off
  toggleOverview() {
    if (this.overviewMode) {
      this.hideOverview();
    } else {
      this.showOverview();
    }
  }

  // Show overview mode with screenshots of all webviews
  showOverview() {
    if (this.overviewMode) {
      return;
    }
    this.overviewMode = true;

    // Ensure the overview element exists
    this.ensureOverviewElement();

    // Build overview data and set properties directly on the element
    const overviewData = this.buildOverviewData();
    this.overviewElement.panels = overviewData.panels;
    this.overviewElement.rootWidth = overviewData.rootWidth;
    this.overviewElement.rootHeight = overviewData.rootHeight;
    this.overviewElement.open = true;
  }

  // Hide overview mode
  hideOverview() {
    if (!this.overviewMode) {
      return;
    }
    this.overviewMode = false;

    if (this.overviewElement) {
      this.overviewElement.open = false;
    }
  }

  // Ensure overview element exists
  ensureOverviewElement() {
    if (this.overviewElement) {
      return;
    }

    // Create the panel-overview element
    const overview = document.createElement("panel-overview");
    document.body.appendChild(overview);
    this.overviewElement = overview;

    // Listen for events from the overview element
    overview.addEventListener("overview-select", (e) => {
      this.setActiveWebView(e.detail.webviewId);
      if (e.detail.panelIndex !== undefined) {
        this.scrollToPanel(e.detail.panelIndex);
      }
    });

    overview.addEventListener("overview-close", () => {
      this.hideOverview();
    });
  }

  // Build the data structure for the overview element
  buildOverviewData() {
    const panels = [];

    // Get the root's bounding rect as reference for relative positioning
    const rootRect = this.root.getBoundingClientRect();

    for (let panelIndex = 0; panelIndex < this.panels.length; panelIndex++) {
      const panel = this.panels[panelIndex];
      const panelRect = panel.element.getBoundingClientRect();

      // Build thumbnails data with actual bounding rects
      const thumbnails = [];
      const leafIds = this.collectLeafIds(panel.tree);

      for (const webviewId of leafIds) {
        const entry = this.webviews.get(webviewId);
        if (!entry) continue;

        // Get the webview's bounding rect relative to the panel
        const wvRect = entry.webview.getBoundingClientRect();

        thumbnails.push({
          webviewId: webviewId,
          panelIndex: panelIndex,
          title: entry.webview.title,
          screenshotUrl: entry.webview.screenshotUrl || null,
          themeColor: entry.webview.themeColor || null,
          active: webviewId === this.activeWebviewId,
          // Position relative to panel origin
          x: wvRect.left - panelRect.left,
          y: wvRect.top - panelRect.top,
          width: wvRect.width,
          height: wvRect.height,
        });
      }

      panels.push({
        width: panelRect.width,
        height: panelRect.height,
        thumbnails,
      });
    }

    return {
      panels,
      rootWidth: rootRect.width,
      rootHeight: rootRect.height,
    };
  }

  // Collect all leaf webview IDs from a tree
  collectLeafIds(node) {
    if (!node) {
      return [];
    }
    if (node.type === "leaf") {
      return [node.webviewId];
    }
    return [
      ...this.collectLeafIds(node.children[0]),
      ...this.collectLeafIds(node.children[1]),
    ];
  }
}
