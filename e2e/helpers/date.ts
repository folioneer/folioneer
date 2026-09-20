/**
 * Converts ISO date to the DateField display format.
 *
 * The E2E suite forces `LANG=en_US.UTF-8` in wdio.conf.ts beforeSession
 * (so aria-labels and translated text resolve to English). DateField's
 * effectiveLocale therefore resolves to "en-US" → MM/DD/YYYY. (E2E rule E7)
 *
 * If the project ever stops forcing en-US in beforeSession, this helper
 * must be updated to match the new locale, OR DateField must be passed an
 * explicit `locale` prop in the affected modals. The helper and the runtime
 * locale must agree, otherwise dates are silently mis-parsed by
 * useDateField.formatDateForStorage and land on the wrong day.
 */
export function isoToDisplayDate(iso: string): string {
  const [year, month, day] = iso.split("-");
  return `${month}/${day}/${year}`; // "2020-01-15" → "01/15/2020" (en-US)
}
