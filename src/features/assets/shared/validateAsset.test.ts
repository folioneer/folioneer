import { describe, expect, it } from "vitest";
import type { Asset } from "@/bindings";
import { hasDuplicateReference } from "./validateAsset";

const makeAsset = (id: string, reference: string, is_archived = false): Asset => ({
  id,
  name: "Test",
  reference,
  isin: null,
  class: "Stocks",
  currency: "USD",
  risk_level: 4,
  category: { id: "cat-1", name: "Cat" },
  is_archived,
  price_refresh_blocked: false,
  interest_bearing: false,
  exchange: null,
});

const assets: Asset[] = [
  makeAsset("a1", "AAPL"),
  makeAsset("a2", "MSFT"),
  makeAsset("a3", "BND", true), // archived
];

describe("hasDuplicateReference", () => {
  // R9 — exact match
  it("returns true when reference matches exactly", () => {
    expect(hasDuplicateReference("AAPL", assets)).toBe(true);
  });

  // R9 — case-insensitive
  it("returns true for case-insensitive match", () => {
    expect(hasDuplicateReference("aapl", assets)).toBe(true);
  });

  // R9 — leading/trailing spaces trimmed
  it("returns true after trimming spaces", () => {
    expect(hasDuplicateReference("  AAPL  ", assets)).toBe(true);
  });

  // R9 — no match returns false
  it("returns false when no reference matches", () => {
    expect(hasDuplicateReference("GOOG", assets)).toBe(false);
  });

  // R9 — empty reference always returns false
  it("returns false for empty reference", () => {
    expect(hasDuplicateReference("", assets)).toBe(false);
  });

  // R9 — includes archived assets
  it("returns true for archived asset reference", () => {
    expect(hasDuplicateReference("BND", assets)).toBe(true);
  });

  // R9 — excludeId excludes self
  it("returns false when the only match is the excluded asset", () => {
    expect(hasDuplicateReference("AAPL", assets, "a1")).toBe(false);
  });

  // R9 — excludeId does not exclude other matches
  it("returns true when another asset (not excluded) matches", () => {
    const withDuplicate: Asset[] = [...assets, makeAsset("a4", "AAPL")];
    expect(hasDuplicateReference("AAPL", withDuplicate, "a1")).toBe(true);
  });
});
