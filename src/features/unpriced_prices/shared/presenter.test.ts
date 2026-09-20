import { describe, expect, it } from "vitest";
import { recordPriceErrorToI18n } from "./presenter";

describe("recordPriceErrorToI18n (F27)", () => {
  it("maps a payload-free error code to error.<code>", () => {
    expect(recordPriceErrorToI18n({ code: "NotPositive" })).toEqual({ key: "error.NotPositive" });
    expect(recordPriceErrorToI18n({ code: "DatabaseError" })).toEqual({
      key: "error.DatabaseError",
    });
  });

  it("carries the offending date for InvalidDateFormat", () => {
    expect(recordPriceErrorToI18n({ code: "InvalidDateFormat", date: "2026/06/19" })).toEqual({
      key: "error.InvalidDateFormat",
      vars: { date: "2026/06/19" },
    });
  });

  it("carries the id for AssetNotFound", () => {
    expect(recordPriceErrorToI18n({ code: "AssetNotFound", id: "asset-9" })).toEqual({
      key: "error.AssetNotFound",
    });
  });
});
