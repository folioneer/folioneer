const AUTO_RECORD_PRICE_KEY = "auto_record_price";

/**
 * MKT-050 — Read the global auto-record-price preference from localStorage.
 * Returns false when the key is absent (default OFF).
 */
export function getAutoRecordPrice(): boolean {
  return localStorage.getItem(AUTO_RECORD_PRICE_KEY) === "true";
}

/**
 * MKT-050 — Persist the global auto-record-price preference to localStorage.
 */
export function setAutoRecordPrice(enabled: boolean): void {
  localStorage.setItem(AUTO_RECORD_PRICE_KEY, String(enabled));
}
