const KEY_PREFIX = "perf_period_";

/** The performance-column windows on account details: since start plus the five windowed returns (ACD-054). */
export const PERF_PERIODS = [
  "since_start",
  "ytd",
  "one_year",
  "two_years",
  "five_years",
  "ten_years",
] as const;

export type StoredPerfPeriod = (typeof PERF_PERIODS)[number];

/**
 * The remembered performance period for a given account, or null when the
 * account has no stored preference yet (the caller then applies the since-start default).
 */
export function getPerfPeriod(accountId: string): StoredPerfPeriod | null {
  if (!accountId) return null;
  const stored = localStorage.getItem(`${KEY_PREFIX}${accountId}`);
  return stored !== null && (PERF_PERIODS as readonly string[]).includes(stored)
    ? (stored as StoredPerfPeriod)
    : null;
}

/**
 * Remember the performance period for a given account, so the next visit to
 * that account's details page restores the same window.
 */
export function setPerfPeriod(accountId: string, period: StoredPerfPeriod): void {
  if (!accountId) return;
  localStorage.setItem(`${KEY_PREFIX}${accountId}`, period);
}
