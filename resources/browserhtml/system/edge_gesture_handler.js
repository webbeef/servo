// SPDX-License-Identifier: AGPL-3.0-or-later

export class EdgeGestureHandler {
  constructor(layoutManager) {
    this.lm = layoutManager;

    // Configuration
    this.edgeThreshold = 30; // px from edge to trigger edge swipe
    this.swipeThreshold = 50; // px to complete a swipe action
    this.longSwipeThreshold = 200; // px for long swipe (overview mode)

    // Touch state
    this.touchStartX = 0;
    this.touchStartY = 0;
    this.touchCurrentX = 0;
    this.touchCurrentY = 0;
    this.isEdgeSwipe = false;
    this.edgeDirection = null; // 'left', 'right', 'bottom', 'top'
    this.isPeeking = false;

    // Peek preview element
    this.peekPreview = null;

    // Edge overlay elements
    this.edgeOverlays = {};

    this.createEdgeOverlays();
  }

  createEdgeOverlays() {
    // Create thin overlay strips along each edge
    const edges = [
      {
        name: "left",
        style: `left: 0; top: 0; width: ${this.edgeThreshold}px; height: 100%;`,
      },
      {
        name: "right",
        style: `right: 0; top: 0; width: ${this.edgeThreshold}px; height: 100%;`,
      },
      {
        name: "top",
        style: `top: 0; left: 0; width: 100%; height: ${this.edgeThreshold}px;`,
      },
      {
        name: "bottom",
        style: `bottom: 0; left: 0; width: 100%; height: ${this.edgeThreshold}px;`,
      },
    ];

    edges.forEach((edge) => {
      const overlay = document.createElement("div");
      overlay.className = `gesture-edge-overlay gesture-edge-${edge.name}`;
      overlay.style.cssText = `
        position: fixed;
        ${edge.style}
        z-index: var(--z-gesture);
        touch-action: none;
        pointer-events: auto;
        background: transparent; /* rgba(77, 215, 220, 0.49); */
      `;
      overlay.dataset.edge = edge.name;
      document.body.appendChild(overlay);
      this.edgeOverlays[edge.name] = overlay;

      // Add listeners to each edge overlay
      overlay.addEventListener("touchstart", this.onEdgeTouchStart.bind(this), {
        capture: false,
      });
      overlay.addEventListener("touchmove", this.onTouchMove.bind(this), {
        capture: false,
      });
      overlay.addEventListener("touchend", this.onTouchEnd.bind(this), {
        capture: false,
      });
      overlay.addEventListener("touchcancel", this.onTouchCancel.bind(this), {
        capture: false,
      });
    });
  }

  onEdgeTouchStart(event) {
    console.log(`[EdgeGesture] touchstart`);
    console.log("onEdgeTouchStart");
    if (event.touches.length !== 1) {
      return;
    }

    const touch = event.touches[0];

    this.touchStartX = touch.clientX;
    this.touchStartY = touch.clientY;
    this.touchCurrentX = touch.clientX;
    this.touchCurrentY = touch.clientY;

    // Set edge swipe state based on which edge was touched
    this.isEdgeSwipe = true;
    this.edgeDirection = event.currentTarget.dataset.edge;

    event.preventDefault();
  }

  onTouchMove(event) {
    console.log(`[EdgeGesture] touchmove isEdgeSwipe=${this.isEdgeSwipe}`);

    if (!this.isEdgeSwipe || event.touches.length !== 1) {
      return;
    }

    const touch = event.touches[0];
    this.touchCurrentX = touch.clientX;
    this.touchCurrentY = touch.clientY;

    const deltaX = this.touchCurrentX - this.touchStartX;
    const deltaY = this.touchCurrentY - this.touchStartY;

    // Handle horizontal edge swipes (left/right)
    if (this.edgeDirection === "left" && deltaX > 0) {
      event.preventDefault();
      this.handleLeftEdgeMove(deltaX);
    } else if (this.edgeDirection === "right" && deltaX < 0) {
      event.preventDefault();
      this.handleRightEdgeMove(-deltaX);
    } else if (this.edgeDirection === "bottom" && deltaY < 0) {
      event.preventDefault();
      this.handleBottomEdgeMove(-deltaY);
    } else if (this.edgeDirection === "top" && deltaY > 0) {
      event.preventDefault();
      this.handleTopEdgeMove(deltaY);
    }
  }

  onTouchEnd() {
    console.log(`[EdgeGesture] touchend ${this.edgeDirection}`);
    if (!this.isEdgeSwipe) {
      return;
    }

    const deltaX = this.touchCurrentX - this.touchStartX;
    const deltaY = this.touchCurrentY - this.touchStartY;

    // Complete the gesture based on direction and distance
    if (this.edgeDirection === "left") {
      this.completeLeftEdgeSwipe(deltaX);
    } else if (this.edgeDirection === "right") {
      this.completeRightEdgeSwipe(-deltaX);
    }

    // Reset state
    this.isEdgeSwipe = false;
    this.edgeDirection = null;
    this.isPeeking = false;
    this.hidePeekPreview();
  }

