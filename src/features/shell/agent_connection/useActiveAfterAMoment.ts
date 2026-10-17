import { useEffect, useState } from "react";

/** How long a dialog that just appeared keeps its approving action inactive, in milliseconds. */
export const ALLOW_AFTER_MS = 800;

/**
 * AGT-032 / AGT-083 / AGT-089 — whether a dialog's approving action may be used yet: not for
 * a moment after the dialog appears, so a click aimed at what was there before approves
 * nothing.
 */
export function useActiveAfterAMoment(): boolean {
  const [active, setActive] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setActive(true), ALLOW_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);
  return active;
}
