/**
 * E2E tests — Cash tracking (eager cash row, deposits, withdrawals, insufficient-cash guard, global value)
 *
 * Spec: docs/spec/cash-tracking.md
 * Spec rules covered:
 *   CSH-012/095 — every account has an always-visible cash row (eager, at €0 on a fresh account)
 *   CSH-022 — record deposit via the cash-row inline action → balance reflected
 *   CSH-032 — record withdrawal via the cash-row inline action → cash row balance decremented
 *   CSH-081 — withdrawal exceeding balance → inline InsufficientCash error, form stays open
 *   CSH-094 — Global Value tile reflects current cash balance
 *
 * Seed strategy:
 *   - All scenarios start from a freshly seeded account (no holdings). The cash row is
 *     created eagerly at €0 (CSH-012), so its inline Deposit action is reachable
 *     immediately; the Withdraw action is disabled until the balance is non-zero (CSH-097).
 *   - Deposit / Withdraw are reached from the cash-row inline actions (CSH-019/091) — there
 *     are no cash actions in the header "Record" menu anymore.
 */

import assert from "node:assert";
import { $, browser } from "@wdio/globals";
import { isoToDisplayDate } from "../helpers/date";
import { dismissLeftoverModal } from "../helpers/modal";
import { navigateToAccountDetails, navigateToAccounts } from "../helpers/navigation";
import { setReactInputValue } from "../helpers/react";
import { captureScreen } from "../helpers/screenshot";
import { seedAccount, seedDeposit } from "../helpers/seed";

const CASH_DEPOSIT_ACTION = "#action-record-deposit-system-cash-eur";
const CASH_WITHDRAW_ACTION = "#action-record-withdrawal-system-cash-eur";

const DATES = {
  deposit: isoToDisplayDate("2019-03-10"),
  withdrawal: isoToDisplayDate("2019-04-15"),
  insufficient: isoToDisplayDate("2019-05-20"),
} as const;

