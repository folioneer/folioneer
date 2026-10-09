import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { todayIso } from "@/ui/format/date";
import { getLastOperationDate, setLastOperationDate } from "./lastOperationDateStorage";

describe("lastOperationDateStorage", () => {
  afterEach(() => localStorage.clear());

  it("falls back to today when no date is stored for the account", () => {
    expect(getLastOperationDate("acc-1")).toBe(todayIso());
  });

  it("falls back to today for an empty account id", () => {
    expect(getLastOperationDate("")).toBe(todayIso());
  });

  it("round-trips a stored date per account", () => {
    setLastOperationDate("acc-1", "2018-03-01");
    setLastOperationDate("acc-2", "2024-09-15");
    expect(getLastOperationDate("acc-1")).toBe("2018-03-01");
    expect(getLastOperationDate("acc-2")).toBe("2024-09-15");
  });

  it("ignores a non-ISO date on write", () => {
    setLastOperationDate("acc-1", "01/03/2018");
    expect(getLastOperationDate("acc-1")).toBe(todayIso());
  });

  it("ignores a write with an empty account id", () => {
    setLastOperationDate("", "2018-03-01");
    expect(getLastOperationDate("")).toBe(todayIso());
  });

  it("ignores a stored value that is not a well-formed ISO date", () => {
    localStorage.setItem("last_operation_date_acc-1", "garbage");
    expect(getLastOperationDate("acc-1")).toBe(todayIso());
  });
});

describe("getLastOperationDate just after local midnight", () => {
  // 00:30 in Paris on 9 October is still the 8th in UTC.
  beforeEach(() => {
    vi.stubEnv("TZ", "Europe/Paris");
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-08T22:30:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllEnvs();
  });

  it("falls back to today as the user lives it", () => {
    expect(getLastOperationDate("an-account-with-no-operation")).toBe("2026-10-09");
  });
});
