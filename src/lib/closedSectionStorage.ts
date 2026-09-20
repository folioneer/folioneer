const KEY_PREFIX = "closed_section_open_";

/**
 * Whether the closed-positions section is expanded for a given account.
 * Defaults to open (true) when the account has no remembered preference yet.
 */
export function getClosedSectionOpen(accountId: string): boolean {
  if (!accountId) return true;
  const stored = localStorage.getItem(`${KEY_PREFIX}${accountId}`);
  return stored === null ? true : stored === "true";
}

/**
 * Remember whether the closed-positions section is expanded for a given account,
 * so the next visit to that account restores the same fold state.
 */
export function setClosedSectionOpen(accountId: string, open: boolean): void {
  if (!accountId) return;
  localStorage.setItem(`${KEY_PREFIX}${accountId}`, String(open));
}
