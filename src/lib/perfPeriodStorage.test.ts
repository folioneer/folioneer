import { afterEach, describe, expect, it } from "vitest";
import { getPerfPeriod, setPerfPeriod } from "./perfPeriodStorage";

describe("perfPeriodStorage", () => {
  afterEach(() => localStorage.clear());

  it("returns null when no preference is stored for the account", () => {
    expect(getPerfPeriod("acc-1")).toBeNull();
  });

  it("returns null for an empty account id", () => {
    expect(getPerfPeriod("")).toBeNull();
  });

  it("round-trips a stored period per account", () => {
    setPerfPeriod("acc-1", "ytd");
    setPerfPeriod("acc-2", "ten_years");
    expect(getPerfPeriod("acc-1")).toBe("ytd");
    expect(getPerfPeriod("acc-2")).toBe("ten_years");
  });

  it("overwrites a stored period (last write wins)", () => {
    setPerfPeriod("acc-1", "one_year");
    setPerfPeriod("acc-1", "five_years");
    expect(getPerfPeriod("acc-1")).toBe("five_years");
  });

  it("ignores a write with an empty account id", () => {
    setPerfPeriod("", "ytd");
    expect(getPerfPeriod("")).toBeNull();
  });

  it("treats an unrecognized stored value as no preference", () => {
    localStorage.setItem("perf_period_acc-1", "three_years");
    expect(getPerfPeriod("acc-1")).toBeNull();
  });

  it("accepts every allowed period value", () => {
    for (const period of [
      "since_start",
      "ytd",
      "one_year",
      "two_years",
      "five_years",
      "ten_years",
    ] as const) {
      setPerfPeriod("acc-1", period);
      expect(getPerfPeriod("acc-1")).toBe(period);
    }
  });
});
