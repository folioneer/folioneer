/**
 * E2E tests — Cash dividend recording flow (DIV)
 *
 * Spec: docs/spec/cash-dividend.md
 * Contract: docs/contracts/account-contract.md § Dividend
 * Spec rules covered:
 *   DIV-023    — recording credits the account's Cash Holding (cash row present after)
 *   DIV-072    — paying asset's holding row stays intact and its dividends-received
 *                cell surfaces the recorded amount
 *   DIV-021    — submit stays disabled until the form is valid
 *
 * Seed strategy:
 *   - Account + category + asset seeded via IPC (mirrors buy_sell.test.ts).
 *   - seedBuy opens a position so the asset qualifies for dividend recording
 *     (DIV-011 requires quantity > 0).
 *
 * ADR 007 (docs/adr/007-e2e-combobox-boundary.md): the dividend modal's asset
 * selector is now a ComboboxField, which cannot be automated in WebKitGTK
 * (HeadlessUI isTrusted + floating-ui portal). The happy path is therefore
 * driven via the record_dividend IPC (seedDividend) and the resulting UI state
 * asserted; a separate UI test verifies the submit-disabled guard. Full form
 * fill/submit wiring is covered by the Vitest component suite.
 */

import assert from "node:assert";
import { $ } from "@wdio/globals";
import { isoToDisplayDate } from "../helpers/date";
import { dismissLeftoverModal } from "../helpers/modal";
import {
  clickHeaderAction,
  navigateToAccountDetails,
  navigateToAccounts,
} from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedAsset, seedBuy, seedCategory, seedDividend } from "../helpers/seed";

// ---------------------------------------------------------------------------
// Fixed past dates — one per write operation (E2E rule E9)
// ---------------------------------------------------------------------------
const DATES = {
  dividend: "2020-06-15",
} as const;

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("dividend", () => {
  const ACCOUNT_NAME = "E2E Dividend DIV-023";
  const ASSET_NAME = "E2E Asset DIV023";
  let astId: string;
  let accId: string;

  // Seed prerequisites once via IPC — no UI interaction needed for setup
  // (mirrors open_balance.test.ts: seed in before(), not inside it()).
  before(async () => {
    const catId = await seedCategory("E2E Cat DIV023");
    accId = await seedAccount(ACCOUNT_NAME);
    astId = await seedAsset(ASSET_NAME, catId);
    // Open a 10-unit position so the asset qualifies for dividend recording
    // (DIV-011: quantity > 0). seedBuy internally seeds a deposit for cash.
    await seedBuy(accId, astId, "2020-05-01", 10_000_000); // 10 units
  });

  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // DIV-023/072 — record a dividend via IPC (ADR 007: combobox cannot be
  //   UI-automated), then assert the resulting account-details UI state: cash
  //   holding credited, paying asset holding intact, dividends-received cell surfaced.
  // -------------------------------------------------------------------------
  it("DIV-023/072: a recorded dividend credits cash and surfaces in the holding's dividends cell", async () => {
    // 75 EUR dividend on the held asset (EUR == account currency, rate 1:1).
    await seedDividend(accId, astId, DATES.dividend, 75_000_000);

    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // DIV-023 — Cash Holding credited: the cash row's inline Deposit button
    // must be present (same assertion anchor used by cash.test.ts CSH-022).
    const cashDepositBtn = await $("#action-record-deposit-system-cash-eur");
    await cashDepositBtn.waitForExist({ timeout: 8000 });
    assert.ok(
      await cashDepositBtn.isExisting(),
      "Cash holding row must be present after dividend (DIV-023 cash credit)",
    );

    // DIV-072 — Paying asset holding must be intact (Buy button still present).
    const buyBtn = await $(`#action-buy-${astId}`);
    await buyBtn.waitForExist({ timeout: 8000 });
    assert.ok(
      await buyBtn.isExisting(),
      "Paying asset holding row must remain after recording dividend (DIV-072 quantity unchanged)",
    );

    // DIV-072 — the paying asset's row must surface the dividend in its
    // "Dividends received" cell. Scoped to its stable id (E4) so the assertion
    // can't be satisfied by a coincidental "75,00" elsewhere on the page.
    // 75 EUR — locale formats as "75,00" (fr-FR) or "75.00" (en-US).
    const dividendsCell = await $(`#holding-dividends-received-${astId}`);
    await dividendsCell.waitForExist({ timeout: 8000 });
    await captureScreen("account-details-dividend");
    const dividendsCellText = await dividendsCell.getText();
    assert.ok(
      dividendsCellText.includes("75,00") || dividendsCellText.includes("75.00"),
      `Holding row dividends cell should surface the 75 EUR dividend (DIV-072) — got: ${dividendsCellText}`,
    );
  });

  // -------------------------------------------------------------------------
  // DIV-021 — submit-disabled guard via the UI. ADR 007: the asset combobox
  //   cannot be driven in WebDriver, so the asset stays empty; with date and
  //   amount filled, the composite isFormValid guard must keep submit disabled.
  // -------------------------------------------------------------------------
  it("DIV-021: submit stays disabled while no paying asset is selected", async () => {
    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    await clickHeaderAction("add-menu-dividend");

    const form = await $("form#dividend-transaction-form");
    await form.waitForExist({ timeout: 8000 });

    // Fill date + amount; asset selection skipped per ADR 007.
    await setReactInputValue("dividend-trx-date", isoToDisplayDate(DATES.dividend));
    await setReactInputValue("dividend-trx-amount", "75");

    const submitBtn = await $('button[type="submit"][form="dividend-transaction-form"]');
    await submitBtn.waitForExist({ timeout: 5000 });
    assert.strictEqual(
      await submitBtn.isEnabled(),
      false,
      "Submit must stay disabled until a paying asset is selected (DIV-021)",
    );
  });
});
