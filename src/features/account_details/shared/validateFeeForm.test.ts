import { describe, expect, it } from "vitest";
import { validateFeeSchedule, validatePercentage } from "./validateFeeForm";

describe("validatePercentage (FEE-021/032)", () => {
  it("rejects empty", () => {
    expect(validatePercentage("")).toEqual({ key: "validation.percentage_not_positive" });
  });

  it("rejects zero", () => {
    expect(validatePercentage("0")).toEqual({ key: "validation.percentage_not_positive" });
  });

  it("rejects negative", () => {
    expect(validatePercentage("-1")).toEqual({ key: "validation.percentage_not_positive" });
  });

  it("rejects NaN", () => {
    expect(validatePercentage("abc")).toEqual({ key: "validation.percentage_not_positive" });
  });

  it("rejects above 100", () => {
    expect(validatePercentage("100.01")).toEqual({ key: "validation.percentage_above_hundred" });
  });

  it("accepts a value in (0, 100]", () => {
    expect(validatePercentage("1.5")).toBeNull();
    expect(validatePercentage("100")).toBeNull();
  });
});

describe("validateFeeSchedule (FEE-032/045)", () => {
  const valid = { ratePercent: "1.5", startDate: "2024-01-01", endDate: "" };

  it("accepts a valid open-ended schedule", () => {
    expect(validateFeeSchedule(valid)).toBeNull();
  });

  it("accepts a valid schedule with an end date after the start", () => {
    expect(validateFeeSchedule({ ...valid, endDate: "2025-01-01" })).toBeNull();
  });

  it("surfaces the rate error first", () => {
    expect(validateFeeSchedule({ ...valid, ratePercent: "0" })).toEqual({
      key: "validation.percentage_not_positive",
    });
  });

  it("rejects an invalid start date", () => {
    expect(validateFeeSchedule({ ...valid, startDate: "nope" })).toEqual({
      key: "validation.invalid_date",
    });
  });

  it("rejects an invalid end date", () => {
    expect(validateFeeSchedule({ ...valid, endDate: "2024/02/02" })).toEqual({
      key: "validation.invalid_date",
    });
  });

  it("rejects an end date on or before the start date", () => {
    expect(validateFeeSchedule({ ...valid, endDate: "2024-01-01" })).toEqual({
      key: "validation.end_date_before_start",
    });
    expect(validateFeeSchedule({ ...valid, endDate: "2023-12-31" })).toEqual({
      key: "validation.end_date_before_start",
    });
  });
});
