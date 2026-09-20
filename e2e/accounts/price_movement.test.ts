/**
 * E2E tests — Price Movement (PMV)
 *
 * Spec:     docs/spec/price-movement.md (PMV-010–061)
 * Contract: docs/contracts/asset-contract.md § "Asset Price Fetch Tasks",
 *           § Shared Types PriceMovementReport / PriceMovementRow,
 *           § Events AssetPriceFetchCompleted
 *
 * Spec rules exercised by this file:
 *   PMV-010/015 — a Global refresh (trigger="Manual") is the only fetch path
 *                 that produces a report; the dialog appearing at all proves
 *                 the trigger reached the backend and the report round-tripped
 *                 through the real event bus, not a mock.
 *   PMV-013     — the report is presented as its own dialog on the accounts
 *                 list.
 *   PMV-018     — the same refresh leaves the asset unpriced, so the
 *                 unupdated-prices modal (MKT-172) opens too. The report waits
 *                 for it: no report dialog while that modal is up, and the
 *                 report is not lost — it opens once the modal clears.
 *   PMV-032/060 — an account whose only holding could never be priced is
 *                 marked incomplete; when that leaves nothing moved in the
 *                 whole portfolio the report states that plainly, together
 *                 with the incompleteness sentence, instead of an empty table.
 *   PMV-061     — dismissing the dialog removes it and it does not return.
 *
 * Seed strategy (determinism):
 *   Each E2E spec file runs in its own session against a fresh ephemeral DB
 *   (wdio.conf.ts beforeSession/afterSession — new FOLIONEER_E2E_DATA_DIR
 *   per worker), so no other test file's accounts are present when this
 *   file's Global refresh runs; the report's rows are exactly what this file
 *   seeds.
 *
 *   One account holding one asset whose reference (ZZ-NOPE-PMV) Yahoo Finance
 *   cannot resolve (no exchange set, so MKT-110 branch 2 uses the bare
 *   reference as the symbol — the same technique as
 *   account_details/manual_price_fill.test.ts). Yahoo returns no data for it
 *   either online or offline (offline: the request times out), so the asset
 *   always lands in the MKT-114 skip set and the account's reading is always
 *   marked incomplete (PMV-032), regardless of network state. The holding has
 *   never had a price recorded, so both the "before" and "after" readings
 *   value it at 0 (a missing price contributes 0) and the account's value
 *   cannot move — "nothing moved" (PMV-060) is therefore the deterministic
 *   report state, online or offline.
 *
 *   That same unresolvable asset is what puts the unupdated-prices modal on
 *   screen (MKT-170), which is what makes the PMV-018 ordering observable here
 *   rather than needing a second scenario to provoke it.
 *
 * Why one scenario:
 *   The core cross-layer contract at E2E is:
 *     click Global refresh → trigger="Manual" reaches Rust → real
 *     capture/fetch/report pipeline → AssetPriceFetchCompleted carries a real
 *     PriceMovementReport → the report waits for the manual-fill modal → the
 *     dialog renders it → dismiss discards it for good.
 *   A single scenario that traverses this full path covers the critical
 *   integration points without duplicating the existing Vitest-level coverage
 *   of the PMV rules (18 dialog-rendering tests, 8 hook tests, 5 manager
 *   wiring tests) or the Rust resolution matrix (1263 lib tests).
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccounts, navigateToAssets } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Timeout constants — explicit on every wait call (E2E rule E10)
// ---------------------------------------------------------------------------

/**
 * Upper bound for the fetch to complete after clicking "Refresh prices".
 * Mirrors MODAL_APPEARS_TIMEOUT in manual_price_fill.test.ts: the fetch task
 * calls Yahoo once for the single bogus symbol (10 s per-request timeout,
 * yahoo_client.rs) plus IPC + event propagation plus the "before"/"after"
 * valuation reads. In practice this completes in a few seconds because the
 * bogus symbol fails fast; the wide ceiling guards offline CI where the TCP
 * handshake itself may time out.
 */
