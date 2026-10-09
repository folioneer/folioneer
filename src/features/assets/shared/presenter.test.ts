import { describe, expect, it } from "vitest";
import type { Asset, AssetKind } from "@/bindings";
import {
  assetKindLabelKey,
  assetKindSaysKey,
  assetMutationErrorToI18n,
  currencyLabelKey,
  formatAssetClass,
  getRiskBadgeClasses,
  presentOtherListings,
  referenceLabelKey,
} from "./presenter";

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

  // AST-032 — the refusal of an asset that already exists names it.
  it("AssetAlreadyExists names the existing asset", () => {
    expect(
      assetMutationErrorToI18n({
        code: "AssetAlreadyExists",
        existing_id: "a1",
        existing_name: "ASML Holding",
      }),
    ).toEqual({ key: "error.AssetAlreadyExists", vars: { existing_name: "ASML Holding" } });
  });

  // AST-041 — when the existing asset is known, the refusal says what tells it apart.
  it("AssetAlreadyExists adds the reference, exchange and currency of a known asset", () => {
    const existing = {
      id: "a1",
      name: "ASML Holding",
      reference: "ASML",
      exchange: { code: "XAMS", label: "Euronext Amsterdam" },
      currency: "EUR",
    } as Asset;
    expect(
      assetMutationErrorToI18n(
        { code: "AssetAlreadyExists", existing_id: "a1", existing_name: "ASML Holding" },
        [existing],
      ),
    ).toEqual({
      key: "error.AssetAlreadyExists",
      vars: { existing_name: "ASML Holding (ASML · Euronext Amsterdam · EUR)" },
    });
  });

  // AST-031 — what a kind forbids has its own message, never the unknown one.
  it("maps what a kind forbids to its own message", () => {
    expect(assetMutationErrorToI18n({ code: "IsinRequired" })).toEqual({
      key: "error.IsinRequired",
    });
    expect(assetMutationErrorToI18n({ code: "IsinNotAllowed", kind: "Custom" })).toEqual({
      key: "error.IsinNotAllowed",
    });
    expect(assetMutationErrorToI18n({ code: "ExchangeNotAllowed", kind: "Crypto" })).toEqual({
      key: "error.ExchangeNotAllowed",
    });
    expect(
      assetMutationErrorToI18n({ code: "ClassNotAllowed", kind: "Listed", class: "DigitalAsset" }),
    ).toEqual({ key: "error.ClassNotAllowed" });
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

describe("kinds and listings", () => {
  // AST-040 — every kind has a name, a line saying what it means, and a label for its
  // reference and its currency.
  it("gives every kind its own keys", () => {
    const kinds: AssetKind[] = ["Listed", "Crypto", "Custom", "Cash"];
    expect(new Set(kinds.map(assetKindLabelKey)).size).toBe(4);
    expect(new Set(kinds.map(assetKindSaysKey)).size).toBe(4);
    expect(referenceLabelKey("Listed")).toBe("asset.form_reference_label_listed");
    expect(referenceLabelKey("Crypto")).toBe("asset.form_reference_label_crypto");
    expect(referenceLabelKey("Custom")).toBe("asset.form_reference_label");
    expect(referenceLabelKey("Cash")).toBe("asset.form_reference_label");
    expect(currencyLabelKey("Crypto")).toBe("asset.form_currency_label_crypto");
    expect(currencyLabelKey("Listed")).toBe("asset.form_currency_label");
  });

  // AST-039 — one other listing shows in full; several show the first and an ellipsis,
  // the whole list being the hint; none shows nothing.
  it("presents the other listings of an instrument", () => {
    const nasdaq = {
      asset_id: "n",
      reference: "ASML",
      exchange: { code: "XNAS", label: "Nasdaq" },
      currency: "USD",
    };
    const otc = { asset_id: "o", reference: "ASMLF", exchange: null, currency: "USD" };

    expect(presentOtherListings([])).toBeNull();
    expect(presentOtherListings([nasdaq])).toEqual({
      shown: "ASML · Nasdaq · USD",
      hint: "ASML · Nasdaq · USD",
    });
    expect(presentOtherListings([nasdaq, otc])).toEqual({
      shown: "ASML · Nasdaq · USD, …",
      hint: "ASML · Nasdaq · USD\nASMLF · USD",
    });
  });
});
