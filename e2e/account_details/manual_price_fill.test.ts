/**
 * E2E tests — Unupdated-Price Manual Fill (MKT-170–179)
 *
 * Spec:     docs/spec/market-price.md (MKT-170–179)
 * Contract: docs/contracts/asset-contract.md § "Asset Price Fetch Tasks",
 *           § "Asset Prices", § UnpricedAsset
 *
 * Spec rules exercised by this file:
 *   MKT-170/171 — completion signal carries the unpriced list
 *   MKT-172     — modal auto-opens when the unpriced list is non-empty
 *   MKT-173     — partial-result snackbar suppressed when modal opens
 *   MKT-174     — modal shows one row per unpriced asset (ticker, last-price slot)
 *   MKT-175     — entering a price and confirming records it via record_asset_price
 *   MKT-176     — skip leaves the asset stale and removes the row
 *   MKT-177     — when every row is resolved the modal closes automatically
 *   MKT-179     — after a manual fill AssetPriceUpdated causes Account Details
 *                 to re-fetch; the holding row's Current Price column reflects
 *                 the entered value
 *
 * Seed strategy:
 *   Two assets whose references (ZZ-NOPE-1 / ZZ-NOPE-2) cannot be resolved by
 *   Yahoo Finance. Both assets are intentionally seeded WITHOUT an exchange
 *   value so MKT-110 branch 2 applies (bare reference as symbol). Yahoo
 *   Finance returns no data for unknown symbols (online) or the request times
 *   out (offline); either way, both assets land in the MKT-114 skip set and
 *   appear in the unpriced list. The test outcome is therefore deterministic
 *   regardless of network state.
 *
 *   Both assets are bought in the same account so
 *   fetch_account_asset_prices scopes to them. Cash asset is excluded from
 *   fetch scope (MKT-116) and never appears in the unpriced list.
 *
 * Why one scenario (happy path + auto-close):
 *   The core cross-layer contract at E2E is:
 *     fetch dispatch → background job → completion event → modal → IPC record_asset_price → re-fetch
 *   A single scenario that traverses this full path (confirm one row, skip
 *   one row, assert modal auto-closes, assert holding now shows the entered
 *   price) covers the most critical integration points without duplicating
 *   Vitest-level coverage of per-row error paths and the hook state machine.
 *
 * Timing notes:
 *   fetch_account_asset_prices is fire-and-forget (returns () once dispatched).
 *   The background job calls Yahoo for each asset. With bogus symbols, Yahoo
 *   returns quickly (404 / no-data); per-request timeout is 10 s (yahoo_client.rs).
 *   The MODAL_APPEARS_TIMEOUT (35 s) is the outer ceiling for the event to fire
 *   and the modal to mount. In practice the job completes in < 5 s because the
 *   bogus requests fail fast. The long ceiling guards offline CI where the TCP
 *   handshake itself may time out (2 × 10 s + overhead).
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Timeout constants — explicit on every wait call (E2E rule E10)
// ---------------------------------------------------------------------------

/**
 * Upper bound for the unupdated-prices modal to auto-open after clicking
 * "Refresh prices". Covers two sequential Yahoo requests (10 s timeout each)
 * plus IPC + event propagation. In practice the job completes in < 5 s because
 * bogus symbols fail fast; 35 s guards the offline case.
 */
