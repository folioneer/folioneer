import { describe, expect, it } from "vitest";
import { computeDayDelta, formatStalenessLabel } from "./staleness";

const TODAY = new Date(2026, 4, 17); // 2026-05-17, local time

describe("computeDayDelta", () => {
  it("returns null for a null date", () => {
    expect(computeDayDelta(null, TODAY)).toBeNull();
  });

  it("returns null for an unparseable date", () => {
    expect(computeDayDelta("not-a-date", TODAY)).toBeNull();
  });

  it("returns 0 when the date is today", () => {
    expect(computeDayDelta("2026-05-17", TODAY)).toBe(0);
  });

  // computeDayDelta returns the raw signed delta; treating <= 0 as "today" is
  // formatStalenessLabel's job, not this function's.
  it("returns a negative delta for a future date", () => {
    expect(computeDayDelta("2026-05-20", TODAY)).toBe(-3);
  });

  it("returns the whole-day delta for a past date", () => {
    expect(computeDayDelta("2026-05-10", TODAY)).toBe(7);
  });
});

const KEYS = { today: "test.today", daysAgo: "test.days_ago" };

describe("formatStalenessLabel", () => {
  it("returns null for a null date", () => {
    expect(formatStalenessLabel(null, TODAY, KEYS)).toBeNull();
  });

  it("returns null for an unparseable date", () => {
    expect(formatStalenessLabel("not-a-date", TODAY, KEYS)).toBeNull();
  });

  it("returns the today key (no params) for same-day and future dates", () => {
    expect(formatStalenessLabel("2026-05-17", TODAY, KEYS)).toEqual({ key: "test.today" });
    expect(formatStalenessLabel("2026-05-20", TODAY, KEYS)).toEqual({ key: "test.today" });
  });

  it("returns the daysAgo key with the day delta for a past date", () => {
    expect(formatStalenessLabel("2026-05-16", TODAY, KEYS)).toEqual({
      key: "test.days_ago",
      params: { days: 1 },
    });
    expect(formatStalenessLabel("2026-05-10", TODAY, KEYS)).toEqual({
      key: "test.days_ago",
      params: { days: 7 },
    });
  });
});
