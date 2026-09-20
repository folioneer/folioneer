/**
 * E2E tests — Free Share Distribution recording flow (FSD)
 *
 * Spec:     docs/spec/free-share-distribution.md
 * Contract: docs/contracts/account-contract.md § record_free_shares
 *
 * Spec rules covered by this file:
 *   FSD-010 — "Free shares" item in the header "Add" menu opens the modal
 *   FSD-020 — form fields: asset selector, date, quantity, note
 *   FSD-021 — frontend validation disables submit until quantity > 0 + date valid
 *   FSD-022 — recording increases the holding quantity (no cash movement)
 *   FSD-025 — modal closes and snackbar appears on success (observable via form gone)
 *   FSD-026 — Account Details re-fetches; holding row reflects the new quantity
 *   FSD-050 — TXL row for a free-share distribution shows "—" in the money columns
 *   FSD-051 — holding row shows the increased quantity after recording
 *   FSD-028 — deleting the distribution via TXL restores the holding (self-cleaning)
 *
 * Seed strategy:
 *   - category + asset (Stocks, non-cash) + account + buy 10 units seeded via IPC.
 *   - seedBuy internally seeds a deposit (CSH-041 pre-condition).
 *   - The free-shares modal's asset selector is a native <select> (SelectField),
 *     driven with selectByAttribute — no combobox automation needed.
 *   - The quantity field is a React controlled <input type="number"> driven with
 *     setReactInputValue (E2E rule E6).
 *
 * Why one scenario (recording + holding assertion):
 *   The TXL row-level assertion (FSD-050) and the self-cleaning delete (FSD-028)
 *   are blocked on missing stable ids (see Halt Artifacts). Once those ids are
 *   added the two stubs below become the completed single scenario.
 *   Error variants (AssetNotHeld, FreeSharesOnCashAsset, QuantityNotPositive) are
 *   adequately covered at the backend-integration and Vitest-frontend tiers.
 */