  onTouchCancel() {
    console.log(`[EdgeGesture] touchcancel`);
    this.isEdgeSwipe = false;
    this.edgeDirection = null;
    this.isPeeking = false;
    this.hidePeekPreview();
  }

  // ============================================================================
  // Left Edge (Previous WebView)
  // ============================================================================

  handleLeftEdgeMove(distance) {
    const progress = Math.min(distance / this.swipeThreshold, 1);
    console.log(
      `[EdgeGesture] handleLeftEdgeMove distance=${distance} progress=${progress} isPeeking=${this.isPeeking}`,
    );

    // Show peek preview of previous webview
    if (!this.isPeeking && progress > 0.1) {
      this.isPeeking = true;
      this.showPeekPreview("prev", progress);
    } else if (this.isPeeking) {
      this.updatePeekPreview(progress);
    }
  }

  completeLeftEdgeSwipe(distance) {
    if (distance >= this.longSwipeThreshold) {
      // Long swipe: enter overview mode
      this.lm.showOverview();
    } else if (distance >= this.swipeThreshold) {
      // Normal swipe: switch to previous webview
      this.lm.prevWebView();
    }
    // Otherwise: cancelled, peek snaps back
  }

  // ============================================================================
  // Right Edge (Next WebView)
  // ============================================================================

  handleRightEdgeMove(distance) {
    const progress = Math.min(distance / this.swipeThreshold, 1);

    // Show peek preview of next webview
    if (!this.isPeeking && progress > 0.1) {
      this.isPeeking = true;
      this.showPeekPreview("next", progress);
    } else if (this.isPeeking) {
      this.updatePeekPreview(progress);
    }
  }

  completeRightEdgeSwipe(distance) {
    if (distance >= this.longSwipeThreshold) {
      // Long swipe: enter overview mode
      this.lm.showOverview();
    } else if (distance >= this.swipeThreshold) {
      // Normal swipe: switch to next webview
      this.lm.nextWebView();
    }
  }

  // ============================================================================
  // Bottom Edge (Action Bar)
  // ============================================================================

  handleBottomEdgeMove(distance) {
    console.log(`[EdgeGesture] handleBottomEdgeMove ${distance}px`);
    if (distance >= this.edgeThreshold / 2) {
      // Swipe up from bottom: show action bar
      this.lm.showActionBar();
    }
  }

  // ============================================================================
  // Top Edge (Notifications)
  // ============================================================================

  handleTopEdgeMove(distance) {
    if (distance >= this.edgeThreshold / 2) {
      // Swipe down from top: show notifications
      // Dispatch event for notification panel
      document.dispatchEvent(
        new CustomEvent("mobile-show-notifications", { bubbles: true }),
      );
    }
  }

  // ============================================================================
  // Peek Preview
  // ============================================================================

  showPeekPreview(direction, progress) {
    console.log(`[EdgeGesture] showPeekPreview ${direction} ${progress}`);
    const adjacentId = this.lm.getAdjacentWebViewId(direction);
    if (!adjacentId) {
      console.error(`[EdgeGesture] no adjacentId`);
      return;
    }

    const info = this.lm.getWebViewInfo(adjacentId);
    if (!info) {
      console.error(`[EdgeGesture] no WebViewInfo for ${adjacentId}`);
      return;
    }

    // Create peek preview element if it doesn't exist
    if (!this.peekPreview) {
      this.peekPreview = document.createElement("div");
      this.peekPreview.className = "mobile-peek-preview";
      this.peekPreview.innerHTML = `
        <div class="peek-header">
          <img class="peek-favicon" src="" alt="">
          <span class="peek-title"></span>
        </div>
        <img class="peek-screenshot" src="" alt="">
      `;
      document.body.appendChild(this.peekPreview);
    }

    // Update content
    const favicon = this.peekPreview.querySelector(".peek-favicon");
    const title = this.peekPreview.querySelector(".peek-title");
    const screenshot = this.peekPreview.querySelector(".peek-screenshot");

    favicon.src = info.favicon || "";
    title.textContent = info.title;
    if (info.screenshotUrl) {
      screenshot.src = info.screenshotUrl;
      screenshot.style.display = "block";
    } else {
      screenshot.style.display = "none";
    }

    // Position based on direction
    this.peekPreview.classList.remove("from-left", "from-right");
    this.peekPreview.classList.add(
      direction === "prev" ? "from-left" : "from-right",
    );
    this.peekPreview.classList.add("visible");

    this.updatePeekPreview(progress);
  }

  updatePeekPreview(progress) {
    console.log(`[EdgeGesture] updatePeekPreview ${progress}`);
    if (!this.peekPreview) {
      return;
    }

    // Animate the preview sliding in
    const maxTranslate = 30; // percentage of screen width to reveal
    const translatePercent = progress * maxTranslate;

    if (this.peekPreview.classList.contains("from-left")) {
      this.peekPreview.style.transform = `translateX(${-100 + translatePercent}%)`;
    } else {
      this.peekPreview.style.transform = `translateX(${100 - translatePercent}%)`;
    }
  }

  hidePeekPreview() {
    console.log(`[EdgeGesture] hidePeekPreview`);
    if (this.peekPreview) {
      this.peekPreview.classList.remove("visible");
      this.peekPreview.style.transform = "";
    }
  }
}
