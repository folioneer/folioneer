import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AccountSummary, PortfolioTotal } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAccountSummaries } from "./useAccountSummaries";

const mockGetAccountSummaries = vi.fn();
const mockSubscribeToEvents = vi.fn<(cb: (type: string) => void) => Promise<() => void>>(() =>
  Promise.resolve(() => {}),
);

vi.mock("./gateway", () => ({
  accountGateway: {
    getAccountSummaries: () => mockGetAccountSummaries(),
    subscribeToEvents: (cb: (type: string) => void) => mockSubscribeToEvents(cb),
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

const makeSummary = (overrides: Partial<AccountSummary> = {}): AccountSummary => ({
  id: "acc-1",
  name: "Main",
  currency: "EUR",
  update_frequency: "ManualMonth",
  total_global_value: 100_000_000,
  total_unrealized_pnl: null,
  ytd_performance_pct: null,
  has_inconsistent_holding: false,
  ...overrides,
});

describe("useAccountSummaries", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSubscribeToEvents.mockImplementation(() => Promise.resolve(() => {}));
  });

  // ACC-021 — happy path: gateway returns list → summaries state populated, isLoading cleared
  it("populates summaries from gateway result on mount", async () => {
    const summaries = [makeSummary(), makeSummary({ id: "acc-2", name: "Side" })];
    const total: PortfolioTotal = {
      total_global_value: 200_000_000,
      total_unrealized_pnl: null,
      currency: "EUR",
      incomplete: false,
    };
    mockGetAccountSummaries.mockResolvedValue({ status: "ok", data: { summaries, total } });

    const { result } = renderHook(() => useAccountSummaries());
    await act(async () => {});

    expect(result.current.summaries).toEqual(summaries);
    // ACC-027 — the backend's portfolio total is exposed as given, never recomputed.
    expect(result.current.portfolioTotal).toEqual(total);
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  // Typed-error path — DatabaseError surfaces via presenter
  it("maps backend DatabaseError to error.DatabaseError and clears isLoading", async () => {
    mockGetAccountSummaries.mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => useAccountSummaries());
    await act(async () => {});

    expect(result.current.error).toEqual({ key: "error.DatabaseError" });
    expect(result.current.isLoading).toBe(false);
    expect(result.current.summaries).toEqual([]);
    expect(result.current.portfolioTotal).toBeNull();
  });

  // Throw path — gateway rejection falls back to UNKNOWN_ERROR
  it("falls back to UNKNOWN_ERROR when gateway throws", async () => {
    mockGetAccountSummaries.mockRejectedValue(new Error("boom"));

    const { result } = renderHook(() => useAccountSummaries());
    await act(async () => {});

    expect(result.current.error).toEqual({ key: "error.Unknown" });
    expect(result.current.isLoading).toBe(false);
    expect(logger.error).toHaveBeenCalledWith("[useAccountSummaries] fetch threw", {
      error: expect.any(Error),
    });
  });

  // ACC-021 — re-fetches when AccountUpdated / AssetPriceUpdated events fire
  it("re-fetches summaries when relevant events arrive", async () => {
    let capturedCallback: ((type: string) => void) | null = null;
    mockSubscribeToEvents.mockImplementation((cb: (type: string) => void) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    mockGetAccountSummaries.mockResolvedValue({
      status: "ok",
      data: {
        summaries: [],
        total: {
          total_global_value: 0,
          total_unrealized_pnl: null,
          currency: "EUR",
          incomplete: false,
        },
      },
    });

    renderHook(() => useAccountSummaries());
    await act(async () => {});
    const beforeCount = mockGetAccountSummaries.mock.calls.length;

    await act(async () => {
      capturedCallback?.("AssetPriceUpdated");
    });

    expect(mockGetAccountSummaries.mock.calls.length).toBeGreaterThan(beforeCount);
  });

  // #020 / SYN-064 — changes applied from another device arrive as one SyncCompleted.
  it("re-fetches summaries once when a sync completes", async () => {
    let capturedCallback: ((type: string) => void) | null = null;
    mockSubscribeToEvents.mockImplementation((cb: (type: string) => void) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    mockGetAccountSummaries.mockResolvedValue({
      status: "ok",
      data: {
        summaries: [],
        total: {
          total_global_value: 0,
          total_unrealized_pnl: null,
          currency: "EUR",
          incomplete: false,
        },
      },
    });

    renderHook(() => useAccountSummaries());
    await act(async () => {});
    const beforeCount = mockGetAccountSummaries.mock.calls.length;

    await act(async () => {
      capturedCallback?.("SyncCompleted");
    });

    expect(mockGetAccountSummaries.mock.calls.length).toBe(beforeCount + 1);
  });

  // ACC-033 — a rate or pair change moves the converted values and the total.
  it.each([
    "CurrencyRateUpdated",
    "CurrencyPairUpdated",
  ])("re-fetches summaries when %s arrives", async (eventType) => {
    let capturedCallback: ((type: string) => void) | null = null;
    mockSubscribeToEvents.mockImplementation((cb: (type: string) => void) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    mockGetAccountSummaries.mockResolvedValue({
      status: "ok",
      data: {
        summaries: [],
        total: {
          total_global_value: 0,
          total_unrealized_pnl: null,
          currency: "EUR",
          incomplete: false,
        },
      },
    });

    renderHook(() => useAccountSummaries());
    await act(async () => {});
    const beforeCount = mockGetAccountSummaries.mock.calls.length;

    await act(async () => {
      capturedCallback?.(eventType);
    });

    expect(mockGetAccountSummaries.mock.calls.length).toBeGreaterThan(beforeCount);
  });

  // Unrelated events do NOT trigger a re-fetch (cheap noise filter)
  it("ignores unrelated event types", async () => {
    let capturedCallback: ((type: string) => void) | null = null;
    mockSubscribeToEvents.mockImplementation((cb: (type: string) => void) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    mockGetAccountSummaries.mockResolvedValue({
      status: "ok",
      data: {
        summaries: [],
        total: {
          total_global_value: 0,
          total_unrealized_pnl: null,
          currency: "EUR",
          incomplete: false,
        },
      },
    });

    renderHook(() => useAccountSummaries());
    await act(async () => {});
    const beforeCount = mockGetAccountSummaries.mock.calls.length;

    await act(async () => {
      capturedCallback?.("SomethingUnrelated");
    });

    expect(mockGetAccountSummaries.mock.calls.length).toBe(beforeCount);
  });
});

describe("useAccountSummaries — bulk-fetch coalescing (MKT-181)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("skips AssetPriceUpdated during an active fetch, reloads on completion", async () => {
    const { useAppStore } = await import("@/lib/store");
    let capturedCallback: ((type: string) => void) | null = null;
    mockSubscribeToEvents.mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    mockGetAccountSummaries.mockResolvedValue({
      status: "ok",
      data: {
        summaries: [makeSummary()],
        total: {
          total_global_value: 0,
          total_unrealized_pnl: null,
          currency: "EUR",
          incomplete: false,
        },
      },
    });

    useAppStore.setState({ priceFetch: { active: true, done: 1, total: 3 } });
    renderHook(() => useAccountSummaries());
    await act(async () => {});
    const initialCalls = mockGetAccountSummaries.mock.calls.length;

    await act(async () => {
      capturedCallback?.("AssetPriceUpdated");
    });
    expect(mockGetAccountSummaries.mock.calls.length).toBe(initialCalls);

    useAppStore.setState({ priceFetch: { active: false, done: 0, total: 0 } });
    await act(async () => {
      capturedCallback?.("AssetPriceFetchCompleted");
    });
    expect(mockGetAccountSummaries.mock.calls.length).toBe(initialCalls + 1);
  });
});
