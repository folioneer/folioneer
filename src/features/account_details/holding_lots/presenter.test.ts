import { describe, expect, it } from "vitest";
import { lotsErrorToI18n, toLotRows } from "./presenter";

describe("toLotRows (#515, the validated mock; CBM-034)", () => {
  // A lot taken whole is dimmed; free shares are named; figures formatted.
  it("dims a lot taken whole and names free shares", () => {
    const rows = toLotRows(
      [
        {
          acquired_on: "2024-02-01",
          quantity: "10",
          cost: 1_000_000_000,
          remaining: "0",
          free: false,
        },
        { acquired_on: "2024-03-15", quantity: "2", cost: 0, remaining: "2", free: true },
      ],
      "en-US",
    );
    expect(rows.map((row) => [row.acquiredOn, row.spent, row.free])).toEqual([
      ["2/1/2024", true, false],
      ["3/15/2024", false, true],
    ]);
    expect(rows[0]?.cost).toContain("1");
    expect(rows[1]?.remaining).toBe("2");
  });
});

describe("lotsErrorToI18n (CBM-034)", () => {
  // An unknown account and a database failure keep their reason; anything else is unknown.
  it("names the reason the lots were not read", () => {
    expect(lotsErrorToI18n({ code: "AccountNotFound", account_id: "x" })).toEqual({
      key: "error.AccountNotFound",
    });
    expect(lotsErrorToI18n({ code: "DatabaseError" })).toEqual({ key: "error.DatabaseError" });
    expect(lotsErrorToI18n({ code: "NameEmpty" })).toEqual({ key: "error.Unknown" });
  });
});