const FETCH_COMPLETES_TIMEOUT = 35_000;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("price_movement", () => {
  let accountId: string;
  let assetId: string;

  // Seed shared prerequisites once via IPC — no UI interaction needed for
  // setup (seed in before(), never inside it() blocks).
  before(async () => {
    const catId = await seedCategory("E2E Cat PMV");
    accountId = await seedAccount("E2E PMV Account");

    // Bogus reference guarantees Yahoo can never resolve it (see file header).
    assetId = await seedAsset("E2E PMV Unresolvable Asset", catId, {
      reference: "ZZ-NOPE-PMV",
    });

    // Buy 10 units so the asset is an active holding in the Global refresh's
    // scope (MKT-130). seedBuy also seeds the deposit CSH-041 requires, so the
    // account carries a cash holding too — system cash is excluded from fetch
    // scope (MKT-116) and never contributes to incompleteness (PMV-032).
    await seedBuy(accountId, assetId, "2020-06-01", 10);
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
    await navigateToAccounts();
  });

  it("PMV-010/013/018/032/060/061: Global refresh reports nothing moved, marks it incomplete, waits for the unpriced-prices modal, and dismisses for good", async () => {
    // -----------------------------------------------------------------------
    // Step 1 — Trigger the Global refresh (PMV-010/015: trigger="Manual").
    //   The button already exists on the accounts list header.
    // -----------------------------------------------------------------------
    const refreshBtn = await $("#account-manager-refresh-prices");
    await refreshBtn.waitForExist({ timeout: 10_000 });
    await refreshBtn.click();

    // -----------------------------------------------------------------------
    // Step 2 — PMV-018: the same fetch left the asset unpriced, so the
    //   manual-fill modal (MKT-172) opens. The report must not be on screen
    //   while that modal is asking the user for something.
    // -----------------------------------------------------------------------
    const unpricedRow = await $(`#unpriced-row-${assetId}`);
    await unpricedRow.waitForExist({ timeout: FETCH_COMPLETES_TIMEOUT });

    // This check is not vacuous, and the reason is load-bearing: the unpriced
    // list and the report ride the SAME AssetPriceFetchCompleted event, on two
    // listeners that run in one callstack and commit in one React render. So by
    // the time the row is in the DOM the report has already reached the hook —
    // an ungated dialog would be on screen right now. Move either listener off
    // that synchronous path (an extra await, a debounce, a lazy import) and the
    // two land in separate renders; this line would still pass, but only
    // because the report had not arrived yet. Re-check it if you touch either
    // subscription.
    assert.ok(
      !(await $("#price-movement-dialog").isExisting()),
      "Report dialog must not stack over the unupdated-prices modal",
    );

    // Skip the only row — MKT-176/177 closes the modal automatically once
    // every row is resolved.
    const skipBtn = await $(`#unpriced-skip-${assetId}`);
    await skipBtn.waitForExist({ timeout: 5_000 });
    await skipBtn.click();
    await unpricedRow.waitForExist({ timeout: 8_000, reverse: true });

    // -----------------------------------------------------------------------
    // Step 3 — PMV-013/018: with the modal gone the report opens on its own.
    //   Its appearance proves the real round trip: the trigger reached Rust,
    //   the fetch task ran, AssetPriceFetchCompleted carried a real
    //   PriceMovementReport, and it survived the wait rather than being
    //   dropped — no mocking at any layer.
    // -----------------------------------------------------------------------
    const dialog = await $("#price-movement-dialog");
    await dialog.waitForExist({ timeout: 8_000 });

    // PMV-060 — nothing moved (the only holding never had a price, so both
    //   readings value it at 0); PMV-032 — that account is incomplete, so the
    //   report states both in one sentence rather than an empty/zeroed table.
    const dialogText = await dialog.getText();
    assert.ok(
      dialogText.includes("No account's value changed") &&
        dialogText.includes("could not be read at their current price"),
      `Report must state both that nothing moved and that a holding was incomplete (got: "${dialogText}")`,
    );

    // No table is rendered in the nothing-moved state — the per-account row
    // and total row must be absent (distinguishes this from the Moved state).
    assert.ok(
      !(await $(`#price-movement-row-${accountId}`).isExisting()),
      "Nothing-moved state must not render a per-account row",
    );
    assert.ok(
      !(await $("#price-movement-total").isExisting()),
      "Nothing-moved state must not render a total row",
    );

    // -----------------------------------------------------------------------
    // Step 4 — PMV-061: dismiss the report; it is gone for good.
    // -----------------------------------------------------------------------
    await captureScreen("price-movement-dialog");
    const dismissBtn = await $("#price-movement-dismiss");
    await dismissBtn.waitForExist({ timeout: 5_000 });
    await dismissBtn.click();
    await dialog.waitForExist({ timeout: 5_000, reverse: true });

    // Navigating away and back must not resurrect it — the report is held in
    // mount-scoped state with no persistence and no restore path (PMV-061).
    await navigateToAssets();
    await navigateToAccounts();
    assert.ok(
      !(await $("#price-movement-dialog").isExisting()),
      "Dismissed report must not reappear after navigating away and back",
    );
  });
});
