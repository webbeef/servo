// SPDX-License-Identifier: AGPL-3.0-or-later

// Update top_1M.csv from https://github.com/zakird/crux-top-lists

const MAX_ENTRIES = 100000; // Limit to top 100K for performance
const MAX_RESULTS = 10; // Maximum results to return

export class TopSitesProvider {
  constructor() {
    this.name = "topsites";
    this.icon = "trophy";
    this.sites = []; // Array of {domain, url, rank}
    this.loaded = false;
    this.loading = null;
    this.load();
  }

  // Load and parse the CSV file
  async load() {
    if (this.loaded) {
      return;
    }
    if (this.loading) {
      return this.loading;
    }

    this.loading = (async () => {
      try {
        // Use absolute URL so it works from any origin (homescreen, system, etc.)
        const response = await fetch("//system.localhost:8888/providers/top_100K.txt");
        const text = await response.text();
        const lines = text.split("\n");

        // Skip header line
        for (let i = 1; i < lines.length && this.sites.length < MAX_ENTRIES; i++) {
          const line = lines[i].trim();
          if (!line) {
            continue;
          }

          const commaIndex = line.lastIndexOf(",");
          if (commaIndex === -1) {
            continue;
          }

          const url = line.substring(0, commaIndex);
          const rank = parseInt(line.substring(commaIndex + 1), 10);

          // Extract domain from URL
          try {
            const urlObj = new URL(url);
            const domain = urlObj.hostname;

            this.sites.push({
              domain: domain.toLowerCase(),
              url: url,
              rank: rank,
            });
          } catch {
            // Skip invalid URLs
          }
        }

        // Sort by rank (lower is better)
        this.sites.sort((a, b) => a.rank - b.rank);

        this.loaded = true;
        console.log(`TopSitesProvider loaded ${this.sites.length} sites`);
      } catch (e) {
        console.error("Failed to load top sites:", e);
      }
    })();

    return this.loading;
  }

  async query(text) {
    if (!text || text.trim() === "") {
      return [];
    }

    // Ensure data is loaded
    await this.load();

    const query = text.toLowerCase().trim();
    const results = [];

    for (const site of this.sites) {
      const domain = site.domain;

      // Check for prefix match (higher score)
      const prefixMatch = domain.startsWith(query) ||
                          domain.startsWith("www." + query) ||
                          domain.substring(domain.indexOf(".") + 1).startsWith(query);

      // Check for substring match
      const substringMatch = !prefixMatch && domain.includes(query);

      if (prefixMatch || substringMatch) {
        // Calculate score:
        // - Prefix match gets bonus of 0.5
        // - Popularity adds up to 0.5 based on rank (lower rank = higher score)
        const matchBonus = prefixMatch ? 0.5 : 0;
        const popularityScore = (1 - site.rank / 1000000) * 0.5;
        const score = matchBonus + popularityScore;

        results.push({
          score: score,
          kind: "link",
          value: {
            title: site.domain,
            url: site.url,
          },
        });

        // Stop early if we have enough results
        if (results.length >= MAX_RESULTS * 3) {
          break;
        }
      }
    }

    // Sort by score and limit results
    results.sort((a, b) => b.score - a.score);
    return results.slice(0, MAX_RESULTS);
  }
}
