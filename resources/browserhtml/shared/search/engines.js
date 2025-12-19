// SPDX-License-Identifier: AGPL-3.0-or-later

const PREF_NAME = "browserhtml.search_engine";

export default class SearchEngines extends EventTarget {
  constructor() {
    super();
    this.engines = null;
    this.currentEngine = null;
  }

  async ensureReady() {
    if (this.currentEngine) {
      return;
    }

    // Load the default search engines resource.
    try {
      let response = await fetch(
        "//shared.localhost:8888/search/default_engines.json",
      );
      this.engines = await response.json();
    } catch (e) {
      console.error(`Failed to load search engines: ${e}`);
      return;
    }

    // Check if the preference matches an existing engine. If not, reset to the
    // first engine.
    let prefEngine = navigator.servo.getStringPreference(PREF_NAME);
    console.log(`search engine in prefs: ${prefEngine}`);
    let engine = this.engines.find((engine) => {
      return engine.id == prefEngine;
    });

    if (!engine) {
      engine = this.engines[0];
      navigator.servo.setStringPreference(PREF_NAME, engine.id);
    }
    this.currentEngine = engine;

    console.log(`current engine: ${this.currentEngine.name}`);

    this.dispatchCurrentEngine();

    navigator.embedder.addEventListener(
      "preferencechanged",
      this.onPrefChange.bind(this),
    );
  }

  set(engineId) {
    if (!this.engines) {
      return;
    }

    let engine = this.engines.find((engine) => {
      return engine.id == engineId;
    });
    if (!engine) {
      console.error(`Unknown search engine: ${engineId}`);
      return;
    }
    this.currentEngine = engine;
    this.dispatchCurrentEngine();
  }

  onPrefChange(event) {
    if (event.detail.name !== PREF_NAME) {
      return;
    }
    this.set(event.detail.value);
  }

  dispatchCurrentEngine() {
    if (!this.currentEngine) {
      return;
    }

    let event = new CustomEvent("engine", { detail: this.currentEngine });
    this.dispatchEvent(event);
  }

  queryUrl(text) {
    if (!this.currentEngine) {
      return null;
    }

    return this.currentEngine["template"].replace(
      "{searchTerms}",
      encodeURIComponent(text),
    );
  }
}
