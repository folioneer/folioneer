/**
 * E2E tests — Stock Split recording flow (SPL)
 *
 * Spec:     docs/spec/stock-split.md
 * Contract: docs/contracts/account-contract.md § record_split
 *
 * Spec rules covered by this file:
 *   SPL-061 — "Split" holding-row action opens the split modal (ratio pair, date)
 *   SPL-010 — recording writes a Split transaction on the held position
 *   SPL-020 — the position rescales value-neutrally: quantity ×2, average ÷2
 *   SPL-060 — the journal row shows the "×N" ratio label and "—" money columns
 *
 * Seed strategy:
 *   - category + asset (Stocks, non-cash) + account + buy 10 units @ 100 via IPC.
 *   - seedBuy internally seeds a deposit (CSH-041 pre-condition).
 *   - No asset price is seeded, so the SPL-040 "Record post-split price"
 *     checkbox starts unchecked and the flow records no price.
 *   - The ratio fields are React controlled <input type="number"> driven with
 *     setReactInputValue (E2E rule E6); the date is a DateField driven with
 *     the locale display format (E7).
 *
 * Why one scenario:
 *   The critical path (record → rescaled holding → journal rendering) exercises
 *   the full UI → IPC → replay stack. Error variants (SplitFactorIsOne,
 *   SplitOnCashAsset, SplitCollapsesPosition, ClosedPosition) are covered at
 *   the backend-integration and Vitest-frontend tiers.
 */

import assert from "node:assert";
import { $, $$, browser } from "@wdio/globals";
import { isoToDisplayDate } from "../helpers/date";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Fixed past dates — one per write operation (E2E rule E9)
// ---------------------------------------------------------------------------
const DATES = {
  split: isoToDisplayDate("2020-10-01"),
} as const;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("split", () => {
  const ACCOUNT_NAME = "E2E Split SPL-020";
  const ASSET_NAME = "E2E Asset SPL020";
  let astId: string;
  let accId: string;

  // Seed prerequisites once via IPC — no UI interaction for setup.
  // A 10-unit buy @ 100 opens the position the split rescales (SPL-012:
  // quantity > 0 at the split date).
  before(async () => {
    const catId = await seedCategory("E2E Cat SPL020");
    accId = await seedAccount(ACCOUNT_NAME);
    astId = await seedAsset(ASSET_NAME, catId);
    // seedBuy seeds a deposit on the prior day (CSH-041) then buys 10 units @ 100.
    await seedBuy(accId, astId, "2020-09-01", 10_000_000); // 10 units in micros
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // SPL-061/010/020/060 — critical path:
  //   open the split modal from the holding row → record a 2 : 1 split →
  //   holding quantity 10 → 20, average price 100 → 50 → journal shows the
  //   "×2" Split row with "—" money columns.
  // -------------------------------------------------------------------------
  it("SPL-061/020/060: record a 2:1 split, holding doubles, average halves, journal shows ×2", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // -------------------------------------------------------------------
    // Step 1 — Open the split modal from the holding-row action (SPL-061).
    // -------------------------------------------------------------------
    const splitBtn = await $(`#action-split-${astId}`);
    await splitBtn.waitForExist({ timeout: 8000 });
    await splitBtn.click();

    // -------------------------------------------------------------------
    // Step 2 — Wait for the split form (SPL-061).
    // -------------------------------------------------------------------
    const form = await $("form#split-trx-form");
    await form.waitForExist({ timeout: 8000 });

    // -------------------------------------------------------------------
    // Step 3 — Fill the form.
    //   date: DateField (type="text") → setReactInputValue + isoToDisplayDate (E7).
    //   ratio: two <input type="number"> → setReactInputValue (E6); 2 : 1 is
    //   the default, set explicitly so the scenario never depends on it.
    //   note: optional, left empty. No price is seeded, so the SPL-040
    //   checkbox is unchecked and no price field is shown.
    // -------------------------------------------------------------------
    await setReactInputValue("split-trx-date", DATES.split);
    await setReactInputValue("split-trx-ratio-new", "2");
    await setReactInputValue("split-trx-ratio-old", "1");

    // -------------------------------------------------------------------
    // Step 4 — Submit (SPL-061 valid-ratio path).
    //   waitForEnabled confirms React processed the input events (E6).
    // -------------------------------------------------------------------
    const submitBtn = await $('button[type="submit"][form="split-trx-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // The form must close on success.
    await form.waitForExist({ timeout: 8000, reverse: true });

    // -------------------------------------------------------------------
    // Step 5 — Holding rescaled (SPL-020): quantity 10 → 20, average 100 → 50.
    //   microToFormattedQuantity renders integers without a separator, so
    //   "20" is locale-invariant; the average renders with 2 decimals in the
    //   forced en-US locale → "50.00".
    // -------------------------------------------------------------------
    const holdingQty = await $(`#holding-quantity-${astId}`);
    await holdingQty.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await holdingQty.getText()) === "20", {
      timeout: 8000,
      timeoutMsg: "Holding quantity must double to 20 after the 2:1 split (SPL-020)",
    });

    const holdingAvg = await $(`#holding-avg-price-${astId}`);
    await holdingAvg.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await holdingAvg.getText()) === "50.00", {
      timeout: 8000,
      timeoutMsg: "Average price must halve to 50.00 after the 2:1 split (SPL-020)",
    });
    await captureScreen("account-details-split");

    // -------------------------------------------------------------------
    // Step 6 — Journal rendering (SPL-060): open the per-asset transaction
    //   list; the split row's quantity cell carries the "×2" ratio label and
    //   its unit-price / total-amount cells render the "—" placeholder.
    // -------------------------------------------------------------------
    const viewTxBtn = await $(`#action-view-transactions-${astId}`);
    await viewTxBtn.waitForExist({ timeout: 8000 });
    await viewTxBtn.click();

    // The split row is the only one whose quantity cell renders the "×"
    // ratio label (buys carry plain numbers) — locate it by that
    // locale-invariant signal, then derive its row id so the per-cell
    // selectors can be targeted by stable id (E4).
    let splitRowId: string | null = null;
    await browser.waitUntil(
      async () => {
        const rows = await $$('[id^="txl-row-"]');
        for (const row of rows) {
          const qtyCell = await row.$('[id^="txl-qty-"]');
          if ((await qtyCell.getText()) === "×2") {
            splitRowId = await row.getAttribute("id");
            return true;
          }
        }
        return false;
      },
      { timeout: 8000, timeoutMsg: "TXL must list the ×2 split row (SPL-060)" },
    );
    assert.ok(splitRowId, "Split row must exist in the TXL (SPL-060)");

    // SPL-060 — the money cells both show the "—" placeholder.
    const txId = (splitRowId as string).replace("txl-row-", "");
    assert.strictEqual(
      await (await $(`#txl-unit-price-${txId}`)).getText(),
      "—",
      "SPL-060 — unit-price cell is the neutral placeholder",
    );
    assert.strictEqual(
      await (await $(`#txl-total-${txId}`)).getText(),
      "—",
      "SPL-060 — total-amount cell is the neutral placeholder",
    );
  });
});