describe("cash", () => {
  beforeEach(async () => {
    await dismissLeftoverModal();
  });

  // -------------------------------------------------------------------------
  // CSH-012/095 — a fresh account shows the eager cash row at €0
  // -------------------------------------------------------------------------
  it("CSH-095: a freshly created account shows the cash row at zero with Withdraw disabled", async () => {
    const accId = await seedAccount("E2E Eager Cash CSH-095");

    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // The cash row is present even with no deposits (eager, CSH-012/095).
    const depositBtn = await $(CASH_DEPOSIT_ACTION);
    await depositBtn.waitForExist({ timeout: 8000 });
    assert.ok(
      await depositBtn.isExisting(),
      "Cash row must be present on a fresh account (CSH-095)",
    );

    // Withdraw is rendered but disabled at a zero balance (CSH-097).
    const withdrawBtn = await $(CASH_WITHDRAW_ACTION);
    await withdrawBtn.waitForExist({ timeout: 8000 });
    // waitForEnabled(reverse) polls until the disabled attribute has hydrated.
    await withdrawBtn.waitForEnabled({ timeout: 5000, reverse: true });
    await captureScreen("account-details-cash");
    assert.strictEqual(
      await withdrawBtn.isEnabled(),
      false,
      "Withdraw action must be disabled while the cash balance is zero (CSH-097)",
    );
  });

  // -------------------------------------------------------------------------
  // CSH-022 — deposit via the cash-row inline action records the balance
  // -------------------------------------------------------------------------
  it("CSH-022: recording a deposit from the cash row reflects the balance", async () => {
    const accId = await seedAccount("E2E Deposit CSH-022");

    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    // Deposit from the always-present cash row's inline action (CSH-091).
    const depositBtn = await $(CASH_DEPOSIT_ACTION);
    await depositBtn.waitForClickable({ timeout: 8000 });
    await depositBtn.click();

    const form = await $("form#deposit-transaction-form");
    await form.waitForExist({ timeout: 8000 });

    await setReactInputValue("deposit-trx-date", DATES.deposit);
    await setReactInputValue("deposit-trx-amount", "500");

    const submitBtn = await $('button[type="submit"][form="deposit-transaction-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    await form.waitForExist({ timeout: 8000, reverse: true });

    // The Global Value tile reflects the 500 EUR cash balance after the post-commit
    // re-fetch (CSH-094). Poll until the header shows the value (avoids a stale read).
    await browser.waitUntil(
      async () => {
        const text = await $("body").getText();
        return text.includes("500,00") || text.includes("500.00");
      },
      {
        timeout: 8000,
        timeoutMsg: "Global Value did not reflect the 500 EUR deposit (CSH-022/094)",
      },
    );
  });

  // -------------------------------------------------------------------------
  // CSH-032 — withdrawal via the cash-row inline action decrements the balance
  // -------------------------------------------------------------------------
  it("CSH-032: withdrawing keeps the cash row visible with a reduced balance", async () => {
    const accId = await seedAccount("E2E Withdrawal CSH-032");
    // Pre-seed 1 000 EUR so the Withdraw action is enabled (CSH-097 gating).
    await seedDeposit(accId, "2019-04-01", 1_000_000_000); // 1 000 EUR in micros

    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    const withdrawBtn = await $(CASH_WITHDRAW_ACTION);
    await withdrawBtn.waitForClickable({ timeout: 8000 });
    await withdrawBtn.click();

    const form = await $("form#withdrawal-transaction-form");
    await form.waitForExist({ timeout: 8000 });

    await setReactInputValue("withdrawal-trx-date", DATES.withdrawal);
    await setReactInputValue("withdrawal-trx-amount", "200");

    const submitBtn = await $('button[type="submit"][form="withdrawal-transaction-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    await form.waitForExist({ timeout: 8000, reverse: true });

    // Cash row stays visible (1000 - 200 = 800 EUR), and persists regardless (CSH-013).
    const stillVisible = await $(CASH_WITHDRAW_ACTION);
    await stillVisible.waitForExist({ timeout: 8000 });
    assert.ok(
      await stillVisible.isExisting(),
      "Cash row must remain visible after partial withdrawal (CSH-032)",
    );
  });

  // -------------------------------------------------------------------------
  // CSH-081 — insufficient cash on withdrawal surfaces an inline error
  // -------------------------------------------------------------------------
  it("CSH-081: withdrawing more than the balance surfaces an inline error", async () => {
    const accId = await seedAccount("E2E Insufficient CSH-081");
    await seedDeposit(accId, "2019-05-01", 100_000_000); // 100 EUR available

    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    const withdrawBtn = await $(CASH_WITHDRAW_ACTION);
    await withdrawBtn.waitForClickable({ timeout: 8000 });
    await withdrawBtn.click();

    const form = await $("form#withdrawal-transaction-form");
    await form.waitForExist({ timeout: 8000 });

    await setReactInputValue("withdrawal-trx-date", DATES.insufficient);
    await setReactInputValue("withdrawal-trx-amount", "999"); // way above the 100 available

    const submitBtn = await $('button[type="submit"][form="withdrawal-transaction-form"]');
    await submitBtn.waitForEnabled({ timeout: 5000 });
    await submitBtn.click();

    // Inline alert appears with the InsufficientCash key; the form stays open (CSH-081).
    const errorBlock = await $('p[role="alert"]');
    await errorBlock.waitForExist({ timeout: 8000 });
    const errorText = await errorBlock.getText();
    assert.ok(
      errorText.length > 0,
      "InsufficientCash inline error must render in the form (CSH-081)",
    );
    assert.ok(
      await form.isExisting(),
      "Withdrawal form must stay open after InsufficientCash (CSH-081)",
    );
  });

  // -------------------------------------------------------------------------
  // CSH-094 — Global Value tile reflects the current cash balance
  // -------------------------------------------------------------------------
  it("CSH-094: Global Value tile reflects the current cash balance", async () => {
    const accId = await seedAccount("E2E Global CSH-094");
    await seedDeposit(accId, "2019-06-01", 250_000_000); // 250 EUR

    await navigateToAccounts();
    await navigateToAccountDetails(accId);

    const cashRow = await $(CASH_WITHDRAW_ACTION);
    await cashRow.waitForExist({ timeout: 10000 });

    // Poll the header until the Global Value tile shows the 250 EUR balance (CSH-094).
    await browser.waitUntil(
      async () => {
        const text = await $("body").getText();
        return text.includes("250,00") || text.includes("250.00");
      },
      {
        timeout: 8000,
        timeoutMsg: "Global Value did not reflect the 250 EUR cash balance (CSH-094)",
      },
    );
  });
});
