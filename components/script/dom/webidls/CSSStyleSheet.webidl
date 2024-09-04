/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

dictionary CSSStyleSheetInit {
  // TODO: support (MediaList or UTF8String) media = "";
  USVString media = "";
  boolean disabled = false;
  USVString baseURL;
};

// https://drafts.csswg.org/cssom/#the-cssstylesheet-interface
[Exposed=Window]
interface CSSStyleSheet : StyleSheet {
  [Throws] constructor(optional CSSStyleSheetInit options = {});

  // readonly attribute CSSRule? ownerRule;
  [Throws, SameObject] readonly attribute CSSRuleList cssRules;
  [Throws] unsigned long insertRule(DOMString rule, optional unsigned long index = 0);
  [Throws] undefined deleteRule(unsigned long index);

  [Throws] undefined replaceSync(USVString text);
};
