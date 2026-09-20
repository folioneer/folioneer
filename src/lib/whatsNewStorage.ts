const KEY = "whats_new_last_seen_version";

/**
 * The app version the What's-new dialog was last acknowledged for (WNW-010), or null
 * on a fresh install (the caller then seeds silently per WNW-030).
 */
export function getWhatsNewLastSeenVersion(): string | null {
  return localStorage.getItem(KEY);
}

/**
 * Remember the acknowledged app version so subsequent launches only surface
 * changelog sections newer than it.
 */
export function setWhatsNewLastSeenVersion(version: string): void {
  localStorage.setItem(KEY, version);
}
