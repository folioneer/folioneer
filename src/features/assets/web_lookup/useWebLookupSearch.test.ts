import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetLookupResult } from "@/bindings";
import { useWebLookupSearch } from "./useWebLookupSearch";

const mockLookupAsset = vi.fn();

vi.mock("../gateway", () => ({
  assetGateway: {
    lookupAsset: (...args: unknown[]) => mockLookupAsset(...args),
  },
}));

describe("useWebLookupSearch", () => {
  beforeEach(() => {
    mockLookupAsset.mockReset();
  });

  // WEB-011 — empty query submit is a no-op: state stays idle
  it("does not call the gateway and stays idle when query is empty", async () => {
    const { result } = renderHook(() => useWebLookupSearch());

    expect(result.current.state.status).toBe("idle");

    await act(async () => {
      result.current.submit("Isin");
    });

    expect(mockLookupAsset).not.toHaveBeenCalled();
    expect(result.current.state.status).toBe("idle");
  });

  // WEB-014 / WEB-030 — Isin mode dispatches to gateway with "Isin" and transitions through loading
  it("transitions idle → loading → results on successful ISIN search", async () => {
    // WEB-046 — on the ISIN path, `reference` carries the ticker from OpenFIGI
    // and `isin` carries the normalized query.
    const results: AssetLookupResult[] = [
      {
        name: "iShares Core S&P 500 UCITS ETF",
        reference: "CSPX",
        isin: "IE00B53L3W79",
        currency: "EUR",
        asset_class: "ETF",
        exchange: null,
      },
    ];
    mockLookupAsset.mockResolvedValue({ status: "ok", data: results });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("IE00B53L3W79");
    });

    await act(async () => {
      result.current.submit("Isin");
    });

    expect(mockLookupAsset).toHaveBeenCalledWith("IE00B53L3W79", "Isin");
    expect(result.current.state.status).toBe("results");
    if (result.current.state.status === "results") {
      expect(result.current.state.results).toEqual(results);
    }
  });

  // WEB-014 / WEB-030 — Keyword mode dispatches to gateway with "Keyword"
  it("transitions idle → loading → results on successful keyword search", async () => {
    const results: AssetLookupResult[] = [
      {
        name: "Apple Inc.",
        reference: "AAPL",
        isin: null,
        currency: "USD",
        asset_class: "Stocks",
        exchange: null,
      },
    ];
    mockLookupAsset.mockResolvedValue({ status: "ok", data: results });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("Apple");
    });

    await act(async () => {
      result.current.submit("Keyword");
    });

    expect(mockLookupAsset).toHaveBeenCalledWith("Apple", "Keyword");
    expect(result.current.state.status).toBe("results");
  });

  // WEB-032 — empty result list transitions to empty state
  it("transitions to empty state when gateway returns no results", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("unknown-instrument");
    });

    await act(async () => {
      result.current.submit("Keyword");
    });

    expect(result.current.state.status).toBe("empty");
  });

  // WEB-033 — NetworkError transitions to error state with code preserved
  it("transitions to error state when gateway returns NetworkError", async () => {
    mockLookupAsset.mockResolvedValue({
      status: "error",
      error: { code: "NetworkError" },
    });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("AAPL");
    });

    await act(async () => {
      result.current.submit("Keyword");
    });

    expect(result.current.state.status).toBe("error");
    if (result.current.state.status === "error") {
      expect(result.current.state.code).toBe("NetworkError");
    }
  });

  // WEB-025 / WEB-033 — InvalidIsinFormat on ISIN path transitions to error state
  it("transitions to error state with InvalidIsinFormat when ISIN format is invalid", async () => {
    mockLookupAsset.mockResolvedValue({
      status: "error",
      error: { code: "InvalidIsinFormat" },
    });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("NOTANISIN");
    });

    await act(async () => {
      result.current.submit("Isin");
    });

    expect(mockLookupAsset).toHaveBeenCalledWith("NOTANISIN", "Isin");
    expect(result.current.state.status).toBe("error");
    if (result.current.state.status === "error") {
      expect(result.current.state.code).toBe("InvalidIsinFormat");
    }
  });

  // WEB-033 — retry re-issues the last query with the last mode
  it("retry re-issues the last query and mode after an error", async () => {
    mockLookupAsset
      .mockResolvedValueOnce({
        status: "error",
        error: { code: "NetworkError" },
      })
      .mockResolvedValueOnce({
        status: "ok",
        data: [
          {
            name: "Apple Inc.",
            reference: "AAPL",
            isin: null,
            currency: "USD",
            asset_class: "Stocks",
            exchange: null,
          },
        ],
      });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("AAPL");
    });

    await act(async () => {
      result.current.submit("Keyword");
    });

    expect(result.current.state.status).toBe("error");

    await act(async () => {
      result.current.retry();
    });

    expect(mockLookupAsset).toHaveBeenCalledTimes(2);
    expect(mockLookupAsset).toHaveBeenNthCalledWith(2, "AAPL", "Keyword");
    expect(result.current.state.status).toBe("results");
  });

  // WEB-030 — submit while loading is ignored (no duplicate request)
  it("ignores a second submit while a search is already loading", async () => {
    let resolveFirst!: (v: unknown) => void;
    const firstCall = new Promise((resolve) => {
      resolveFirst = resolve;
    });
    mockLookupAsset.mockReturnValueOnce(firstCall);

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("AAPL");
    });

    act(() => {
      result.current.submit("Keyword");
    });

    act(() => {
      result.current.submit("Keyword");
    });

    await act(async () => {
      resolveFirst({ status: "ok", data: [] });
    });

    expect(mockLookupAsset).toHaveBeenCalledTimes(1);
  });

  // lastMode is exposed in state so components can anchor error/loading UI to the right field
  it("exposes lastMode in state after an ISIN submit", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("IE00B53L3W79");
    });

    await act(async () => {
      result.current.submit("Isin");
    });

    expect(result.current.lastMode).toBe("Isin");
  });

  it("exposes lastMode in state after a Keyword submit", async () => {
    mockLookupAsset.mockResolvedValue({ status: "ok", data: [] });

    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("Apple");
    });

    await act(async () => {
      result.current.submit("Keyword");
    });

    expect(result.current.lastMode).toBe("Keyword");
  });

  // Guard branch — retry before any submit is a no-op (lastMode is null)
  it("retry is a no-op when no prior submit was made (lastMode is null)", async () => {
    const { result } = renderHook(() => useWebLookupSearch());

    act(() => {
      result.current.setQuery("IE00B53L3W79");
    });

    await act(async () => {
      result.current.retry();
    });

    expect(mockLookupAsset).not.toHaveBeenCalled();
    expect(result.current.state.status).toBe("idle");
  });
});
