/* SPDX Id: AGPL-3.0-or-later */

[Exposed=Window,
Func="Embedder::is_allowed_to_embed"]
interface Peer : EventTarget {
    attribute DOMString displayName;
    readonly attribute DOMString id;

    // This peer left.
    attribute EventHandler onpeerleft;
};

[Exposed=Window,
Func="Embedder::is_allowed_to_embed"]
interface Pairing : EventTarget {
    // Our own endpoint.
    Promise<Peer> local();

    // The list of paired peers.
    Promise<sequence<Peer>> peers();

    // Start a pairing handshake with a discovered peer.
    Promise<boolean> requestPairing(Peer peer);

    // A new unpaired peer was discovered.
    attribute EventHandler onpeerdiscovered;

    // A paired peer joined.
    attribute EventHandler onpeerjoined;

    // A paired peer left.
    attribute EventHandler onpeerleft;

    // An unpaired peer is requesting pairing.
    attribute EventHandler onpairingrequest;
};

partial interface Embedder {
    [Func="Embedder::is_allowed_to_embed"]
    readonly attribute Pairing pairing;
};
