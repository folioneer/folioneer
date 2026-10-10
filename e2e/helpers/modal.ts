import { $, browser } from "@wdio/globals";

/** Whether a modal's close button is on the page; `null` when the page changed under the lookup. */
async function closeButtonExists(): Promise<boolean | null> {
  try {
    return await (await $("#modal-close-btn")).isExisting();
  } catch {
    return null;
  }
}

/**
 * Defensively close any leftover modal from a previous test.
 *
 * Use in beforeEach to guarantee a clean starting state for the next test.
 * A modal may unmount at any step — while its close button is looked up, between
 * the lookup and the click, or during the click — and the driver then reports a
 * stale element. Every step is therefore taken on a fresh lookup, and a step the
 * page changed under is taken again until the page holds no close button.
 */
export async function dismissLeftoverModal(): Promise<void> {
  await browser
    .waitUntil(
      async () => {
        const exists = await closeButtonExists();
        if (exists !== true) return exists === false;
        try {
          await (await $("#modal-close-btn")).click();
        } catch {
          // The modal unmounted under the click: the next lookup says whether it is gone.
        }
        return false;
      },
      { timeout: 3000, interval: 100 },
    )
    .catch(() => {
      // A modal that stays is the next step's failure to report, with its own message.
    });
}
