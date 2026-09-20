/**
 * E2E tests — Management Fee recording and schedule creation (FEE)
 *
 * Spec:     docs/spec/management-fee-deduction.md
 * Contract: docs/contracts/account-contract.md § Management Fee
 *
 * Spec rules covered by this file:
 *   FEE-010 — "Management Fee" item in the header action row opens the modal
 *   FEE-011 — fee-schedule modal opened via the holding row's manage-fee action button
 *   FEE-021 — form fields: asset selector, date, percentage
 *   FEE-022 — recording reduces the holding quantity; no cash movement
 *   FEE-025 — modal closes and snackbar appears on success (observable via form gone)
 *   FEE-026 — Account Details re-fetches; holding row reflects the new quantity
 *   FEE-030 — create_fee_schedule: one schedule per (account, asset) pair
 *   FEE-032 — schedule fields: annual rate, frequency, start date
 *
 * Seed strategy:
 *   - category + asset (Stocks, non-cash) + account + buy 10 units seeded via IPC.
 *   - seedBuy internally seeds a deposit the prior day (CSH-041 pre-condition).
 *   - The management-fee modal's asset selector is a native <select> (SelectField),
 *     driven with selectByAttribute — no combobox automation needed.
 *   - The percentage field is a CalcField (type="text") driven with
 *     setReactInputValue (E2E rule E6).
 *
 * Scenario 1 — record a one-off fee:
 *   Record 10% fee on 10 held units → floor(10 × 0.10) = 1 unit removed →
 *   holding quantity drops from 10 to 9. The suite then asserts the holding row
 *   reflects "9".
 *
 * Scenario 2 — create a recurring fee schedule:
 *   Open the holding row's per-asset "Manage fee schedule" action; create a 2%
 *   annual Monthly schedule; assert the modal closes and the holding row survives
 *   the FeeScheduleUpdated re-fetch.
 *
 * Quantity math (FEE-022): removed_qty = floor(holding_as_of(date) × percent / 100)
 *   10 units at 10% → floor(10 × 0.10) = 1 unit removed → 9 remaining.
 *   Displayed as "9" by microToFormattedQuantity (whole units have no decimal).
 *
 * Error variants (AssetNotHeld, ManagementFeeOnCashAsset, PercentageNotPositive,
 * PercentageAboveHundred, CascadingOversell, ScheduleAlreadyExists) are adequately
 * covered at the backend-integration and Vitest-frontend tiers and are not repeated
 * here (test pyramid — E2E selects critical paths only).
 */

