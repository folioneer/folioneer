const KEY_PREFIX = "perf_view_mode_";
const GLOBAL_KEY = "global_perf_view_mode";

/** The performance page granularity toggle: monthly or yearly (PRF-013/014). */
export type StoredPerfViewMode = "month" | "year";

/**
 * The remembered performance-page view mode for a given account, or null when the
 * account has no stored preference yet (the caller then applies the PRF-014 default).
 */
export function getPerfViewMode(accountId: string): StoredPerfViewMode | null {
  if (!accountId) return null;
  const stored = localStorage.getItem(`${KEY_PREFIX}${accountId}`);
  return stored === "month" || stored === "year" ? stored : null;
}

/**
 * Remember the performance-page view mode for a given account, so the next visit to
 * that account's performance page restores the same granularity.
 */
export function setPerfViewMode(accountId: string, mode: StoredPerfViewMode): void {
  if (!accountId) return;
  localStorage.setItem(`${KEY_PREFIX}${accountId}`, mode);
}

/**
 * The remembered view mode for the global performance page, or null when no
 * preference is stored yet (the caller then applies the GPF-016 default).
 */
export function getGlobalPerfViewMode(): StoredPerfViewMode | null {
  const stored = localStorage.getItem(GLOBAL_KEY);
  return stored === "month" || stored === "year" ? stored : null;
}

/**
 * Remember the view mode for the global performance page, so the next visit
 * restores the same granularity.
 */
export function setGlobalPerfViewMode(mode: StoredPerfViewMode): void {
  localStorage.setItem(GLOBAL_KEY, mode);
}
