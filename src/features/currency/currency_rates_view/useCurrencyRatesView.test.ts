import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrencyPairSummary, CurrencyRate, Event } from "@/bindings";
import * as gateway from "../gateway";
import { useCurrencyRatesView } from "./useCurrencyRatesView";

vi.mock("../gateway");

const PAIR: CurrencyPairSummary = {
  from_currency: "USD",
  to_currency: "EUR",
  latest_rate: 920_000,
  latest_rate_date: "2026-06-01",
  latest_rate_source: "Manual",
};
const RATE: CurrencyRate = {
  from_currency: "USD",
  to_currency: "EUR",
  date: "2026-06-01",
  rate: 920_000,
  source: "Manual",
};

describe("useCurrencyRatesView", () => {
  let eventCallback: ((type: Event["type"]) => void) | undefined;
  const unlisten = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    eventCallback = undefined;
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({ status: "ok", data: [PAIR] });
    vi.mocked(gateway.getCurrencyRates).mockResolvedValue({ status: "ok", data: [RATE] });
    vi.mocked(gateway.subscribeToEvents).mockImplementation(async (cb) => {
      eventCallback = cb;
      return unlisten;
    });
  });

  // FXR-051 — pairs load on mount
  it("loads pairs on mount and clears loading", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.pairs).toEqual([PAIR]);
    expect(result.current.error).toBeNull();
  });

  // FXR-051 — pair-load failure surfaces an i18n error
  it("surfaces an i18n error when the pair load fails", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.error).not.toBeNull();
  });

  // FXR-050 — drill-in loads the pair's rate history
  it("selectPair loads the pair's rates", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.selectPair("USD", "EUR"));
    await waitFor(() => expect(result.current.rates).toEqual([RATE]));
    expect(result.current.selectedPair).toEqual({ fromCurrency: "USD", toCurrency: "EUR" });
  });

  // FXR-050 — a failed rate load surfaces a ratesError without dropping the selection
  it("surfaces a ratesError when the rate load fails", async () => {
    vi.mocked(gateway.getCurrencyRates).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.selectPair("USD", "EUR"));
    await waitFor(() => expect(result.current.ratesError).not.toBeNull());
    expect(result.current.selectedPair).toEqual({ fromCurrency: "USD", toCurrency: "EUR" });
  });

  // FXR-026/037 — a CurrencyRateUpdated event re-fetches pairs and the selected pair's rates
  it("re-fetches pairs and selected rates on CurrencyRateUpdated", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.selectPair("USD", "EUR"));
    await waitFor(() => expect(gateway.getCurrencyRates).toHaveBeenCalledTimes(1));

    vi.mocked(gateway.getCurrencyPairs).mockClear();
    vi.mocked(gateway.getCurrencyRates).mockClear();
    act(() => eventCallback?.("CurrencyRateUpdated"));

    await waitFor(() => {
      expect(gateway.getCurrencyPairs).toHaveBeenCalledTimes(1);
      expect(gateway.getCurrencyRates).toHaveBeenCalledWith("USD", "EUR");
    });
  });

  // FXR-054/SYN-064 — a pair declared here announces itself; pairs and rates applied from
  // another device arrive as one SyncCompleted per sync (#020).
  it("re-fetches pairs on CurrencyPairUpdated and once on SyncCompleted", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.selectPair("USD", "EUR"));
    await waitFor(() => expect(gateway.getCurrencyRates).toHaveBeenCalledTimes(1));

    vi.mocked(gateway.getCurrencyPairs).mockClear();
    vi.mocked(gateway.getCurrencyRates).mockClear();
    act(() => eventCallback?.("SyncCompleted"));
    await waitFor(() => {
      expect(gateway.getCurrencyPairs).toHaveBeenCalledTimes(1);
      expect(gateway.getCurrencyRates).toHaveBeenCalledWith("USD", "EUR");
    });

    act(() => eventCallback?.("CurrencyPairUpdated"));
    await waitFor(() => expect(gateway.getCurrencyPairs).toHaveBeenCalledTimes(2));
  });

  // FXR-037 — unrelated events do not trigger a re-fetch
  it("ignores unrelated events", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    vi.mocked(gateway.getCurrencyPairs).mockClear();
    act(() => eventCallback?.("AssetUpdated"));
    expect(gateway.getCurrencyPairs).not.toHaveBeenCalled();
  });

  // FXR-050 — clearSelection resets the drill-in state
  it("clearSelection resets the selected pair, rates, and ratesError", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.selectPair("USD", "EUR"));
    await waitFor(() => expect(result.current.rates).toEqual([RATE]));
    act(() => result.current.clearSelection());
    expect(result.current.selectedPair).toBeNull();
    expect(result.current.rates).toEqual([]);
    expect(result.current.ratesError).toBeNull();
  });

  // FXR-051 — refetch reloads the pair list
  it("refetch reloads the pairs", async () => {
    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    vi.mocked(gateway.getCurrencyPairs).mockClear();
    act(() => result.current.refetch());
    await waitFor(() => expect(gateway.getCurrencyPairs).toHaveBeenCalledTimes(1));
  });

  // unsubscribes from the event bus on unmount
  it("unsubscribes on unmount", async () => {
    const { unmount } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(gateway.subscribeToEvents).toHaveBeenCalled());
    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  // FXR-110 — a successful backfill resolves with the written count and
  // refreshes the pair list (and the drilled-in history when one is open).
  it("backfillHistory resolves ok with the written count and refetches", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({ status: "ok", data: [PAIR] });
    vi.mocked(gateway.getCurrencyRates).mockResolvedValue({ status: "ok", data: [RATE] });
    vi.mocked(gateway.backfillCurrencyRateHistory).mockResolvedValue({
      status: "ok",
      data: 42,
    });

    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    let outcome: Awaited<ReturnType<typeof result.current.backfillHistory>> | undefined;
    await act(async () => {
      outcome = await result.current.backfillHistory();
    });

    expect(outcome).toEqual({ status: "ok", ratesWritten: 42 });
    expect(gateway.getCurrencyPairs).toHaveBeenCalledTimes(2); // mount + post-backfill
    expect(result.current.isBackfilling).toBe(false);
  });

  // FXR-114 — an unreachable provider surfaces as a presented message.
  it("backfillHistory resolves an error message when the provider is unreachable", async () => {
    vi.mocked(gateway.getCurrencyPairs).mockResolvedValue({ status: "ok", data: [PAIR] });
    vi.mocked(gateway.backfillCurrencyRateHistory).mockResolvedValue({
      status: "error",
      error: { code: "ProviderUnreachable" },
    });

    const { result } = renderHook(() => useCurrencyRatesView());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    let outcome: Awaited<ReturnType<typeof result.current.backfillHistory>> | undefined;
    await act(async () => {
      outcome = await result.current.backfillHistory();
    });

    expect(outcome).toEqual({
      status: "error",
      message: { key: "error.currency.ProviderUnreachable" },
    });
    expect(gateway.getCurrencyPairs).toHaveBeenCalledTimes(1); // mount only — no refetch
  });
});