import { $, browser } from "@wdio/globals";
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
// Fixed past dates — one per write operation (E2E rule E9).
// "2020-09-15" is after the seeded buy on "2020-09-01", so holding_as_of
// returns 10 units and the 10% fee removes exactly 1 unit.
// ---------------------------------------------------------------------------
const DATES = {
  fee: isoToDisplayDate("2020-09-15"),
  feeByQuantity: isoToDisplayDate("2020-09-16"),
} as const;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("management_fee", () => {
  const ACCOUNT_NAME = "E2E ManagementFee FEE-022";
  const ASSET_NAME = "E2E Asset FEE022";
  let astId: string;
  let accId: string;

  // Seed prerequisites once via IPC — no UI interaction for setup.
  // A 10-unit buy opens the position so the asset appears in the
  // management-fee modal's asset selector (FEE-011/012: quantity > 0).
  before(async () => {
    const catId = await seedCategory("E2E Cat FEE022");
    // FEE-075 — new accounts default to fees-disabled; this suite needs the gate open.
    accId = await seedAccount(ACCOUNT_NAME, "EUR", "ManualMonth", true);
    astId = await seedAsset(ASSET_NAME, catId);
    // seedBuy seeds a deposit on the prior day (CSH-041) then buys 10 units.
    await seedBuy(accId, astId, "2020-09-01", 10_000_000); // 10 units in micros
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // FEE-022/025/026 — record a one-off management fee via the header action.
  //   10% of 10 held units = floor(10 × 0.10) = 1 unit removed → 9 units remain.
  //   The scenario asserts the holding row reflects the new quantity after the
  //   TransactionUpdated-triggered re-fetch.
  // -------------------------------------------------------------------------
  it("FEE-022/026: record a 10% management fee reduces holding quantity from 10 to 9", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // -------------------------------------------------------------------
    // Step 1 — Click the "Management Fee" header action (FEE-010).
    // -------------------------------------------------------------------
    await clickHeaderAction("add-menu-management-fee");

    // -------------------------------------------------------------------
    // Step 2 — Wait for the management-fee form (FEE-021).
    // -------------------------------------------------------------------
    const form = await $("form#management-fee-form");
    await form.waitForExist({ timeout: 8000 });

    // -------------------------------------------------------------------
    // Step 3 — Fill the form (FEE-021).
    //   asset selector: native <select> → selectByAttribute (locale-invariant).
    //   date: DateField (type="text") → setReactInputValue + isoToDisplayDate (E7).
    //   percent: CalcField (type="text") → setReactInputValue (E6).
    // -------------------------------------------------------------------
    const assetSelect = await $("#management-fee-asset-select");
    await assetSelect.waitForExist({ timeout: 5000 });
    await assetSelect.selectByAttribute("value", astId);

    // DateField expects the locale display format (en-US: MM/DD/YYYY) — E2E rule E7.
    await setReactInputValue("management-fee-date", DATES.fee);

    // CalcField is type="text"; plain numeric strings pass through without arithmetic.
    await setReactInputValue("management-fee-percent", "10");

    // -------------------------------------------------------------------
    // Step 4 — Submit (FEE-025 in-flight + success path).
    //   waitForEnabled confirms React processed all three input events (E6).
    // -------------------------------------------------------------------
    const submitBtn = await $('button[type="submit"][form="management-fee-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // -------------------------------------------------------------------
    // Step 5 — Assert post-conditions (FEE-025/026).
    // -------------------------------------------------------------------

    // FEE-025 — form must close on success.
    await form.waitForExist({ timeout: 8000, reverse: true });

    // FEE-022/026 — the holding quantity <td> must reflect 10 - 1 = 9 units
    // after the re-fetch triggered by TransactionUpdated (FEE-026).
    // microToFormattedQuantity renders whole units as plain integers: "9".
    const holdingQty = await $(`#holding-quantity-${astId}`);
    await holdingQty.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await holdingQty.getText()) === "9", {
      timeout: 8000,
      timeoutMsg: "Holding quantity must drop to 9 after recording a 10% management fee (FEE-022)",
    });
    await captureScreen("account-details-management-fee");
  });

  // -------------------------------------------------------------------------
  // FEE-028/029 — record a one-off fee by the quantity the holding should end at.
  //   Seeds its own account and 10-unit holding, so it depends on no other
  //   scenario: a resulting quantity of 9.5 removes exactly 0.5. The submit button
  //   only enables once the backend's preview has come back (FEE-029), which the
  //   preview line confirms.
  // -------------------------------------------------------------------------
  it("FEE-028/029: a fee entered as a resulting quantity of 9.5 leaves exactly 9.5 units", async () => {
    const catId = await seedCategory("E2E Cat FEE028");
    const accId = await seedAccount("E2E ManagementFee FEE-028", "EUR", "ManualMonth", true);
    const astId = await seedAsset("E2E Asset FEE028", catId);
    await seedBuy(accId, astId, "2020-09-01", 10_000_000);

    await navigateToAccounts();
    await navigateToAccountDetails(accId);
    await clickHeaderAction("add-menu-management-fee");

    const form = await $("form#management-fee-form");
    await form.waitForExist({ timeout: 8000 });

    const assetSelect = await $("#management-fee-asset-select");
    await assetSelect.waitForExist({ timeout: 5000 });
    await assetSelect.selectByAttribute("value", astId);
    await setReactInputValue("management-fee-date", DATES.feeByQuantity);

    const quantityMode = await $("#management-fee-mode-quantity");
    await quantityMode.waitForClickable({ timeout: 5000 });
    await quantityMode.click();
    const resultingQuantity = await $("#management-fee-resulting-quantity");
    await resultingQuantity.waitForExist({ timeout: 5000 });
    await setReactInputValue("management-fee-resulting-quantity", "9.5");

    // FEE-029 — the backend's preview appears before the form can be submitted.
    const preview = await $("#management-fee-preview");
    await preview.waitForExist({ timeout: 8000 });
    await captureScreen("account-details-management-fee-by-quantity");

    const submitBtn = await $('button[type="submit"][form="management-fee-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();
    await form.waitForExist({ timeout: 8000, reverse: true });

    const holdingQty = await $(`#holding-quantity-${astId}`);
    await holdingQty.waitForExist({ timeout: 8000 });
    await browser.waitUntil(async () => (await holdingQty.getText()) === "9.5", {
      timeout: 8000,
      timeoutMsg: "Holding quantity must be exactly 9.5 after the resulting-quantity fee (FEE-028)",
    });
  });

  // -------------------------------------------------------------------------
  // FEE-030/032 — create a recurring fee schedule for the held asset.
  //   Opens the per-holding "Manage fee schedule" action from the holding row;
  //   fills the 2% annual rate and submits. The modal's start date and frequency
  //   fields are pre-filled by useFeeSchedule (today + Monthly) and need no
  //   manual entry. Assert: form closes and the holding row survives the
  //   FeeScheduleUpdated-triggered re-fetch.
  //
  //   Note: this scenario runs after FEE-022/026 above, so the holding has 9
  //   units (not 10). The fee-schedule modal does not filter by quantity, so
  //   the manage-fee button is always present for any active non-cash holding.
  // -------------------------------------------------------------------------
  it("FEE-030/032: create a recurring 2% annual monthly fee schedule for the held asset", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // -------------------------------------------------------------------
    // Step 1 — Click the holding row's "Manage fee schedule" action (FEE-011).
    //   Direct button click (not a <tr> click) — WebDriver can target the
    //   button element by its stable id (E4) without the td:first-child workaround.
    // -------------------------------------------------------------------
    //   waitForClickable, not waitForExist: the previous scenario's success
    //   snackbar can still cover the row's second line of actions for its 4 s.
    const manageFeeBtn = await $(`#action-manage-fee-${astId}`);
    await manageFeeBtn.waitForClickable({ timeout: 10000 });
    await manageFeeBtn.click();

    // -------------------------------------------------------------------
    // Step 2 — Wait for the fee-schedule form (FEE-032).
    // -------------------------------------------------------------------
    const form = await $("form#fee-schedule-form");
    await form.waitForExist({ timeout: 8000 });

    // -------------------------------------------------------------------
    // Step 3 — Fill the annual rate (FEE-032).
    //   CalcField (type="text") → setReactInputValue (E6).
    //   "2" = 2% per year; CalcField passes plain numbers through unchanged.
    //
    //   Frequency defaults to "Monthly" (hook initial state — FEE-034).
    //   Start date defaults to today's ISO date (hook initial state — FEE-032);
    //   validateDate(today) returns null so no manual entry is needed.
    // -------------------------------------------------------------------
    await setReactInputValue("fee-schedule-rate", "2");

    // -------------------------------------------------------------------
    // Step 4 — Submit (FEE-030 in-flight + success path).
    //   waitForEnabled gates on isLoading=false (getFeeSchedule IPC returned)
    //   AND isFormValid=true (rate set + startDate already valid).
    //   Timeout is 8000 to allow the IPC round-trip for schedule load (E10).
    // -------------------------------------------------------------------
    const submitBtn = await $('button[type="submit"][form="fee-schedule-form"]');
    await submitBtn.waitForEnabled({ timeout: 8000 });
    await submitBtn.click();

    // -------------------------------------------------------------------
    // Step 5 — Assert post-conditions (FEE-030).
    // -------------------------------------------------------------------

    // FEE-030 — form must close on successful schedule creation.
    await form.waitForExist({ timeout: 8000, reverse: true });

    // FEE-011/064 — the manage-fee action survives the FeeScheduleUpdated re-fetch.
    // Re-query to avoid a stale element reference after the table re-renders;
    // waitForExist is itself the post-condition that the row came back.
    const manageFeeAction = await $(`#action-manage-fee-${astId}`);
    await manageFeeAction.waitForExist({ timeout: 8000 });
  });
});
