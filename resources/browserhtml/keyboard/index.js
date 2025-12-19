// SPDX-License-Identifier: AGPL-3.0-or-later

let shiftActive = false;
let inputType = "text";
let currentValue = "";

// Check if the keyboard API is available
const keyboardAvailable = typeof navigator.keyboard !== "undefined";
if (keyboardAvailable) {
  console.log("[Keyboard] navigator.keyboard API is available");
} else {
  console.warn(
    "[Keyboard] navigator.keyboard API is NOT available - virtual keyboard will not function",
  );
}

// Send a character to the active input via composition event
function sendCharacter(char) {
  if (!keyboardAvailable) {
    return;
  }
  try {
    navigator.keyboard.sendCompositionEvent({
      state: "end",
      data: char,
    });
  } catch (e) {
    console.error("[Keyboard] Failed to send character:", e);
  }
}

// Send a keyboard event (keydown + keyup) for special keys
function sendKeyEvent(key, code) {
  if (!keyboardAvailable) {
    return;
  }
  try {
    navigator.keyboard.sendKeyboardEvent({
      state: "down",
      key: key,
      code: code || key,
    });
    navigator.keyboard.sendKeyboardEvent({
      state: "up",
      key: key,
      code: code || key,
    });
  } catch (e) {
    console.error("[Keyboard] Failed to send key event:", e);
  }
}

// Listen for messages from parent to get input context
window.addEventListener("message", (event) => {
  if (event.data.type === "show") {
    inputType = event.data.inputType || "text";
    currentValue = event.data.currentValue || "";
    console.log("[Keyboard] Input context received:", {
      inputType,
      currentValue,
    });
    // TODO: Adapt keyboard layout based on inputType (e.g., number pad for "number")
  }
});

// Update key labels based on shift state
function updateKeyLabels() {
  document.querySelectorAll(".key[data-key]").forEach((key) => {
    const keyValue = key.dataset.key;
    if (keyValue.length === 1 && keyValue.match(/[a-z]/i)) {
      key.textContent = shiftActive
        ? keyValue.toUpperCase()
        : keyValue.toLowerCase();
    }
  });

  // Update shift button visual state
  document.querySelectorAll(".shift").forEach((btn) => {
    btn.classList.toggle("active", shiftActive);
  });
}

// Handle key clicks
document.querySelectorAll(".key").forEach((key) => {
  key.addEventListener("click", (e) => {
    e.preventDefault();

    const action = key.dataset.action;
    const keyValue = key.dataset.key;

    if (action === "shift") {
      shiftActive = !shiftActive;
      updateKeyLabels();
      console.log("[Keyboard] Shift toggled:", shiftActive);
    } else if (action === "backspace") {
      console.log("[Keyboard] Backspace pressed");
      sendKeyEvent("Backspace", "Backspace");
    } else if (action === "return") {
      console.log("[Keyboard] Return pressed");
      sendKeyEvent("Enter", "Enter");
    } else if (action === "symbols") {
      console.log("[Keyboard] Symbols mode requested");
      // TODO: Switch to symbols keyboard layout
    } else if (keyValue) {
      let charToSend = keyValue;
      if (shiftActive && keyValue.match(/[a-z]/i)) {
        charToSend = keyValue.toUpperCase();
        shiftActive = false; // Auto-release shift after typing
        updateKeyLabels();
      }
      console.log("[Keyboard] Key pressed:", charToSend);
      sendCharacter(charToSend);
    }
  });
});

// Initialize
updateKeyLabels();