const MODAL_APPEARS_TIMEOUT = 35_000;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("manual_price_fill", () => {
  let accId: string;
  let astId1: string;
  let astId2: string;

  // Seed shared prerequisites once via IPC — no UI interaction needed for setup.
  // Two distinct assets with bogus Yahoo references + one account holding both.
  before(async () => {
    const catId = await seedCategory("E2E Cat MKT170");
    accId = await seedAccount("E2E MKT-170 Account");

    // seedAsset lets us pass an explicit reference (the "ticker"). We pass
    // ZZ-NOPE-1 / ZZ-NOPE-2 — strings guaranteed never to be real Yahoo
    // symbols. No exchange is set, so MKT-110 branch 2 resolves the symbol
    // to the bare reference, and Yahoo returns no data for both.
    astId1 = await seedAsset("E2E Unpriced Asset One", catId, { reference: "ZZ-NOPE-1" });
    astId2 = await seedAsset("E2E Unpriced Asset Two", catId, { reference: "ZZ-NOPE-2" });

    // Buy 10 units of each in the account so both appear as active holdings
    // and enter the fetch-account-prices scope (MKT-132). seedBuy internally
    // seeds the deposit needed to satisfy CSH-041.
    await seedBuy(accId, astId1, "2020-06-01", 10);
    await seedBuy(accId, astId2, "2020-06-02", 10);

    // Note: seedBuy internally deposits cash (CSH-041), so the account already
    // has a cash holding. System-cash assets are excluded from fetch scope
    // per MKT-116 and never appear in the unpriced list.
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // MKT-172/174/175/176/177/179 — full critical path:
  //   Refresh prices → modal auto-opens with both assets listed →
  //   enter a price for asset-1 and confirm → row disappears →
  //   skip asset-2 → row disappears → modal closes automatically →
  //   navigate to Account Details → assert asset-1's holding row shows the
  //   entered price in the Current Price column (MKT-179 reactivity).
  // -------------------------------------------------------------------------
  it("MKT-172/175/176/177/179: fetch → modal auto-opens → confirm one price → skip one → modal closes → holding reflects entered price", async () => {
    // -----------------------------------------------------------------------
    // Step 1 — Navigate to the account details page and trigger a price fetch.
    //   The "Refresh prices" button (id="account-details-refresh-prices") scopes
    //   the fetch to this account's holdings (MKT-131 / MKT-132).
    // -----------------------------------------------------------------------
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    const refreshBtn = await $("#account-details-refresh-prices");
    await refreshBtn.waitForExist({ timeout: 10_000 });
    await refreshBtn.click();

    // -----------------------------------------------------------------------
    // Step 2 — Wait for the unupdated-prices modal to auto-open (MKT-172).
    //   The modal mounts when the AssetPriceFetchCompleted event fires with a
    //   non-empty unpriced list. Both assets (ZZ-NOPE-1/2) are in the skip
    //   set (MKT-114/171) so the modal always opens.
    //   Use the first row's stable id as the mount signal (E4).
    // -----------------------------------------------------------------------
    const row1 = await $(`#unpriced-row-${astId1}`);
    await row1.waitForExist({ timeout: MODAL_APPEARS_TIMEOUT });

    const row2 = await $(`#unpriced-row-${astId2}`);
    await row2.waitForExist({ timeout: 8_000 });

    // MKT-174 — reference cells must be visible (ticker = "ZZ-NOPE-1" / "ZZ-NOPE-2")
    const ref1 = await $(`#unpriced-reference-${astId1}`);
    await ref1.waitForExist({ timeout: 5_000 });
    const ref1Text = await ref1.getText();
    assert.strictEqual(ref1Text, "ZZ-NOPE-1", "Reference cell must show the asset ticker");

    const ref2 = await $(`#unpriced-reference-${astId2}`);
    await ref2.waitForExist({ timeout: 5_000 });
    const ref2Text = await ref2.getText();
    assert.strictEqual(ref2Text, "ZZ-NOPE-2", "Reference cell must show the asset ticker");

    // MKT-173 — the partial-result snackbar must NOT appear alongside the modal.
    //   role="status" is the snackbar region. The dispatch snackbar ("Fetching
    //   prices…", MKT-115) is expected, so we assert only that the *outcome*
    //   (partial/failure) copy is absent. The substring is English because the
    //   suite runs under the `en` locale; MKT-173 suppression is also covered
    //   locale-invariantly by the store unit tests (src/lib/store.test.ts).
    const snackbarRegion = await $('[role="status"]');
    if (await snackbarRegion.isExisting()) {
      const snackText = await snackbarRegion.getText();
      assert.ok(
        !snackText.includes("couldn't be updated") && !snackText.includes("Couldn't update"),
        `MKT-173 — partial-result snackbar must not appear when the modal opens (got: "${snackText}")`,
      );
    }

    // -----------------------------------------------------------------------
    // Step 3 — Enter a price for asset-1 and click Confirm (MKT-175).
    //   The price input is a React controlled <input type="number"> (E6).
    //   Use setReactInputValue to trigger the React onChange. The Confirm
    //   button (id="unpriced-confirm-<id>") enables once the input is non-empty
    //   and parseable.
    // -----------------------------------------------------------------------

    await $(`#unpriced-price-input-${astId1}`).waitForExist({ timeout: 5_000 });
    await captureScreen("unpriced-prices-modal");
    await setReactInputValue(`unpriced-price-input-${astId1}`, "42.50");

    const confirmBtn1 = await $(`#unpriced-confirm-${astId1}`);
    await confirmBtn1.waitForEnabled({ timeout: 5_000 });
    await confirmBtn1.click();

    // MKT-178 — on success the row leaves the list (MKT-177 partial resolution).
    await row1.waitForExist({ timeout: 10_000, reverse: true });

    // -----------------------------------------------------------------------
    // Step 4 — Skip asset-2 (MKT-176).
    //   Nothing is recorded; the row leaves the list.
    // -----------------------------------------------------------------------
    const skipBtn2 = await $(`#unpriced-skip-${astId2}`);
    await skipBtn2.waitForExist({ timeout: 5_000 });
    await skipBtn2.click();

    // MKT-177 — skipping the last row means every row is resolved; the modal
    //   closes automatically.
    await row2.waitForExist({ timeout: 8_000, reverse: true });

    // Confirm the modal itself is gone (FormModal wraps in role="dialog").
    const dialog = await $('[role="dialog"]');
    await dialog.waitForExist({ timeout: 8_000, reverse: true });

    // -----------------------------------------------------------------------
    // Step 5 — Navigate to Account Details and assert asset-1's holding row
    //   now shows the entered price (MKT-179 — AssetPriceUpdated triggers a
    //   re-fetch; the Account Details view reflects the new value). The
    //   current-price cell carries a stable id (#holding-current-price-<assetId>),
    //   so the assertion is scoped to that cell rather than the whole page.
    // -----------------------------------------------------------------------
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // The view re-fetches on AssetPriceUpdated (MKT-036/179). Wait until asset-1's
    // current-price cell shows the newly entered price ("42.50" from 42.50 × 1e6
    // micros via microToFormattedPrice).
    await browser.waitUntil(
      async () => {
        const cell = await $(`#holding-current-price-${astId1}`);
        if (!(await cell.isExisting())) return false;
        return (await cell.getText()).includes("42.50");
      },
      {
        timeout: 12_000,
        timeoutMsg:
          'Account Details current-price cell for asset-1 must show "42.50" after a manual price fill via the unupdated-prices modal (MKT-179)',
      },
    );
  });
});
