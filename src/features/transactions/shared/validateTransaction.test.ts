import { describe, expect, it } from "vitest";
import type { TransactionFormData } from "./types";
import { validateSellForm, validateTransactionForm } from "./validateTransaction";

const MICRO = 1_000_000;

const validForm: TransactionFormData = {
  accountId: "account-1",
  assetId: "asset-1",
  date: "2026-06-01",
  quantity: "2",
  unitPrice: "100",
  exchangeRate: "1.000000",
  fees: "10",
  note: "",
};

describe("validateTransactionForm", () => {
  it("returns null for a valid form", () => {
    expect(validateTransactionForm(validForm, 2 * MICRO, 210 * MICRO)).toBeNull();
  });

  it("rejects a non-positive quantity", () => {
    expect(validateTransactionForm(validForm, 0, 210 * MICRO)).toEqual({
      key: "transaction.error_validation_quantity",
    });
  });

  it("rejects a non-positive total", () => {
    expect(validateTransactionForm(validForm, 2 * MICRO, 0)).toEqual({
      key: "transaction.error_validation_total",
    });
  });

  // TRX-060 — total-entry buy: the all-in total must be at least the fees
  it("rejects a total below the fees in total-entry mode", () => {
    expect(validateTransactionForm(validForm, 2 * MICRO, 5 * MICRO, 10 * MICRO)).toEqual({
      key: "transaction.error_validation_total_below_fees",
    });
  });

  // TRX-060 — total == fees is allowed (securities part is zero, not negative)
  it("accepts a total equal to the fees in total-entry mode", () => {
    expect(validateTransactionForm(validForm, 2 * MICRO, 10 * MICRO, 10 * MICRO)).toBeNull();
  });

  it("skips the below-fees check outside total-entry mode", () => {
    expect(validateTransactionForm(validForm, 2 * MICRO, 5 * MICRO, null)).toBeNull();
  });
});

describe("validateSellForm", () => {
  it("returns null for a valid sell within the held quantity", () => {
    expect(validateSellForm(validForm, 2 * MICRO, 190 * MICRO, 3 * MICRO)).toBeNull();
  });

  // SEL-022 — oversell guard applies regardless of entry mode
  it("rejects a quantity above the held quantity", () => {
    expect(validateSellForm(validForm, 4 * MICRO, 190 * MICRO, 3 * MICRO)).toEqual({
      key: "transaction.error_validation_oversell",
      vars: { max: expect.any(String) },
    });
  });

  // SEL-050 — a typed sell total must be strictly positive
  it("rejects a non-positive total", () => {
    expect(validateSellForm(validForm, 2 * MICRO, 0, 3 * MICRO)).toEqual({
      key: "transaction.error_validation_total",
    });
  });
});
