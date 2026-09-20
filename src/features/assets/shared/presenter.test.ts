import { describe, expect, it } from "vitest";
import { assetMutationErrorToI18n, formatAssetClass, getRiskBadgeClasses } from "./presenter";

describe("getRiskBadgeClasses", () => {
  // R11 — 5 distinct colours for risk levels 1–5
  it("returns a distinct class for each risk level 1–5", () => {
    const classes = [1, 2, 3, 4, 5].map(getRiskBadgeClasses);
    const unique = new Set(classes);
    expect(unique.size).toBe(5);
  });

  it("level 1 is green-toned", () => {
    expect(getRiskBadgeClasses(1)).toContain("green");
  });

  it("level 3 is orange-toned", () => {
    expect(getRiskBadgeClasses(3)).toContain("orange");
  });

  it("level 5 is red-toned", () => {
    expect(getRiskBadgeClasses(5)).toContain("red");
  });

  it("returns fallback class for out-of-range level", () => {
    expect(getRiskBadgeClasses(0)).toContain("gray");
    expect(getRiskBadgeClasses(6)).toContain("gray");
  });
});

describe("formatAssetClass", () => {
  // Identity t — lets us verify which i18n key is dispatched without a real i18n setup
  const t = (key: string) => key;

  // WEB-031 — compound class names map to their i18n keys
  it("maps compound class names to their i18n keys", () => {
    expect(formatAssetClass("RealEstate", t)).toBe("asset.class.RealEstate");
    expect(formatAssetClass("MutualFunds", t)).toBe("asset.class.MutualFunds");
    expect(formatAssetClass("DigitalAsset", t)).toBe("asset.class.DigitalAsset");
  });

  // WEB-031 — single-word and abbreviation classes map to their i18n keys
  it("maps single-word and abbreviation classes to their i18n keys", () => {
    expect(formatAssetClass("Cash", t)).toBe("asset.class.Cash");
    expect(formatAssetClass("Bonds", t)).toBe("asset.class.Bonds");
    expect(formatAssetClass("ETF", t)).toBe("asset.class.ETF");
    expect(formatAssetClass("ETP", t)).toBe("asset.class.ETP");
    expect(formatAssetClass("Stocks", t)).toBe("asset.class.Stocks");
    expect(formatAssetClass("Derivatives", t)).toBe("asset.class.Derivatives");
  });

  // Exhaustiveness — all 9 AssetClass variants dispatch to distinct non-empty keys
  it("covers all 9 AssetClass variants with distinct non-empty keys", () => {
    const all = [
      "Cash",
      "Bonds",
      "RealEstate",
      "MutualFunds",
      "ETF",
      "ETP",
      "Stocks",
      "DigitalAsset",
      "Derivatives",
    ] as const;
    const keys = all.map((c) => formatAssetClass(c, t));
    expect(keys.every((k) => k.trim().length > 0)).toBe(true);
    expect(new Set(keys).size).toBe(9);
  });
});

// F27 layer-3 presenter — exhaustive variant coverage. Each test pairs a code with
// its expected i18n key + vars; new variants added to the typed union without a
// presenter case will fail compile, not silently regress.
describe("assetMutationErrorToI18n", () => {
  it("InvalidExchange interpolates the exchange_code payload", () => {
    expect(assetMutationErrorToI18n({ code: "InvalidExchange", exchange_code: "BAR" })).toEqual({
      key: "error.InvalidExchange",
      vars: { exchange_code: "BAR" },
    });
  });

  it("InvalidCurrency interpolates the currency payload", () => {
    expect(assetMutationErrorToI18n({ code: "InvalidCurrency", currency: "ZZZ" })).toEqual({
      key: "error.InvalidCurrency",
      vars: { currency: "ZZZ" },
    });
  });

  it("InvalidRiskLevel maps to its flat key (payload not user-meaningful)", () => {
    expect(assetMutationErrorToI18n({ code: "InvalidRiskLevel", received: 99 })).toEqual({
      key: "error.InvalidRiskLevel",
    });
  });

  it("AssetNotFound maps to its flat key (id payload not surfaced)", () => {
    expect(assetMutationErrorToI18n({ code: "AssetNotFound", id: "asset-1" })).toEqual({
      key: "error.AssetNotFound",
    });
  });

  it("CategoryNotFound (cross-aggregate lookup) maps to its flat key", () => {
    expect(assetMutationErrorToI18n({ code: "CategoryNotFound", id: "cat-1" })).toEqual({
      key: "error.CategoryNotFound",
    });
  });

  it("AccountNotFound maps to its flat key (account_id payload not surfaced)", () => {
    expect(assetMutationErrorToI18n({ code: "AccountNotFound", account_id: "acc-1" })).toEqual({
      key: "error.AccountNotFound",
    });
  });

  it.each([
    "NameEmpty",
    "ReferenceEmpty",
    "InvalidIsinFormat",
    "Archived",
    "CashAssetNotEditable",
    "DatabaseError",
    "NameAlreadyExists",
    "ActiveHoldings",
    "ExistingTransactions",
    "DuplicateName",
  ] as const)("%s unit variant maps to its flat error key", (code) => {
    expect(assetMutationErrorToI18n({ code })).toEqual({ key: `error.${code}` });
  });

  it("an unreachable BC-wide code falls back to error.Unknown", () => {
    expect(assetMutationErrorToI18n({ code: "Oversell", available: 1, requested: 2 })).toEqual({
      key: "error.Unknown",
    });
  });
});
