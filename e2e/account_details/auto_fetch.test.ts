/**
 * E2E tests — MKT auto-fetch surface (PR 3 of 3: E2E + closure)
 *
 * Contract: docs/contracts/asset-contract.md § "Asset Price Fetch Tasks"
 *
 * Spec rules covered:
 *   MKT-111 + MKT-131 — AccountDetailsView "Refresh prices" on cash-only account → NoFetchableHoldings snackbar
 *   MKT-120            — Settings auto-fetch toggle persists value to localStorage
 *   MKT-141 + MKT-142  — After IPC-seeding a Manual price, HoldingRow shows MANUAL badge; PriceHistoryModal shows Source column with "Manual"
 *
 * Deliberately NOT covered at this layer (and why):
 *
 *   MKT-130 (AccountManager global refresh — NoFetchableHoldings path) —
 *     `fetch_all_asset_prices` scans ALL accounts in the shared DB for non-cash
 *     active holdings. `derive_yahoo_symbol` returns Some() for any non-empty ASCII
 *     reference, so any buy seeded by other test files (buy_sell.test.ts etc.) makes
 *     the global use case dispatch rather than reject. This scenario therefore cannot
 *     be written in order-independent fashion without controlling the entire DB.
 *     The MKT-130 button is tested via MKT-131's path (same snackbar key, same hook
 *     contract — just different IPC command). The button's wiring to the hook is
 *     covered by FE unit tests (useRefreshGlobalPrices.test.ts).
 *
 *   MKT-113 (FetchAlreadyRunning) — Requires two concurrent clicks with reliable
 *     sub-second timing across the IPC boundary. Not reproducible deterministically
 *     in WebKitGTK; covered by FE hook unit tests (useRefreshGlobalPrices.test.ts,
 *     useRefreshAccountPrices.test.ts).
 *
 *   MKT-122 / fetch dispatch on a populated account — Would hit real Yahoo HTTP
 *     and write a Yahoo price as a side effect, polluting the DB across tests.
 *     The "dispatched" snackbar path (mkt.fetch_dispatched) is covered by FE unit
 *     tests. The Yahoo HTTP client itself is covered by BE integration tests in
 *     src-tauri/tests/.
 *
 *   MKT-132 (AccountNotFound) — The UI button always passes a valid accountId from
 *     the router URL param; the error is not reachable via normal UI interaction.
 *     Covered by BE Tier 2/3 tests.
 *
 *   MKT-121 (auto-fetch on launch) — Requires controlling whether Yahoo responds,
 *     which is a network dependency. The mount-once effect is covered by FE unit tests.
 *
 *   MKT-116 (cash asset exclusion) — The cash row is excluded inside the BE use case
 *     before any HTTP call; verified by BE Tier 3 integration tests.
 *
 * Seed strategy:
 *   - MKT-131: account + deposit seeded via IPC; cash holding is excluded from fetch
 *     scope (MKT-116), so fetch_account_asset_prices returns NoFetchableHoldings.
 *     Zero network calls.
 *   - MKT-141/142: account + asset + buy seeded via IPC; asset price seeded via IPC
 *     (source=Manual, MKT-101). UI navigates to AccountDetailsView and opens
 *     PriceHistoryModal to assert the source column.
 *   - MKT-120: pure localStorage toggle; no IPC calls needed.
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import {
  seedAccount,
  seedAsset,
  seedAssetPrice,
  seedBuy,
  seedCategory,
  seedDeposit,
} from "../helpers/seed";

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("MKT auto-fetch", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // MKT-131 + MKT-111
  // AccountDetailsView "Refresh prices" on a cash-only account surfaces the
  // NoFetchableHoldings snackbar. Cash assets are excluded from fetch scope
  // (MKT-116), so with only a cash holding the use case rejects immediately.
  //
  // Keyless (ADR-017): the fetch is dispatched directly with no API key and no
  // Connections gate, so the no-holdings path is reached without any key setup.
  // The dispatch makes zero network calls — the use case rejects with
  // NoFetchableHoldings before any symbol is fetched. This is the keyless-dispatch
  // E2E proof: the refresh button → backend → snackbar path runs key-free.
  // -------------------------------------------------------------------------
  it("MKT-131+MKT-111: AccountDetailsView Refresh prices on cash-only account shows no-holdings snackbar", async () => {
    const ACCOUNT_NAME = "E2E MKT-131 Cash Only";
    const accountId = await seedAccount(ACCOUNT_NAME);
    // Seed a cash deposit so the account has a cash holding (the cash asset
    // is system-managed; a deposit creates the holding row). The fetch use
    // case filters system-cash-* assets before symbol derivation (MKT-116),
    // leaving zero fetchable holdings → NoFetchableHoldings.
    await seedDeposit(accountId, "2019-01-10", 500_000_000); // 500 EUR

    await navigateToAccounts();
    await navigateToAccountDetails(accountId);

    const refreshBtn = await $("#account-details-refresh-prices");
    await refreshBtn.waitForExist({ timeout: 10000 });
    await refreshBtn.click();

    const snackbarRegion = await $('[role="status"]');
    await snackbarRegion.waitForExist({ timeout: 10000 });

    await browser.waitUntil(
      async () => {
        const text = await snackbarRegion.getText();
        return text.includes("No holdings");
      },
      {
        timeout: 8000,
        timeoutMsg:
          'Expected snackbar to contain "No holdings" (mkt.fetch_no_holdings) after clicking Refresh prices on a cash-only account',
      },
    );
    await captureScreen("account-details-refresh");
  });

  // -------------------------------------------------------------------------
  // MKT-120
  // Settings auto-fetch toggle persists the chosen value to localStorage.
  // This is a pure UI + localStorage test — no IPC calls involved.
  // -------------------------------------------------------------------------
  it("MKT-120: Settings auto-fetch toggle persists to localStorage", async () => {
    // Navigate to Settings via the sidebar button (id="nav-settings").
    const settingsNav = await $("#nav-settings");
    await settingsNav.waitForExist({ timeout: 15000 });
    await settingsNav.click();

    const checkbox = await $("#settings-auto-fetch");
    await checkbox.waitForExist({ timeout: 10000 });

    // Read the current persisted value before toggling.
    const initialValue = await browser.execute(() => localStorage.getItem("auto_fetch_prices"));

    // Click the toggle once — this calls toggleAutoFetch() which calls setAutoFetch().
    await checkbox.click();

    // Verify localStorage was updated to the opposite value.
    const afterFirstToggle = await browser.execute(() => localStorage.getItem("auto_fetch_prices"));
    // The toggle flips the boolean; after one click the value must be the inverse.
    const expectedAfterFirst = initialValue === "true" ? "false" : "true";
    assert.strictEqual(
      afterFirstToggle,
      expectedAfterFirst,
      `localStorage["auto_fetch_prices"] should be "${expectedAfterFirst}" after first toggle — got: "${afterFirstToggle}"`,
    );

    // Click again to restore original state (keeps the DB clean for other runs).
    await checkbox.click();
    const afterSecondToggle = await browser.execute(() =>
      localStorage.getItem("auto_fetch_prices"),
    );
    assert.strictEqual(
      afterSecondToggle,
      initialValue ?? "false",
      `localStorage["auto_fetch_prices"] should be restored to "${initialValue ?? "false"}" after second toggle — got: "${afterSecondToggle}"`,
    );
  });

  // -------------------------------------------------------------------------
  // MKT-141 + MKT-142
  // After seeding a manual price via IPC (source=Manual), the HoldingRow
  // Current Price cell shows the MANUAL source badge, and the PriceHistoryModal
  // shows the Source column with the "Manual" label.
  //
  // No fetch is invoked; we seed via record_asset_price which hardcodes
  // source=Manual (MKT-101). This exercises the full presenter → DOM path
  // without touching the network.
  // -------------------------------------------------------------------------
  it("MKT-141+MKT-142: Manual-seeded price shows MANUAL badge in HoldingRow and Source column in PriceHistoryModal", async () => {
    const ACCOUNT_NAME = "E2E MKT-142 Manual Badge";
    const catId = await seedCategory("E2E MKT-142 Cat");
    const assetId = await seedAsset("E2E MKT-142 Asset", catId, {
      reference: "MKT142",
      assetClass: "Stocks",
    });
    const accountId = await seedAccount(ACCOUNT_NAME);
    // Buy 10 units so the holding row is active (ACD-036 requires holdings for
    // the table to render). seedBuy pre-seeds a deposit to satisfy CSH-041.
    await seedBuy(accountId, assetId, "2020-03-01", 10);

    // Seed a manual price — record_asset_price hardcodes source=Manual (MKT-101).
    // Fixed past date (E2E rule E9): 2020-03-15.
    await seedAssetPrice(assetId, "2020-03-15", 55.0);

    await navigateToAccounts();
    await navigateToAccountDetails(accountId);

    // -----------------------------------------------------------------------
    // MKT-142 — source badge in the Current Price cell of the HoldingRow
    // -----------------------------------------------------------------------
    // The badge renders t("mkt.source_manual") = "Manual" inside a span with
    // `uppercase` Tailwind class. Tauri WebDriver's getText() returns CSS-rendered
    // visible text, so we assert the uppercase variant "MANUAL".
    await browser.waitUntil(
      async () => {
        const bodyText = await $("body").getText();
        return bodyText.includes("E2E MKT-142 Asset") && bodyText.includes("MANUAL");
      },
      {
        timeout: 10000,
        timeoutMsg:
          'HoldingRow must contain the asset name and "MANUAL" source badge after seedAssetPrice',
      },
    );

    // -----------------------------------------------------------------------
    // MKT-141 — Source column in PriceHistoryModal
    // -----------------------------------------------------------------------
    // Open the Price History modal via the History (clock) icon button in the
    // holding row (stable id `action-price-history-{assetId}`).
    const priceHistoryBtn = await $(`#action-price-history-${assetId}`);
    await priceHistoryBtn.waitForExist({ timeout: 10000 });
    await priceHistoryBtn.click();

    // ModalContainer renders inline (no portal) with role="dialog".
    const dialog = await $('[role="dialog"]');
    await dialog.waitForExist({ timeout: 10000 });

    // Column header ("Source") and badge value ("Manual") render through CSS
    // `uppercase`; WebDriver's getText() returns visible cased text.
    await browser.waitUntil(
      async () => {
        const modalText = await dialog.getText();
        return modalText.includes("SOURCE") && modalText.includes("MANUAL");
      },
      {
        timeout: 10000,
        timeoutMsg:
          'PriceHistoryModal must show "SOURCE" column header and "MANUAL" badge for the seeded price (MKT-141)',
      },
    );

    // ListModal's close button has no stable test id; use Escape (handled by
    // ModalContainer.useEffect) so the modal closes cleanly before the next test.
    await browser.keys("Escape");
    await dialog.waitForExist({ timeout: 5000, reverse: true });
  });
});
