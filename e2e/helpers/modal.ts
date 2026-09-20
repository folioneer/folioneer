import { $ } from "@wdio/globals";

/**
 * Defensively close any leftover modal from a previous test.
 *
 * Use in beforeEach to guarantee a clean starting state for the next test.
 * The check-then-click split is wrapped in try/catch because a previous test's
 * confirmation dialog may unmount between isExisting() and click() — a race
 * that surfaces under truly headless xvfb (GTK rendering is slightly slower
 * than WSLg's passthrough display).
 */
export async function dismissLeftoverModal(): Promise<void> {
  const closeBtn = await $("#modal-close-btn");
  if (!(await closeBtn.isExisting())) return;
  try {
    await closeBtn.click();
    await closeBtn.waitForExist({ timeout: 3000, reverse: true });
  } catch {
    // Modal unmounted between check and click — that's fine, it's already gone.
  }
}
