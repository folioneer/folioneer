/**
 * Format an ISO date (YYYY-MM-DD) as a locale-numeric date — e.g. `14/06/2026`
 * for `fr`, `6/14/2026` for `en`. Anchored at noon so the rendered day never
 * shifts under a timezone offset. Returns the raw input unchanged if it does not
 * parse. Lives under `ui/format/` as a cross-feature primitive (not feature-owned).
 */
export function formatIsoDateNumeric(isoDate: string, locale: string): string {
  const date = new Date(`${isoDate}T12:00:00`);
  return Number.isNaN(date.getTime()) ? isoDate : new Intl.DateTimeFormat(locale).format(date);
}

/**
 * Format an ISO date-time (YYYY-MM-DDTHH:MM:SS) as a locale medium date +
 * short time — e.g. `12 juil. 2026, 19:00` for `fr`. Returns the raw input
 * unchanged if it does not parse.
 */
export function formatIsoDateTime(isoDateTime: string, locale: string): string {
  const date = new Date(isoDateTime);
  return Number.isNaN(date.getTime())
    ? isoDateTime
    : new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

/**
 * Format an ISO date-time (YYYY-MM-DDTHH:MM:SS) as a locale-numeric date followed by
 * the locale's short time — e.g. `19/09/2026 08:14` for `fr`, `9/19/2026 8:14 AM` for
 * `en`: the compact form for a moment shown beside other figures. Returns the raw input
 * unchanged if it does not parse.
 */
export function formatIsoDateTimeNumeric(isoDateTime: string, locale: string): string {
  const date = new Date(isoDateTime);
  if (Number.isNaN(date.getTime())) {
    return isoDateTime;
  }
  const day = new Intl.DateTimeFormat(locale).format(date);
  const time = new Intl.DateTimeFormat(locale, { timeStyle: "short" }).format(date);
  return `${day} ${time}`;
}
