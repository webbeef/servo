// SPDX-License-Identifier: AGPL-3.0-or-later

export class ResultsAggregator {
  constructor(providers = []) {
    this.providers = providers;
  }

  // Add a provider to the list
  addProvider(provider) {
    this.providers.push(provider);
  }

  // Remove a provider by name
  removeProvider(name) {
    this.providers = this.providers.filter((p) => p.name !== name);
  }

  // Query all providers in parallel and return sorted results
  async query(text) {
    const promises = this.providers.map(async (provider) => {
      try {
        const results = await provider.query(text);
        // Tag results with provider name and icon
        return results.map((r) => ({
          ...r,
          provider: provider.name,
          providerIcon: provider.icon,
        }));
      } catch (e) {
        console.error(`Provider ${provider.name} error:`, e);
        return [];
      }
    });

    const resultsArrays = await Promise.all(promises);
    const allResults = resultsArrays.flat();

    // Sort by score (descending), defaulting to 0 if not set
    allResults.sort((a, b) => (b.score || 0) - (a.score || 0));

    return allResults;
  }
}
