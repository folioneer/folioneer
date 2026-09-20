/**
 * E2E tests — Currency Rates manual-lifecycle flow
 *
 * Contract: docs/contracts/currency-contract.md
 * Spec rules covered:
 *   FXR-050 — get_currency_rates: rate list rendered in drill-in panel
 *   FXR-051 — get_currency_pairs: pair list rendered on Currency Rates view
 *   FXR-052 — update_currency_rate: edit modal pre-fills; rate row updates
 *   FXR-053 — delete_currency_rate: confirm dialog; rate row removed
 *   FXR-054 — declare_currency_pair: Add pair form accepted; pair row appears
 *   FXR-025 — record_currency_rate: new rate row appears in drill-in panel
 *
 * Pyramid rationale:
 *   Unit/integration tiers cover: domain validation (NotPositive, DateInFuture,
 *   IdentityPair, InvalidCurrency), service idempotency, f64→micros conversion,
 *   CurrencyRateUpdated event publication, and repository SQL. This E2E scenario
 *   locks in the critical-path UI→IPC→backend handshake: the full declare-pair →
 *   record-rate → edit-rate → delete-rate lifecycle as a single observable user
 *   journey. It is the only scenario in this suite because the breadth is already
 *   at the unit/integration apex; E2E sits at the pyramid tip.
 *
 * NOT covered here (and why):
 *   - FXR-023 IdentityPair inline error: domain-error → role="alert" round-trip is
 *     covered by the backend integration tests and the FE gateway unit tests; adding
 *     it here would duplicate coverage at a more expensive tier.
 *   - Holding-row FX shortcut (FXR-012): requires seeding a foreign-currency account +
 *     asset + buy + navigating to account details — setup heavier than the rate
 *     lifecycle and the shortcut itself is a URL-modal mount covered by FE unit tests
 *     (CurrencyRateEditMount.test.tsx). A future E2E pass can add it once the
 *     holding-row stable ids are audited.
 *   - Provider-fetched rates (FXR-102): hit real HTTP; non-deterministic and out of
 *     scope for an ephemeral-DB E2E suite (B36).
 *
 * Seed strategy:
 *   - No IPC seed needed — declare_currency_pair is available through the UI and the
 *     test exercises the full UI path.
 *
 * NOTE — stable `id` additions required before this test can pass:
 *   See selector inventory at the bottom of this file.
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { isoToDisplayDate } from "../helpers/date";
import { dismissLeftoverModal } from "../helpers/modal";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedCurrencyRate } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

/**
 * Navigate to the Currency Rates view via the sidebar nav button.
 *
 * The sidebar nav id is derived from the route path by Sidebar.tsx:
 *   id={`nav-${item.path.slice(1).replace(/\//g, "-")}`}
 * For path "/currency-rates" this produces id="nav-currency-rates".
 *
 * We bounce through Assets first (same pattern as account_details tests) so
 * any component currently mounted re-fetches on the way back. This also
 * forces the CurrencyRatesView to remount and pick up any IPC-seeded state.
 */
async function navigateToCurrencyRates(): Promise<void> {
  const assetsNav = await $("#nav-assets");
  await assetsNav.waitForExist({ timeout: 15000 });
  await assetsNav.click();
  await $("#fab-add-asset").waitForExist({ timeout: 10000 });

  const currencyNav = await $("#nav-currency-rates");
  await currencyNav.waitForExist({ timeout: 10000 });
  await currencyNav.click();
  // Wait for the "Add pair" button — reliable page-ready signal for
  // CurrencyRatesView regardless of whether any pairs have been declared.
  const addPairBtn = await $("#action-add-pair");
  await addPairBtn.waitForExist({ timeout: 10000 });
}

// ---------------------------------------------------------------------------
// Fixed past dates — one per write operation (E2E rule E9)
// ---------------------------------------------------------------------------
const DATES = {
  record: isoToDisplayDate("2020-04-10"),
  edit: isoToDisplayDate("2020-04-11"),
} as const;

