import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetLookupResult } from "@/bindings";
import { useWebLookupModal } from "./useWebLookupModal";

const mockLookupAsset = vi.fn();

vi.mock("../gateway", () => ({
  assetGateway: {
    lookupAsset: (...args: unknown[]) => mockLookupAsset(...args),
  },
}));

const appleResult: AssetLookupResult = {
  name: "Apple Inc.",
  reference: "AAPL",
  isin: null,
  currency: "USD",
  asset_class: "Stocks",
  exchange: null,
};

const etfResult: AssetLookupResult = {
  name: "iShares Core S&P 500",
  reference: "IVV",
  isin: null,
  currency: "USD",
  asset_class: "ETF",
  exchange: null,
};

describe("useWebLookupModal", () => {
  beforeEach(() => {
    mockLookupAsset.mockReset();
  });

  // WEB-041 — a crypto result found from the listed kind opens on the crypto kind, and the
  // way back returns to the search of the listed kind.
  it("opens a result on the kind that offers its class", () => {
    const { result } = renderHook(() => useWebLookupModal());

    act(() => result.current.selectResult({ ...appleResult, asset_class: "DigitalAsset" }));
    expect(result.current.kind).toBe("Crypto");
    expect(result.current.modalStep.step).toBe("form-prefilled");

    act(() => result.current.back());
    expect(result.current.kind).toBe("Listed");
    expect(result.current.modalStep.step).toBe("search");

    act(() => result.current.selectResult(appleResult));
    expect(result.current.kind).toBe("Listed");
    act(() => result.current.selectResult({ ...appleResult, asset_class: null }));
    expect(result.current.kind).toBe("Listed");
  });

  // Initial state
  it("starts in the search step", () => {
    const { result } = renderHook(() => useWebLookupModal());
    expect(result.current.modalStep.step).toBe("search");
  });

  // WEB-040 — selecting a result transitions to form-prefilled
  it("selectResult transitions from search to form-prefilled with the selected result", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [appleResult] });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setIsinQuery("AAPL");
    });

    await act(async () => {
      result.current.submitSearch("Isin");
    });

    act(() => {
      result.current.selectResult(appleResult);
    });

    expect(result.current.modalStep.step).toBe("form-prefilled");
    if (result.current.modalStep.step === "form-prefilled") {
      expect(result.current.modalStep.selection).toEqual(appleResult);
    }
  });

  // WEB-013 — fillManually transitions to form-manual (no gateway call needed)
  it("fillManually transitions from search to form-manual", () => {
    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.fillManually();
    });

    expect(result.current.modalStep.step).toBe("form-manual");
  });

  // WEB-047 — back from form-prefilled restores search state (query + results retained)
  it("back from form-prefilled returns to search step with previous results retained", async () => {
    mockLookupAsset.mockResolvedValue({
      status: "ok",
      data: [appleResult, etfResult],
    });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setKeywordQuery("apple");
    });

    await act(async () => {
      result.current.submitSearch("Keyword");
    });

    act(() => {
      result.current.selectResult(appleResult);
    });

    expect(result.current.modalStep.step).toBe("form-prefilled");

    act(() => {
      result.current.back();
    });

    expect(result.current.modalStep.step).toBe("search");
    expect(result.current.keywordQuery).toBe("apple");
    expect(result.current.searchState.status).toBe("results");
    if (result.current.searchState.status === "results") {
      expect(result.current.searchState.results).toEqual([appleResult, etfResult]);
    }
  });

  // WEB-013 — back is NOT available from form-manual
  it("canGoBack is false when in form-manual step", () => {
    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.fillManually();
    });

    expect(result.current.modalStep.step).toBe("form-manual");
    expect(result.current.canGoBack).toBe(false);
  });

  // AST-040 — the dialog opens on the kind the core preselects, whose form starts from the
  // lookup; a kind without a lookup goes straight to its form, and coming back to a kind
  // with one returns to the search.
  it("opens on the search for a listed asset and on the form for the other kinds", () => {
    const { result } = renderHook(() => useWebLookupModal());
    expect(result.current.kind).toBe("Listed");
    expect(result.current.modalStep.step).toBe("search");

    act(() => {
      result.current.selectKind("Custom");
    });
    expect(result.current.kind).toBe("Custom");
    expect(result.current.modalStep.step).toBe("form-manual");
    expect(result.current.canGoBack).toBe(false);

    act(() => {
      result.current.selectKind("Crypto");
    });
    expect(result.current.modalStep.step).toBe("form-manual");

    act(() => {
      result.current.selectKind("Listed");
    });
    expect(result.current.modalStep.step).toBe("search");
  });

  // WEB-047 — back IS available from form-prefilled
  it("canGoBack is true when in form-prefilled step", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [appleResult] });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setIsinQuery("AAPL");
    });

    await act(async () => {
      result.current.submitSearch("Isin");
    });

    act(() => {
      result.current.selectResult(appleResult);
    });

    expect(result.current.canGoBack).toBe(true);
  });

  // WEB-040 — selecting a different result replaces the previous selection
  it("selecting a different result replaces all pre-filled values", async () => {
    mockLookupAsset.mockResolvedValue({
      status: "ok",
      data: [appleResult, etfResult],
    });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setKeywordQuery("apple");
    });

    await act(async () => {
      result.current.submitSearch("Keyword");
    });

    act(() => {
      result.current.selectResult(appleResult);
    });

    act(() => {
      result.current.back();
    });

    act(() => {
      result.current.selectResult(etfResult);
    });

    expect(result.current.modalStep.step).toBe("form-prefilled");
    if (result.current.modalStep.step === "form-prefilled") {
      expect(result.current.modalStep.selection).toEqual(etfResult);
    }
  });

  // WEB-011 / WEB-014 — ISIN submit dispatches to gateway with "Isin" mode
  it("submitSearch with Isin mode calls gateway with isinQuery and mode Isin", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setIsinQuery("IE00B53L3W79");
    });

    await act(async () => {
      result.current.submitSearch("Isin");
    });

    expect(mockLookupAsset).toHaveBeenCalledWith("IE00B53L3W79", "Isin");
  });

  // WEB-011 / WEB-014 — Keyword submit dispatches to gateway with "Keyword" mode
  it("submitSearch with Keyword mode calls gateway with keywordQuery and mode Keyword", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setKeywordQuery("Apple");
    });

    await act(async () => {
      result.current.submitSearch("Keyword");
    });

    expect(mockLookupAsset).toHaveBeenCalledWith("Apple", "Keyword");
  });

  // WEB-030 / WEB-033 — lastMode is tracked so SearchPanel can anchor loading/error to the right field
  it("lastMode is Isin after an ISIN submit", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setIsinQuery("IE00B53L3W79");
    });

    await act(async () => {
      result.current.submitSearch("Isin");
    });

    expect(result.current.lastMode).toBe("Isin");
  });

  it("lastMode is Keyword after a Keyword submit", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setKeywordQuery("Apple");
    });

    await act(async () => {
      result.current.submitSearch("Keyword");
    });

    expect(result.current.lastMode).toBe("Keyword");
  });

  // WEB-025 / WEB-033 — InvalidIsinFormat error is surfaced in searchState
  it("searchState carries InvalidIsinFormat code when ISIN path rejects the query", async () => {
    mockLookupAsset.mockResolvedValue({
      status: "error",
      error: { code: "InvalidIsinFormat" },
    });

    const { result } = renderHook(() => useWebLookupModal());

    act(() => {
      result.current.setIsinQuery("NOTANISIN");
    });

    await act(async () => {
      result.current.submitSearch("Isin");
    });

    expect(result.current.searchState.status).toBe("error");
    if (result.current.searchState.status === "error") {
      expect(result.current.searchState.code).toBe("InvalidIsinFormat");
    }
  });
});
