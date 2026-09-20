import { describe, expect, it } from "vitest";
import { validateAmount, validateDate } from "./validateCashForm";

describe("validateAmount (CSH-021/031)", () => {
  it("rejects empty", () => {
    expect(validateAmount("")).toEqual({ key: "validation.amount_not_positive" });
  });

  it("rejects zero", () => {
    expect(validateAmount("0")).toEqual({ key: "validation.amount_not_positive" });
  });

  it("rejects negative", () => {
    expect(validateAmount("-5")).toEqual({ key: "validation.amount_not_positive" });
  });

  it("rejects NaN", () => {
    expect(validateAmount("abc")).toEqual({ key: "validation.amount_not_positive" });
  });

  it("accepts strictly positive", () => {
    expect(validateAmount("1.50")).toBeNull();
  });
});

describe("validateDate (CSH-021/031, TRX-020 bounds)", () => {
  it("rejects empty", () => {
    expect(validateDate("")).toEqual({ key: "validation.invalid_date" });
  });

  it("rejects malformed", () => {
    expect(validateDate("2026/01/01")).toEqual({ key: "validation.invalid_date" });
  });

  it("rejects future date", () => {
    expect(validateDate("2099-12-31")).toEqual({ key: "validation.date_in_future" });
  });

  it("rejects pre-1900", () => {
    expect(validateDate("1899-12-31")).toEqual({ key: "validation.date_too_old" });
  });

  it("accepts a past date in range", () => {
    expect(validateDate("2020-01-01")).toBeNull();
  });
});