// Rate key mirrors CurrencyRatesView: `${from}-${to}-${date}` where date is
// the ISO string that the backend stores (not the display format).
const PAIR_FROM = "USD";
const PAIR_TO = "EUR";
const RATE_ISO_DATE_RECORD = "2020-04-10";
const RATE_ISO_DATE_EDIT = "2020-04-11";

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("currency_rates", () => {
  // Navigate to Currency Rates view before each test to get a clean starting
  // point regardless of what the previous test left on screen.
  beforeEach(async () => {
    await dismissLeftoverModal();
    await navigateToCurrencyRates();
  });

  // -------------------------------------------------------------------------
  // FXR-054 + FXR-051 — declare pair via UI → pair row appears in list
  //
  // Critical path: the UI path to declare_currency_pair (the only FE-facing
  // pair-creation command per the contract Notes). After submit the pair list
  // re-fetches (onSuccess → refetch()) and the new pair row must be visible.
  // Idempotent: repeating the submit returns the existing pair (contract FXR-054
  // "existing pair is returned, not duplicated"), so the test is re-run-safe.
  // -------------------------------------------------------------------------
  it("FXR-054+FXR-051: declaring a pair via the UI adds it to the pair list", async () => {
    const addPairBtn = await $("#action-add-pair");
    await addPairBtn.waitForExist({ timeout: 10000 });
    await addPairBtn.click();

    // Dialog renders with role="dialog" — wait for it to appear.
    const dialog = await $('[role="dialog"]');
    await dialog.waitForExist({ timeout: 8000 });

    // Fill the from/to currency fields.
    // TextField forwards id to the underlying <input> (TextField.tsx:30).
    await setReactInputValue("declare-pair-from", PAIR_FROM);
    await setReactInputValue("declare-pair-to", PAIR_TO);

    // Submit via the "Add" button.
    // id="declare-pair-submit" is required — see selector inventory below.
    const submitBtn = await $("#declare-pair-submit");
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // Dialog must close after success.
    await dialog.waitForExist({ timeout: 8000, reverse: true });

    // Pair row must appear in the list.
    // id="pair-row-USD-EUR" is required — see selector inventory below.
    const pairRow = await $(`#pair-row-${PAIR_FROM}-${PAIR_TO}`);
    await pairRow.waitForExist({ timeout: 10000 });
    await captureScreen("currency-pairs");
  });

  // -------------------------------------------------------------------------
  // FXR-025 + FXR-050 — record a rate via UI → rate row appears in drill-in
  //
  // Critical path: after declaring the pair (or it already exists from a prior
  // run — idempotent), drill in to the pair and record a new rate. The drill-in
  // panel lists the pair's rates (get_currency_rates → FXR-050) and must show
  // the new row after the record_currency_rate command completes (FXR-025).
  // -------------------------------------------------------------------------
  it("FXR-025+FXR-050: recording a rate via the UI adds it to the drill-in rate list", async () => {
    // Ensure the pair exists — the declare command is idempotent.
    const addPairBtn = await $("#action-add-pair");
    await addPairBtn.waitForExist({ timeout: 10000 });
    await addPairBtn.click();
    const declarePairDialog = await $('[role="dialog"]');
    await declarePairDialog.waitForExist({ timeout: 8000 });
    await setReactInputValue("declare-pair-from", PAIR_FROM);
    await setReactInputValue("declare-pair-to", PAIR_TO);
    const declareSubmitBtn = await $("#declare-pair-submit");
    await declareSubmitBtn.waitForEnabled({ timeout: 5000 });
    await declareSubmitBtn.click();
    await declarePairDialog.waitForExist({ timeout: 8000, reverse: true });

    // Drill in to the pair by clicking its row.
    // id="pair-row-USD-EUR" is required — see selector inventory below.
    const pairRow = await $(`#pair-row-${PAIR_FROM}-${PAIR_TO}`);
    await pairRow.waitForExist({ timeout: 10000 });
    // Click the first cell (an interactable leaf) rather than the <tr> itself —
    // a row is never pointer-interactable in WebDriver (its centre hit-tests to a
    // child cell). The click bubbles to the row's onClick. Matches the
    // account_performance E2E precedent (`#account-row-{id} td:first-child`).
    await pairRow.$("td:first-child").click();

    // "Record rate" button appears in the drill-in panel header.
    const recordRateBtn = await $("#currency-rates-action-record-rate");
    await recordRateBtn.waitForExist({ timeout: 8000 });
    await recordRateBtn.click();

    const recordDialog = await $('[role="dialog"]');
    await recordDialog.waitForExist({ timeout: 8000 });

    // Date field: record-rate-date expects the DateField display format (en-US MM/DD/YYYY).
    // The backend stores the ISO date; we use a fixed past date (E2E rule E9).
    await setReactInputValue("record-rate-date", DATES.record);
    // Rate field: record-rate-rate accepts a decimal string.
    await setReactInputValue("record-rate-rate", "1.08");

    // Submit via the "Save" button.
    // id="record-rate-submit" is required — see selector inventory below.
    const submitBtn = await $("#record-rate-submit");
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // Dialog must close after success.
    await recordDialog.waitForExist({ timeout: 8000, reverse: true });

    // Rate row must appear in the drill-in list.
    // id mirrors CurrencyRatesView: `rate-row-${from}-${to}-${isoDate}`.
    // id="rate-row-USD-EUR-2020-04-10" is required — see selector inventory below.
    const rateRow = await $(`#rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_RECORD}`);
    await rateRow.waitForExist({ timeout: 10000 });
    assert.ok(
      await rateRow.isExisting(),
      `Rate row #rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_RECORD} must appear after record (FXR-025/FXR-050)`,
    );
  });

  // -------------------------------------------------------------------------
  // FXR-052 — edit a rate via UI → rate row updates with new date
  //
  // Seeds the rate via IPC for independence, then exercises the edit modal
  // through the UI. Changing the date also exercises the delete-old+upsert-new
  // code path in update_currency_rate.
  // -------------------------------------------------------------------------
  it("FXR-052: editing a rate via the UI updates the rate row", async () => {
    // Seed pair + rate via IPC so this test doesn't depend on the record test.
    await seedCurrencyRate(PAIR_FROM, PAIR_TO, RATE_ISO_DATE_RECORD, 1.08);

    // Navigate to Currency Rates view (already done in beforeEach — but the
    // IPC seed above happens after beforeEach, so we need the drill-in from here).
    // Bounce through another nav tab to force the CurrencyRatesView to remount
    // and pick up the IPC-seeded pair/rate.
    const assetsNav = await $("#nav-assets");
    await assetsNav.waitForExist({ timeout: 10000 });
    await assetsNav.click();
    await $("#fab-add-asset").waitForExist({ timeout: 10000 });
    const currencyNav = await $("#nav-currency-rates");
    await currencyNav.waitForExist({ timeout: 10000 });
    await currencyNav.click();
    await $("#action-add-pair").waitForExist({ timeout: 10000 });

    // Drill in to the pair.
    const pairRow = await $(`#pair-row-${PAIR_FROM}-${PAIR_TO}`);
    await pairRow.waitForExist({ timeout: 10000 });
    // Click the first cell (an interactable leaf) rather than the <tr> itself —
    // a row is never pointer-interactable in WebDriver (its centre hit-tests to a
    // child cell). The click bubbles to the row's onClick. Matches the
    // account_performance E2E precedent (`#account-row-{id} td:first-child`).
    await pairRow.$("td:first-child").click();

    // Click the edit button for the seeded rate row.
    // id="action-edit-rate-USD-EUR-2020-04-10" — already present in CurrencyRatesView.tsx.
    const editBtn = await $(`#action-edit-rate-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_RECORD}`);
    await editBtn.waitForExist({ timeout: 10000 });
    await editBtn.click();

    const editDialog = await $('[role="dialog"]');
    await editDialog.waitForExist({ timeout: 8000 });

    // Change the date to DATES.edit (2020-04-11) and adjust the rate.
    await setReactInputValue("record-rate-date", DATES.edit);
    await setReactInputValue("record-rate-rate", "1.09");

    // id="record-rate-submit" is required — see selector inventory below.
    const submitBtn = await $("#record-rate-submit");
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    await editDialog.waitForExist({ timeout: 8000, reverse: true });

    // The old rate row (original date) must be gone.
    const oldRateRow = await $(`#rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_RECORD}`);
    await oldRateRow.waitForExist({ timeout: 8000, reverse: true });
    assert.strictEqual(
      await oldRateRow.isExisting(),
      false,
      `Old rate row #rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_RECORD} must disappear after date-change edit (FXR-052)`,
    );

    // The new rate row (new date) must be present.
    // id="rate-row-USD-EUR-2020-04-11" is required — see selector inventory below.
    const newRateRow = await $(`#rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_EDIT}`);
    await newRateRow.waitForExist({ timeout: 10000 });
    assert.ok(
      await newRateRow.isExisting(),
      `New rate row #rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_EDIT} must appear after edit (FXR-052)`,
    );
  });

  // -------------------------------------------------------------------------
  // FXR-053 — delete a rate via UI → rate row removed from drill-in list
  //
  // Seeds the rate via IPC for independence, then exercises the delete
  // confirmation dialog through the UI. Asserts the rate row disappears.
  // The pair itself must remain (FXR-014 — delete_currency_rate never removes
  // the pair); we verify the pair row is still present.
  // -------------------------------------------------------------------------
  it("FXR-053: deleting a rate via the confirm dialog removes the rate row, pair survives", async () => {
    // Seed pair + rate via IPC.
    await seedCurrencyRate(PAIR_FROM, PAIR_TO, RATE_ISO_DATE_EDIT, 1.09);

    // Force remount to pick up the seeded data.
    const assetsNav = await $("#nav-assets");
    await assetsNav.waitForExist({ timeout: 10000 });
    await assetsNav.click();
    await $("#fab-add-asset").waitForExist({ timeout: 10000 });
    const currencyNav = await $("#nav-currency-rates");
    await currencyNav.waitForExist({ timeout: 10000 });
    await currencyNav.click();
    await $("#action-add-pair").waitForExist({ timeout: 10000 });

    // Drill in to the pair.
    const pairRow = await $(`#pair-row-${PAIR_FROM}-${PAIR_TO}`);
    await pairRow.waitForExist({ timeout: 10000 });
    // Click the first cell (an interactable leaf) rather than the <tr> itself —
    // a row is never pointer-interactable in WebDriver (its centre hit-tests to a
    // child cell). The click bubbles to the row's onClick. Matches the
    // account_performance E2E precedent (`#account-row-{id} td:first-child`).
    await pairRow.$("td:first-child").click();

    // Click the delete button for the seeded rate row.
    // id="action-delete-rate-USD-EUR-2020-04-11" — already present in CurrencyRatesView.tsx.
    const deleteBtn = await $(`#action-delete-rate-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_EDIT}`);
    await deleteBtn.waitForExist({ timeout: 10000 });
    await deleteBtn.click();

    const deleteDialog = await $('[role="dialog"]');
    await deleteDialog.waitForExist({ timeout: 8000 });

    // Confirm deletion.
    // id="delete-rate-confirm" is required — see selector inventory below.
    const confirmBtn = await $("#delete-rate-confirm");
    await confirmBtn.waitForEnabled({ timeout: 5000 });
    await confirmBtn.click();

    await deleteDialog.waitForExist({ timeout: 8000, reverse: true });

    // Rate row must be gone from the drill-in list.
    // id="rate-row-USD-EUR-2020-04-11" is required — see selector inventory below.
    const deletedRateRow = await $(`#rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_EDIT}`);
    await deletedRateRow.waitForExist({ timeout: 8000, reverse: true });
    assert.strictEqual(
      await deletedRateRow.isExisting(),
      false,
      `Rate row #rate-row-${PAIR_FROM}-${PAIR_TO}-${RATE_ISO_DATE_EDIT} must disappear after delete (FXR-053)`,
    );

    // Pair row must still exist (FXR-014 — pair is never removed by rate deletion).
    const pairRowAfter = await $(`#pair-row-${PAIR_FROM}-${PAIR_TO}`);
    await pairRowAfter.waitForExist({ timeout: 10000 });
  });
});

