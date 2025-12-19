// SPDX-License-Identifier: AGPL-3.0-or-later

export class FendProvider {
  constructor() {
    this.name = "Fend";
    this.icon = "pencil-ruler";
    this.initialized = false;
  }

  async init() {
    if (!this.initialized) {
      // Use absolute URL so it works from any origin (homescreen, system, etc.)
      await fend_wasm("//system.localhost:8888/third_party/fend/fend_wasm_bg.wasm");
      fend_wasm.initSync();
      this.initialized = true;
    }
  }

  // Returns a Promise that resolves to a result set.
  async query(text) {
    await this.init();

    let result = await fend_wasm.evaluateFendWithTimeout(text, 500);
    if (!result.startsWith("Error:") && result.length > 0) {
      return [{ kind: "text", value: result, score: 1 }];
    } else {
      return [];
    }
  }
}
