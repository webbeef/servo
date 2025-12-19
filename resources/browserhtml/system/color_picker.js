// SPDX-License-Identifier: AGPL-3.0-or-later

import {
  LitElement,
  html,
  css,
} from "//shared.localhost:8888/third_party/lit/lit-all.min.js";

// Common preset colors
const PRESETS = [
  "#000000",
  "#ffffff",
  "#ff0000",
  "#00ff00",
  "#0000ff",
  "#ffff00",
  "#ff00ff",
  "#00ffff",
  "#ff8000",
  "#8000ff",
  "#808080",
  "#c0c0c0",
  "#800000",
  "#008000",
  "#000080",
];

export class ColorPicker extends LitElement {
  static properties = {
    open: { type: Boolean, reflect: true },
    currentColor: { type: String },
    controlId: { type: String },
    x: { type: Number },
    y: { type: Number },
    // Internal state (HSL)
    hue: { state: true },
    saturation: { state: true },
    lightness: { state: true },
  };

  constructor() {
    super();
    this.open = false;
    this.currentColor = "#000000";
    this.controlId = "";
    this.x = 0;
    this.y = 0;
    this.hue = 0;
    this.saturation = 100;
    this.lightness = 50;
    this.handleKeyDown = this.handleKeyDown.bind(this);
    this.dragging = null;
  }

  updated(changedProperties) {
    if (changedProperties.has("open")) {
      if (this.open) {
        document.addEventListener("keydown", this.handleKeyDown);
      } else {
        document.removeEventListener("keydown", this.handleKeyDown);
      }
    }
    if (changedProperties.has("currentColor") && this.currentColor) {
      this.parseInitialColor();
    }
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    document.removeEventListener("keydown", this.handleKeyDown);
  }

  handleKeyDown(e) {
    if (e.key === "Escape") {
      this.cancel();
    } else if (e.key === "Enter") {
      this.confirm();
    }
  }

  handleBackdropClick(e) {
    if (e.target.classList.contains("backdrop")) {
      this.cancel();
    }
  }

  // Parse the initial hex color to HSL
  parseInitialColor() {
    const hex = this.currentColor;
    if (!hex || !hex.startsWith("#")) {
      return;
    }

    const rgb = this.hexToRgb(hex);
    if (rgb) {
      const hsl = this.rgbToHsl(rgb.r, rgb.g, rgb.b);
      this.hue = hsl.h;
      this.saturation = hsl.s;
      this.lightness = hsl.l;
    }
  }

