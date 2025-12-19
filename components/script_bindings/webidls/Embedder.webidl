/* SPDX Id: AGPL-3.0-or-later */

// Servo-specific API for communication between web content and the embedder.

[Exposed=Window,
Func="Embedder::is_allowed_to_embed"]
interface Embedder : EventTarget {
    // Request the embedder to open a new OS-level window with the given URL.
    undefined openNewOSWindow(USVString url, optional USVString features = "");

    // Request the embedder to close the currently focused OS window.
    undefined closeCurrentOSWindow();

    // Request the embedder to exit the application.
    undefined exit();

    // Request the embedder to start a window drag operation.
    // This allows the window to be moved by dragging from web content.
    undefined startWindowDrag();

    // Request the embedder to start a window resize operation.
    // This allows the window to be resized from web content.
    undefined startWindowResize();

    // Event fired when Servo encounters an error.
    // The event is a CustomEvent with detail: { errorType: string, message: string }
    attribute EventHandler onservoerror;

    // Event fired when a preference changes.
    // The event is a CustomEvent with detail: { name: string, value: any }
    attribute EventHandler onpreferencechanged;
};

partial interface Navigator {
    [Func="Embedder::is_allowed_to_embed"]
    readonly attribute Embedder embedder;
};
