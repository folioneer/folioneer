/**
 * E2E tests — Account Performance page (PRF)
 *
 * Contract: docs/contracts/account-contract.md § "Account Performance"
 * Spec rules covered:
 *   PRF-010 — entry point: "Performance" button in AccountDetailsView header navigates to the page
 *   PRF-013 — month view available for sub-weekly update frequencies (Automatic/ManualDay/ManualWeek)
 *   PRF-037 — YTD column present in month view, absent in year view
 *   PRF-043 — no transactions → empty state rendered (account-performance-empty)
 *   PRF-051 — empty state shows add-transaction affordance
 *
 * Deliberately NOT covered at this layer (and why):
 *
 *   PRF-016 (AccountNotFound error state) — the UI always navigates from a valid account
 *     row; the NotFound path is not reachable via normal UI interaction. Covered by BE
 *     Tier 2/3 tests.
 *
 *   PRF-052 (database error → error state) — requires injecting a DB failure mid-flight;
 *     not reproducible deterministically in the E2E suite. Covered by FE hook unit tests.
 *
 *   PRF-032 (Simple Dietz percentage values) — math correctness belongs at the unit /
 *     integration tier (already covered by BE Rust tests). E2E asserts presence of the
 *     rendered table, not the exact computed figures.
 *
 *   PRF-015 (year selector) — the year selector is only visible when month view is active
 *     AND there is data. Its presence and content are covered by the FE integration test
 *     (AccountPerformancePage.integration.test.tsx). E2E verifies the populated path with
 *     a single-year fixture; cycling through year-selector options adds setup complexity
 *     with no incremental cross-stack value.
 *
 * Seed strategy:
 *   - Populated path (PRF-010 / PRF-013 / PRF-037): account seeded via IPC with
 *     update_frequency="Automatic" so month_view_available is true; one deposit gives the
 *     backend a data span to compute. Navigation mirrors the buy_sell / cash test pattern.
 *   - Empty path (PRF-043 / PRF-051): account seeded via IPC with default ManualMonth
 *     frequency; no transactions → empty state.
 *
 * Note on selectors:
 *   All elements selected here carry a stable `id`; the test selects exclusively by `#id`
 *   (E1/E4). The table, row, YTD column header, and empty-state container were given `id`s
 *   alongside their existing `data-testid` so E2E never depends on a non-`id` selector.
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedDeposit } from "../helpers/seed";

// Fixed past dates (E2E rule E9 — never today's date).
const FIXTURE_DATES = {
  deposit: "2020-03-15",
} as const;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("account_performance", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // PRF-010 / PRF-013 / PRF-037 — populated path
  //
  // Critical-path IPC → UI handshake: account with Automatic frequency + one
  // deposit → navigate to Performance page → table renders with at least one
  // row; in month view the YTD column (PRF-037) is present; toggling to year
  // view makes the YTD column disappear.
  // -------------------------------------------------------------------------
  it("PRF-010/013/037: populated account shows performance table; YTD column present in month view and absent in year view", async () => {
    const ACCOUNT_NAME = "E2E PRF-010 Populated";

    // Seed an account with Automatic frequency (month view available, PRF-013).
    const accountId = await seedAccount(ACCOUNT_NAME, "EUR", "Automatic");

    // Seed one deposit so the backend has a data span to compute (PRF-040).
    await seedDeposit(accountId, FIXTURE_DATES.deposit, 1_000_000_000); // 1 000 EUR

    // Navigate to AccountDetailsView via the standard round-trip pattern.
    await navigateToAccounts();
    await navigateToAccountDetails(accountId);

    // Click the Performance entry-point button (PRF-010, id="account-details-performance").
    const perfBtn = await $("#account-details-performance");
    await perfBtn.waitForExist({ timeout: 10000 });
    await perfBtn.click();

    // Wait for the performance table to appear.
    const table = await $("#account-performance-table");
    await table.waitForExist({ timeout: 15000 });
    assert.ok(
      await table.isExisting(),
      "Performance table must be present after navigation (PRF-010)",
    );

    // At least one row must be rendered (deposit gives a data span). The full row
    // id ends with a backend-derived period key, so match on the stable id prefix.
    const firstRow = await $('[id^="account-performance-row-"]');
    await firstRow.waitForExist({ timeout: 8000 });
    assert.ok(await firstRow.isExisting(), "At least one period row must render (PRF-040)");

    // PRF-013: account has Automatic frequency → month view is the default (PRF-014).
    // The view-mode toggle fieldset must be present.
    const viewToggle = await $("#account-performance-view-toggle");
    await viewToggle.waitForExist({ timeout: 8000 });
    assert.ok(
      await viewToggle.isExisting(),
      "View-mode toggle must be present for Automatic account (PRF-013)",
    );

    // PRF-037: in month view the YTD column header must be visible.
    const ytdCol = await $("#account-performance-col-ytd");
    await ytdCol.waitForExist({ timeout: 8000 });
    await captureScreen("account-performance");
    assert.ok(await ytdCol.isExisting(), "YTD column must be present in month view (PRF-037)");

    // Switch to year view via the toggle button (id="account-performance-view-toggle-year").
    const yearToggle = await $("#account-performance-view-toggle-year");
    await yearToggle.waitForExist({ timeout: 8000 });
    await yearToggle.click();

    // PRF-037: in year view the YTD column must be gone.
    await ytdCol.waitForExist({ timeout: 5000, reverse: true });
    assert.strictEqual(
      await ytdCol.isExisting(),
      false,
      "YTD column must be absent in year view (PRF-037)",
    );

    // The table itself must still be present after the mode switch.
    assert.ok(
      await table.isExisting(),
      "Performance table must remain after switching to year view",
    );
  });

  // -------------------------------------------------------------------------
  // PRF-043 / PRF-051 — empty path
  //
  // A freshly-created account with no transactions produces an empty result
  // from get_account_performance → the page shows the empty state element,
  // not the table.
  // -------------------------------------------------------------------------
  it("PRF-043/051: account with no transactions shows empty state", async () => {
    const ACCOUNT_NAME = "E2E PRF-043 Empty";

    // Default ManualMonth frequency is fine here — we only need the empty state.
    const accountId = await seedAccount(ACCOUNT_NAME);

    await navigateToAccounts();
    await navigateToAccountDetails(accountId);

    const perfBtn = await $("#account-details-performance");
    await perfBtn.waitForExist({ timeout: 10000 });
    await perfBtn.click();

    // Empty state must appear (PRF-051).
    const emptyState = await $("#account-performance-empty");
    await emptyState.waitForExist({ timeout: 15000 });
    assert.ok(
      await emptyState.isExisting(),
      "Empty state must be shown for an account with no transactions (PRF-043)",
    );

    // The Add Transaction affordance must be present inside the empty state (PRF-051).
    const addTrxBtn = await $("#account-performance-add-transaction");
    await addTrxBtn.waitForExist({ timeout: 8000 });
    assert.ok(
      await addTrxBtn.isExisting(),
      "Add Transaction affordance must be present in the empty state (PRF-051)",
    );

    // The table must not be rendered alongside the empty state.
    const table = await $("#account-performance-table");
    assert.strictEqual(
      await table.isExisting(),
      false,
      "Performance table must not render when the empty state is shown (PRF-043)",
    );
  });
});
