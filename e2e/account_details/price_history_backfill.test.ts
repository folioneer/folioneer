/**
 * E2E tests — Price history backfill (MKT-190–199)
 *
 * Spec:     docs/spec/market-price.md (MKT-190–199)
 * Contract: docs/contracts/asset-contract.md § "Price History Backfill"
 *
 * Spec rules exercised by this file:
 *   MKT-190 — the active holding row carries the backfill action
 *   MKT-196 — an asset whose ticker cannot be resolved is rejected
 *   MKT-197 — the rejection surfaces as a snackbar, and no price is written
 *
 * Seed strategy:
 *   The asset's reference ("ZZ NOPE 190") holds spaces, so no provider symbol can
 *   be derived (MKT-110) and the backend rejects before any network call: the
 *   outcome is the same online and offline. The path that writes rows needs the
 *   provider and is covered by src-tauri/tests/price_history_backfill_crud.rs.
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory } from "../helpers/seed";

/** Reads the asset's recorded prices through IPC, after the action ran. */
async function readAssetPrices(assetId: string): Promise<unknown[]> {
  const result = (await browser.executeAsync((astId: string, done: (r: unknown) => void) => {
    // @ts-expect-error __TAURI_INTERNALS__ injected by Tauri WebView
    window.__TAURI_INTERNALS__
      .invoke("get_asset_prices", { assetId: astId })
      .then(done)
      .catch((err: unknown) => done({ __error: String(err) }));
  }, assetId)) as unknown[] | { __error: string };
  assert.ok(Array.isArray(result), `get_asset_prices failed: ${JSON.stringify(result)}`);
  return result;
}

describe("price_history_backfill", () => {
  let accountId: string;
  let assetId: string;

  before(async () => {
    const categoryId = await seedCategory("E2E Cat MKT190");
    accountId = await seedAccount("E2E MKT-190 Account");
    assetId = await seedAsset("E2E Unresolvable Ticker", categoryId, {
      reference: "ZZ NOPE 190",
    });
    await seedBuy(accountId, assetId, "2020-07-01", 5);
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  it("MKT-190/196/197: backfilling an asset whose ticker cannot be resolved reports it and writes no price", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accountId);

    const backfillButton = await $(`#action-backfill-price-history-${assetId}`);
    await backfillButton.waitForClickable({ timeout: 10000 });
    await backfillButton.click();

    const snackbarRegion = await $('[role="status"]');
    await snackbarRegion.waitForExist({ timeout: 10000 });
    await browser.waitUntil(
      async () => (await snackbarRegion.getText()).includes("No price history found"),
      {
        timeout: 8000,
        timeoutMsg:
          'Expected the snackbar to contain "No price history found" (mkt.backfill.error.TickerNotResolved)',
      },
    );
    await captureScreen("price-history-backfill-unresolved-ticker");

    assert.deepStrictEqual(await readAssetPrices(assetId), [], "no price may be written");
  });
});
