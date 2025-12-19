/* SPDX Id: AGPL-3.0-or-later */

// Servo-specific API for virtual keyboard input injection.
// This allows privileged pages (system.localhost, settings.localhost) to
// inject keyboard/composition events into webviews that have active IME input.

dictionary ServoKeyboardEventInit {
    required DOMString state;      // "down" or "up"
    required DOMString key;        // Key character or named key
    DOMString code = "";           // Physical key code
    unsigned long location = 0;    // KeyboardEvent.location value
    boolean shift = false;
    boolean ctrl = false;
    boolean alt = false;
    boolean meta = false;
    boolean repeat = false;
    boolean isComposing = false;
};

dictionary ServoCompositionEventInit {
    required DOMString state;      // "start", "update", or "end"
    DOMString data = "";           // The composition text
};

[Exposed=Window, Func="Embedder::is_allowed_to_embed"]
interface Keyboard {
    // Send a keyboard event (keydown/keyup) to the active IME input.
    [Throws] undefined sendKeyboardEvent(ServoKeyboardEventInit event);

    // Send a composition event to the active IME input.
    [Throws] undefined sendCompositionEvent(ServoCompositionEventInit event);
};

partial interface Navigator {
    [Func="Embedder::is_allowed_to_embed"]
    readonly attribute Keyboard keyboard;
};