  // Color conversion helpers
  hexToRgb(hex) {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})/i.exec(hex);
    return result
      ? {
          r: parseInt(result[1], 16),
          g: parseInt(result[2], 16),
          b: parseInt(result[3], 16),
        }
      : null;
  }

  rgbToHex(r, g, b) {
    return (
      "#" +
      [r, g, b]
        .map((x) => {
          const hex = Math.round(x).toString(16);
          return hex.length === 1 ? "0" + hex : hex;
        })
        .join("")
    );
  }

  rgbToHsl(r, g, b) {
    r /= 255;
    g /= 255;
    b /= 255;
    const max = Math.max(r, g, b),
      min = Math.min(r, g, b);
    let h,
      s,
      l = (max + min) / 2;

    if (max === min) {
      h = s = 0;
    } else {
      const d = max - min;
      s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
      switch (max) {
        case r:
          h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
          break;
        case g:
          h = ((b - r) / d + 2) / 6;
          break;
        case b:
          h = ((r - g) / d + 4) / 6;
          break;
      }
    }

    return {
      h: Math.round(h * 360),
      s: Math.round(s * 100),
      l: Math.round(l * 100),
    };
  }

  hslToRgb(h, s, l) {
    h /= 360;
    s /= 100;
    l /= 100;
    let r, g, b;

    if (s === 0) {
      r = g = b = l;
    } else {
      const hue2rgb = (p, q, t) => {
        if (t < 0) t += 1;
        if (t > 1) t -= 1;
        if (t < 1 / 6) {
          return p + (q - p) * 6 * t;
        }
        if (t < 1 / 2) {
          return q;
        }
        if (t < 2 / 3) {
          return p + (q - p) * (2 / 3 - t) * 6;
        }
        return p;
      };
      const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
      const p = 2 * l - q;
      r = hue2rgb(p, q, h + 1 / 3);
      g = hue2rgb(p, q, h);
      b = hue2rgb(p, q, h - 1 / 3);
    }

    return {
      r: Math.round(r * 255),
      g: Math.round(g * 255),
      b: Math.round(b * 255),
    };
  }

  // Get current color as hex string
  get selectedColor() {
    const rgb = this.hslToRgb(this.hue, this.saturation, this.lightness);
    return this.rgbToHex(rgb.r, rgb.g, rgb.b);
  }

  get currentRgb() {
    return this.hslToRgb(this.hue, this.saturation, this.lightness);
  }

  // Event handlers for the picker areas
  handleSLMouseDown(e) {
    this.dragging = "sl";
    this.updateSL(e);
    document.addEventListener("mousemove", this.handleMouseMove);
    document.addEventListener("mouseup", this.handleMouseUp);
  }

  handleHueMouseDown(e) {
    this.dragging = "hue";
    this.updateHue(e);
    document.addEventListener("mousemove", this.handleMouseMove);
    document.addEventListener("mouseup", this.handleMouseUp);
  }

  handleMouseMove = (e) => {
    if (this.dragging === "sl") this.updateSL(e);
    else if (this.dragging === "hue") this.updateHue(e);
  };

  handleMouseUp = () => {
    this.dragging = null;
    document.removeEventListener("mousemove", this.handleMouseMove);
    document.removeEventListener("mouseup", this.handleMouseUp);
  };

  updateSL(e) {
    const rect = this.shadowRoot
      .querySelector(".sl-area")
      .getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    this.saturation = Math.round(x * 100);
    this.lightness = Math.round((1 - y) * 100);
  }

  updateHue(e) {
    const rect = this.shadowRoot
      .querySelector(".hue-bar")
      .getBoundingClientRect();
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    this.hue = Math.round(y * 360);
  }

  handlePresetClick(color) {
    const rgb = this.hexToRgb(color);
    if (rgb) {
      const hsl = this.rgbToHsl(rgb.r, rgb.g, rgb.b);
      this.hue = hsl.h;
      this.saturation = hsl.s;
      this.lightness = hsl.l;
    }
  }

  handleRgbInput(channel, e) {
    const value = Math.max(0, Math.min(255, parseInt(e.target.value) || 0));
    const rgb = this.currentRgb;
    rgb[channel] = value;
    const hsl = this.rgbToHsl(rgb.r, rgb.g, rgb.b);
    this.hue = hsl.h;
    this.saturation = hsl.s;
    this.lightness = hsl.l;
  }

  handleHexInput(e) {
    let hex = e.target.value;
    if (!hex.startsWith("#")) hex = "#" + hex;
    if (/^#[0-9a-fA-F]{6}$/.test(hex)) {
      const rgb = this.hexToRgb(hex);
      if (rgb) {
        const hsl = this.rgbToHsl(rgb.r, rgb.g, rgb.b);
        this.hue = hsl.h;
        this.saturation = hsl.s;
        this.lightness = hsl.l;
      }
    }
  }

  confirm() {
    this.dispatchEvent(
      new CustomEvent("color-confirm", {
        bubbles: true,
        composed: true,
        detail: { controlId: this.controlId, color: this.selectedColor },
      }),
    );
    this.open = false;
  }

  cancel() {
    this.dispatchEvent(
      new CustomEvent("color-cancel", {
        bubbles: true,
        composed: true,
        detail: { controlId: this.controlId },
      }),
    );
    this.open = false;
  }

  render() {
    const rgb = this.currentRgb;
    const hueColor = `hsl(${this.hue}, 100%, 50%)`;
    const selectedColor = this.selectedColor;

    return html`
      <link rel="stylesheet" href="//system.localhost:8888/color_picker.css" />
      <div class="backdrop" @click=${this.handleBackdropClick}></div>
      <div class="picker" style="left: ${this.x}px; top: ${this.y}px;">
        <div class="picker-main">
          <!-- Saturation/Lightness area -->
          <div
            class="sl-area"
            style="background: linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, ${hueColor});"
            @mousedown=${this.handleSLMouseDown}
          >
            <div
              class="sl-cursor"
              style="left: ${this.saturation}%; top: ${100 - this.lightness}%;"
            ></div>
          </div>

          <!-- Hue bar -->
          <div class="hue-bar" @mousedown=${this.handleHueMouseDown}>
            <div
              class="hue-cursor"
              style="top: ${(this.hue / 360) * 100}%;"
            ></div>
          </div>
        </div>

        <!-- Preview and inputs -->
        <div class="inputs-row">
          <div
            class="preview"
            style="background-color: ${selectedColor};"
            title="${selectedColor}"
          ></div>
          <div class="inputs">
            <div class="input-group">
              <label>R:</label>
              <input
                type="number"
                min="0"
                max="255"
                .value=${rgb.r}
                @input=${(e) => this.handleRgbInput("r", e)}
              />
            </div>
            <div class="input-group">
              <label>G:</label>
              <input
                type="number"
                min="0"
                max="255"
                .value=${rgb.g}
                @input=${(e) => this.handleRgbInput("g", e)}
              />
            </div>
            <div class="input-group">
              <label>B:</label>
              <input
                type="number"
                min="0"
                max="255"
                .value=${rgb.b}
                @input=${(e) => this.handleRgbInput("b", e)}
              />
            </div>
            <div class="input-group hex">
              <label>Hex:</label>
              <input
                type="text"
                .value=${selectedColor}
                @input=${this.handleHexInput}
              />
            </div>
          </div>
        </div>

        <!-- Presets -->
        <div class="presets">
          ${PRESETS.map(
            (color) => html`
              <div
                class="preset-swatch"
                style="background-color: ${color};"
                @click=${() => this.handlePresetClick(color)}
                title="${color}"
              ></div>
            `,
          )}
        </div>

        <!-- Buttons -->
        <div class="buttons">
          <button class="btn" @click=${this.cancel}>Cancel</button>
          <button class="btn primary" @click=${this.confirm}>OK</button>
        </div>
      </div>
    `;
  }
}

customElements.define("color-picker", ColorPicker);
