/**
 * Generic staleness formatting: how old is an ISO date relative to `today`,
 * expressed as an i18n descriptor. Domain-free — it knows nothing about what the
 * date represents (price, FX rate, anything); callers supply the i18n key pair.
 * Lives under `ui/format/` per F28 because it's a cross-feature primitive.
 */

/** i18n key + optional day-count interpolation for a staleness label. */
export type StalenessLabel = { key: string; params?: { days: number } };

/** The i18n key pair a staleness label resolves to: same-day vs N-days-old. */
export type StalenessKeys = { today: string; daysAgo: string };

/**
 * Whole-day delta between an ISO date (`YYYY-MM-DD`) and `today`.
 * `null` for a null or unparseable date; `0` for same-day (or future), `N` for
 * N calendar days in the past.
 */
export function computeDayDelta(isoDate: string | null, today: Date): number | null {
  if (isoDate === null) return null;
  const observed = new Date(`${isoDate}T00:00:00`);
  if (Number.isNaN(observed.getTime())) return null;
  const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const millisPerDay = 24 * 60 * 60 * 1000;
  return Math.floor((startOfToday.getTime() - observed.getTime()) / millisPerDay);
}

/**
 * i18n descriptor for how stale `isoDate` is relative to `today`, using the
 * caller-supplied key pair. `null` when the date is null/unparseable; the
 * `today` key when same-day or future; otherwise the `daysAgo` key carrying the
 * whole-day delta. The caller renders via `t(label.key, label.params)`.
 */
export function formatStalenessLabel(
  isoDate: string | null,
  today: Date,
  keys: StalenessKeys,
): StalenessLabel | null {
  const dayDelta = computeDayDelta(isoDate, today);
  if (dayDelta === null) return null;
  if (dayDelta <= 0) return { key: keys.today };
  return { key: keys.daysAgo, params: { days: dayDelta } };
}
