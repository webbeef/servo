// SPDX-License-Identifier: AGPL-3.0-or-later

/**
 * Simple URL detection - checks if a string looks like a URL
 * @param {string} str - The string to check
 * @returns {boolean} - True if it looks like a URL
 */
export function isUrl(str) {
  return (
    str.includes(".") &&
    !str.includes(" ") &&
    (str.startsWith("file://") ||
      str.startsWith("http://") ||
      str.startsWith("https://") ||
      /^[a-zA-Z0-9][-a-zA-Z0-9]*\.[a-zA-Z]{2,}/.test(str))
  );
}

/**
 * Normalize a URL by adding https:// if no protocol is present
 * @param {string} url - The URL to normalize
 * @returns {string} - The normalized URL
 */
export function normalizeUrl(url) {
  if (
    !url.startsWith("file://") &&
    !url.startsWith("http://") &&
    !url.startsWith("https://")
  ) {
    return "https://" + url;
  }
  return url;
}

/**
 * Create a debounced version of a function
 * @param {Function} fn - The function to debounce
 * @param {number} delay - The delay in milliseconds
 * @returns {Function} - The debounced function
 */
export function debounce(fn, delay) {
  let timeout;
  return (...args) => {
    clearTimeout(timeout);
    timeout = setTimeout(() => fn(...args), delay);
  };
}

/**
 * Group consecutive results by provider
 * @param {Array} results - The results to group
 * @returns {Array} - The grouped results
 */
export function groupResults(results) {
  const groups = [];
  let currentGroup = null;

  for (const result of results) {
    if (!currentGroup || currentGroup.provider !== result.provider) {
      currentGroup = {
        provider: result.provider,
        providerIcon: result.providerIcon,
        items: [],
      };
      groups.push(currentGroup);
    }
    currentGroup.items.push(result);
  }

  return groups;
}
