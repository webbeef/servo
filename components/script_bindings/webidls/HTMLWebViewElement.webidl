/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://html.spec.whatwg.org/multipage/#htmliframeelement
[Exposed=Window]
interface HTMLWebViewElement : HTMLElement {
  [HTMLConstructor] constructor();

  [CEReactions]
           attribute USVString src;

  readonly attribute Document? contentDocument;
  readonly attribute WindowProxy? contentWindow;
};
