const AUTO_FETCH_KEY = "auto_fetch_prices";

/**
 * MKT-120 — Read the global auto-fetch-prices preference from localStorage.
 * Returns false when the key is absent (default OFF).
 */
export function getAutoFetch(): boolean {
  return localStorage.getItem(AUTO_FETCH_KEY) === "true";
}

/**
 * MKT-120 — Persist the global auto-fetch-prices preference to localStorage.
 */
export function setAutoFetch(enabled: boolean): void {
  localStorage.setItem(AUTO_FETCH_KEY, String(enabled));
}