import assert from "node:assert";
import { $, $$, browser } from "@wdio/globals";
import { isoToDisplayDate } from "../helpers/date";
import { dismissLeftoverModal } from "../helpers/modal";
import {
  clickHeaderAction,
  navigateToAccountDetails,
  navigateToAccounts,
} from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Fixed past dates — one per write operation (E2E rule E9)
// ---------------------------------------------------------------------------
const DATES = {
  free_shares: isoToDisplayDate("2020-09-15"),
} as const;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("free_shares", () => {
  const ACCOUNT_NAME = "E2E Free Shares FSD-022";
  const ASSET_NAME = "E2E Asset FSD022";
  let astId: string;
  let accId: string;

  // Seed prerequisites once via IPC — no UI interaction for setup.
  // A 10-unit buy opens the position so the asset appears in the
  // free-shares modal's asset selector (FSD-011: quantity > 0).
  before(async () => {
    const catId = await seedCategory("E2E Cat FSD022");
    accId = await seedAccount(ACCOUNT_NAME);
    astId = await seedAsset(ASSET_NAME, catId);
    // seedBuy seeds a deposit on the prior day (CSH-041) then buys 10 units.
    await seedBuy(accId, astId, "2020-09-01", 10_000_000); // 10 units in micros
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // FSD-022/025/026/028/050/051 — full self-cleaning critical path:
  //   record 5 free shares → holding rises 10 → 15 → TXL row shows "—" money
  //   columns → delete the distribution → holding restored to 10.
  //
  // The scenario ends with the holding back at its seeded 10 units, so the
  // suite leaves no residue (E2E independence — the delete is the cleanup).
  // -------------------------------------------------------------------------
  it("FSD-022/050/028: record free shares, verify TXL, delete, holding restored", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // -------------------------------------------------------------------
    // Step 1 — Click the "Free shares" header action (DIV-012 → FSD-010).
    // -------------------------------------------------------------------
    await clickHeaderAction("add-menu-free-shares");

    // -------------------------------------------------------------------
    // Step 2 — Wait for the free-shares form (FSD-020).
    // -------------------------------------------------------------------
    const form = await $("form#free-shares-form");
    await form.waitForExist({ timeout: 8000 });

    // -------------------------------------------------------------------
    // Step 3 — Fill the form (FSD-020).
    //   asset selector: native <select> → selectByAttribute (locale-invariant).
    //   date: DateField (type="text") → setReactInputValue + isoToDisplayDate (E7).
    //   quantity: <input type="number"> → setReactInputValue (E6).
    //   note: optional, left empty.
    // -------------------------------------------------------------------
    const assetSelect = await $("#free-shares-asset-select");
    await assetSelect.waitForExist({ timeout: 5000 });
    await assetSelect.selectByAttribute("value", astId);

    await setReactInputValue("free-shares-date", DATES.free_shares);
    await setReactInputValue("free-shares-quantity", "5");

    // -------------------------------------------------------------------
    // Step 4 — Submit (FSD-025 in-flight + success path).
    //   waitForEnabled confirms React processed the input events (E6).
    // -------------------------------------------------------------------
    const submitBtn = await $('button[type="submit"][form="free-shares-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // -------------------------------------------------------------------
    // Step 5 — Assert post-conditions (FSD-025/026/051).
    // -------------------------------------------------------------------

    // FSD-025 — form must close on success.
    await form.waitForExist({ timeout: 8000, reverse: true });

    // FSD-026/051 — the holding quantity <td> must reflect 10 + 5 = 15 units
    // after the re-fetch triggered by TransactionUpdated. microToFormattedQuantity
    // renders integers without a separator, so "15" is locale-invariant here.
    const holdingQty = await $(`#holding-quantity-${astId}`);
    await holdingQty.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await holdingQty.getText()) === "15", {
      timeout: 8000,
      timeoutMsg: "Holding quantity must rise to 15 after recording 5 free shares (FSD-051)",
    });

    // The cash holding must be untouched (FSD-022d — no cash leg).
    // The cash row deposit button is the stable proxy for "cash holding present".
    const cashDepositBtn = await $("#action-record-deposit-system-cash-eur");
    await cashDepositBtn.waitForExist({ timeout: 8000 });
    assert.ok(
      await cashDepositBtn.isExisting(),
      "Cash holding must be untouched after free-share distribution (FSD-022d)",
    );

    // -------------------------------------------------------------------
    // Step 6 — Open the transaction list for this holding (FSD-050).
    //   The view-transactions Search button navigates to the TXL filtered
    //   by (accId, astId): the buy and the free-shares rows.
    // -------------------------------------------------------------------
    const viewTxBtn = await $(`#action-view-transactions-${astId}`);
    await viewTxBtn.waitForExist({ timeout: 8000 });
    await viewTxBtn.click();

    // The free-shares row is the only one whose total-amount cell renders the
    // "—" placeholder (FSD-050) — buys always carry a real total. Locate it by
    // that locale-invariant signal, then derive its row id so the per-cell
    // (#txl-{qty,unit-price,total}-<txId>) and delete (#txl-delete-<txId>)
    // selectors can be targeted by stable id (E4).
    let freeSharesRowId: string | null = null;
    await browser.waitUntil(
      async () => {
        const rows = await $$('[id^="txl-row-"]');
        for (const row of rows) {
          const totalCell = await row.$('[id^="txl-total-"]');
          if ((await totalCell.getText()) === "—") {
            freeSharesRowId = await row.getAttribute("id");
            return true;
          }
        }
        return false;
      },
      { timeout: 8000, timeoutMsg: "TXL must list the free-shares row (FSD-050)" },
    );
    await captureScreen("free-shares-transactions");
    assert.ok(freeSharesRowId, "Free-shares row must exist in the TXL (FSD-050)");

    // FSD-050 — quantity cell shows the 5 distributed shares; the unit-price and
    // total-amount money cells both show the "—" placeholder.
    const txId = (freeSharesRowId as string).replace("txl-row-", "");
    assert.strictEqual(
      await (await $(`#txl-qty-${txId}`)).getText(),
      "5.000",
      "FSD-050 — quantity cell shows the distributed shares",
    );
    assert.strictEqual(
      await (await $(`#txl-unit-price-${txId}`)).getText(),
      "—",
      "FSD-050 — unit-price cell is the neutral placeholder",
    );
    assert.strictEqual(
      await (await $(`#txl-total-${txId}`)).getText(),
      "—",
      "FSD-050 — total-amount cell is the neutral placeholder",
    );

    // -------------------------------------------------------------------
    // Step 7 — Delete the distribution via the TXL (FSD-028).
    // -------------------------------------------------------------------
    const freeSharesRow = await $(`#${freeSharesRowId}`);
    const deleteBtn = await $(`#txl-delete-${txId}`);
    await deleteBtn.waitForExist({ timeout: 5000 });
    await deleteBtn.click();

    const confirmBtn = await $("#txl-delete-confirm");
    await confirmBtn.waitForEnabled({ timeout: 5000 });
    await confirmBtn.click();

    // The deleted row must disappear from the TXL.
    await freeSharesRow.waitForExist({ timeout: 8000, reverse: true });

    // -------------------------------------------------------------------
    // Step 8 — Back on Account Details, the holding is restored to 10 units
    //   (FSD-028 — deleting a distribution reverses the quantity delta).
    // -------------------------------------------------------------------
    await navigateToAccounts();
    await navigateToAccountDetails(accId);
    const restoredQty = await $(`#holding-quantity-${astId}`);
    await restoredQty.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await restoredQty.getText()) === "10", {
      timeout: 8000,
      timeoutMsg: "Holding must return to 10 units after deleting the distribution (FSD-028)",
    });
  });
});
