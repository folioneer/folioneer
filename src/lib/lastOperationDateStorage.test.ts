import { afterEach, describe, expect, it } from "vitest";
import { getLastOperationDate, setLastOperationDate } from "./lastOperationDateStorage";

const todayIso = () => new Date().toISOString().slice(0, 10);

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