// ---------------------------------------------------------------------------
// Selector inventory
//
// Existing stable `id`s used by this file (already in the codebase):
//
//   #nav-currency-rates
//     Sidebar.tsx — generated by `nav-${item.path.slice(1).replace(/\//g, "-")}`
//     for navItems.ts entry path="/currency-rates"
//
//   #currency-rates-action-record-rate
//     src/features/currency/currency_rates_view/CurrencyRatesView.tsx:114
//     Button in the drill-in panel header
//
//   #action-edit-rate-${from}-${to}-${isoDate}
//     src/features/currency/currency_rates_view/CurrencyRatesView.tsx:152
//     Per-row edit Button; key = `${rate.from_currency}-${rate.to_currency}-${rate.date}`
//
//   #action-delete-rate-${from}-${to}-${isoDate}
//     src/features/currency/currency_rates_view/CurrencyRatesView.tsx:158
//     Per-row delete Button; same key as edit
//
//   #declare-pair-from
//     src/features/currency/declare_pair/DeclarePairModal.tsx:53
//     TextField id, forwarded to <input> by TextField.tsx:30
//
//   #declare-pair-to
//     src/features/currency/declare_pair/DeclarePairModal.tsx:61
//     TextField id, forwarded to <input> by TextField.tsx:30
//
//   #record-rate-date
//     src/features/currency/record_rate/RecordRateModal.tsx:79
//     TextField id, forwarded to <input> by TextField.tsx:30
//
//   #record-rate-rate
//     src/features/currency/record_rate/RecordRateModal.tsx:99
//     TextField id, forwarded to <input> by TextField.tsx:30
//
// ---------------------------------------------------------------------------
// Missing stable `id`s — the main agent MUST add these in the same PR
// before running the suite (E2E rule E4):
//
//   id="action-add-pair"
//     File: src/features/currency/currency_rates_view/CurrencyRatesView.tsx
//     Line: 95 (the "Add pair" tonal Button)
//     Current: data-testid="action-add-pair" only — add id="action-add-pair"
//
//   id="declare-pair-submit"
//     File: src/features/currency/declare_pair/DeclarePairModal.tsx
//     Line: 35 (the "Add" primary Button inside DeclarePairModal)
//     Current: data-testid="declare-pair-submit" only — add id="declare-pair-submit"
//
//   id="record-rate-submit"
//     File: src/features/currency/record_rate/RecordRateModal.tsx
//     Line: 54 (the "Save" primary Button inside RecordRateModal — used for both
//            create and edit mode because it's the same component)
//     Current: data-testid="record-rate-submit" only — add id="record-rate-submit"
//
//   id="delete-rate-confirm"
//     File: src/features/currency/record_rate/RecordRateModal.tsx
//     Line: 163 (the "Delete" danger Button inside DeleteRateConfirmation)
//     Current: data-testid="delete-rate-confirm" only — add id="delete-rate-confirm"
//
//   id="pair-row-${from}-${to}"  (e.g. id="pair-row-USD-EUR")
//     File: src/features/currency/currency_rates_view/CurrencyRatesView.tsx
//     Line: 55 (<tr> element, currently has data-testid={`pair-row-${key}`} only)
//     Change: add id={`pair-row-${key}`} alongside the existing data-testid
//     Note: <tr> is used as a click target to drill in to the pair's rate list;
//           a stable id is needed for reliable WebDriver click targeting.
//
//   id="rate-row-${from}-${to}-${isoDate}"  (e.g. id="rate-row-USD-EUR-2020-04-10")
//     File: src/features/currency/currency_rates_view/CurrencyRatesView.tsx
//     Line: 140 (<tr> element in the drill-in list, currently has
//            data-testid={`rate-row-${rateKey}`} only)
//     Change: add id={`rate-row-${rateKey}`} alongside the existing data-testid
//     Note: used both for presence assertions and waitForExist(reverse: true)
//           after edit/delete operations.
// ---------------------------------------------------------------------------
