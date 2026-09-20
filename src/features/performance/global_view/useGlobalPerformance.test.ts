import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  Account,
  AccountDetailsResponse,
  AccountPerformanceResponse,
  Asset,
  Event,
  HoldingDetail,
  PerformancePeriod,
} from "@/bindings";
import { useAppStore } from "@/lib/store";

// Mock the gateway so no real Tauri calls fire (docs/test_convention.md § Mocking gateway modules)
vi.mock("../gateway");

const { mockShowSnackbar } = vi.hoisted(() => ({ mockShowSnackbar: vi.fn() }));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

// Identity i18n — t(key) === key so tests assert on stable keys (F24).
// t must be referentially stable across renders (like the real memoized t):
// it sits in effect dependency lists, and a fresh function per render would
// re-run those effects forever.
vi.mock("react-i18next", () => {
  const t = (key: string) => key;
  return {
    useTranslation: () => ({ t, i18n: { language: "en" } }),
  };
});

import * as gateway from "../gateway";
import { useGlobalPerformance } from "./useGlobalPerformance";

// ---- Fixtures ---------------------------------------------------------------

const makeYearRow = (overrides: Partial<PerformancePeriod> = {}): PerformancePeriod => ({
  year: 2025,
  month: null,
  end_value: 10_000_000_000,
  previous_value: 9_000_000_000,
  cash_flow: 500_000_000,
  asset_flow: 0,
  dividends: 120_000_000,
  pnl: 380_000_000,
  period_over_period: { gain: 500_000_000, pct: 5_000_000 },
  year_to_date: null,
  since_inception: { gain: 500_000_000, pct: 5_000_000 },
  annualized_yield: { gain: 500_000_000, pct: 5_000_000 },
  ...overrides,
});

const makeResponse = (
  overrides: Partial<AccountPerformanceResponse> = {},
): AccountPerformanceResponse => ({
  account_name: "",
  currency: "EUR",
  month_view_available: false,
  yearly: [makeYearRow()],
  monthly: [],
  ...overrides,
});

const makeAccount = (overrides: Partial<Account> = {}): Account => ({
  id: "account-1",
  name: "Broker One",
  bank_name: "",
  currency: "EUR",
  update_frequency: "ManualMonth",
  management_fees_enabled: false,
  ...overrides,
});

const makeCatalogAsset = (overrides: Partial<Asset> = {}): Asset => ({
  id: "asset-1",
  name: "Apple Inc",
  reference: "AAPL",
  isin: null,
  class: "Stocks",
  currency: "USD",
  risk_level: 4,
  category: { id: "cat-1", name: "US Stocks" },
  is_archived: false,
  price_refresh_blocked: false,
  interest_bearing: false,
  exchange: null,
  ...overrides,
});

const makeHolding = (overrides: Partial<HoldingDetail> = {}): HoldingDetail => ({
  asset_id: "asset-1",
  asset_name: "Apple Inc",
  asset_reference: "AAPL",
  quantity: 2_000_000,
  average_price: 100_000_000,
  cost_basis: 200_000_000,
  realized_pnl: 0,
  asset_currency: "EUR",
  current_price: null,
  current_price_date: null,
  current_price_source: null,
  unrealized_pnl: null,
  performance_pct: null,
  dividends_received: 0,
  total_return_pct: null,
  fx_rate_date: null,
  management_fees: 0,
  market_value: null,
  fee_rate_percent_micros: null,
  note_text: null,
  note_threshold_price: null,
  note_threshold_direction: null,
  note_alarm_triggered: false,
  inconsistency: null,
  period_performance: {
    ytd: null,
    one_year: null,
    two_years: null,
    five_years: null,
    ten_years: null,
  },
  ...overrides,
});

const makeDetailsResponse = (
  overrides: Partial<AccountDetailsResponse> = {},
): AccountDetailsResponse => ({
  account_name: "Broker One",
  holdings: [makeHolding({ asset_id: "system-cash-EUR", asset_name: "Cash" }), makeHolding()],
  closed_holdings: [],
  total_holding_count: 2,
  total_cost_basis: 200_000_000,
  total_realized_pnl: 0,
  total_unrealized_pnl: null,
  total_global_value: 0,
  total_dividends_received: 0,
  total_management_fees: 0,
  total_net_cash_input: 0,
  ...overrides,
});

// ---- Tests ------------------------------------------------------------------

describe("useGlobalPerformance", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    useAppStore.setState({
      accounts: [
        makeAccount({ id: "account-2", name: "Zeta Bank" }),
        makeAccount({ id: "account-1", name: "Broker One" }),
      ],
      assets: [
        makeCatalogAsset({ id: "asset-2", name: "Microsoft Corp", reference: "MSFT" }),
        makeCatalogAsset(),
        makeCatalogAsset({ id: "asset-3", name: "Old Fund", is_archived: true }),
        makeCatalogAsset({ id: "system-cash-EUR", name: "Cash" }),
      ],
    });
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });
    vi.mocked(gateway.globalPerformanceGateway.getAccountHoldings).mockResolvedValue({
      status: "ok",
      data: makeDetailsResponse(),
    });
    vi.mocked(gateway.globalPerformanceGateway.subscribeToEvents).mockResolvedValue(() => {});
  });

  // GPF-010 — default scope is the whole portfolio: (null, null)
  it("defaults to all accounts and all assets (GPF-010)", async () => {
    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.selectedAccountId).toBeNull();
    expect(result.current.selectedAssetId).toBeNull();
    expect(result.current.scopeLabel).toBeNull();
    expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(null, null);
  });

  // GPF-011 — the reporting currency of the response is exposed
  it("exposes the response currency (GPF-011)", async () => {
    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() => expect(result.current.currency).toBe("EUR"));
  });

  // The account selector offers every catalog account, name asc
  it("exposes the accounts catalog as account options, name asc", async () => {
    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() =>
      expect(result.current.accountOptions).toEqual([
        { accountId: "account-1", accountName: "Broker One" },
        { accountId: "account-2", accountName: "Zeta Bank" },
      ]),
    );
  });

  // All-accounts scope — the asset selector offers the non-archived non-cash catalog, name asc
  it("offers the non-archived non-cash catalog assets when unscoped", async () => {
    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() =>
      expect(result.current.assetOptions).toEqual([
        { assetId: "asset-1", assetName: "Apple Inc" },
        { assetId: "asset-2", assetName: "Microsoft Corp" },
      ]),
    );
  });

  // #020 / SYN-064 — changes applied from another device arrive as one SyncCompleted.
  it("re-fetches once when a sync completes", async () => {
    let capturedCallback: ((type: Event["type"]) => void) | null = null;
    vi.mocked(gateway.globalPerformanceGateway.subscribeToEvents).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    const before = vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mock.calls
      .length;

    await act(async () => {
      capturedCallback?.("SyncCompleted");
    });

    expect(vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mock.calls.length).toBe(
      before + 1,
    );
  });

  // GPF-010 — selecting an account re-fetches with the account scope
  it("re-fetches with the account id when an account scope is selected (GPF-010)", async () => {
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));

    await waitFor(() =>
      expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
        "account-1",
        null,
      ),
    );
    expect(result.current.selectedAccountId).toBe("account-1");
    await waitFor(() => expect(result.current.scopeLabel).toBe("Broker One"));
  });

  // Account scope — the asset selector switches to the account's non-cash holdings
  it("offers the scoped account's non-cash holdings as asset options", async () => {
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));

    await waitFor(() =>
      expect(result.current.assetOptions).toEqual([{ assetId: "asset-1", assetName: "Apple Inc" }]),
    );
    expect(gateway.globalPerformanceGateway.getAccountHoldings).toHaveBeenCalledWith("account-1");
  });

  // GPF-010 — selecting an asset re-fetches with both scope ids
  it("re-fetches with account and asset ids when both scopes are selected (GPF-010)", async () => {
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));
    await waitFor(() => expect(result.current.selectedAccountId).toBe("account-1"));

    act(() => result.current.setSelectedAssetId("asset-1"));

    await waitFor(() =>
      expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
        "account-1",
        "asset-1",
      ),
    );
    await waitFor(() => expect(result.current.scopeLabel).toBe("Broker One — Apple Inc"));
  });

  // GPF-010 — an asset scope without an account scope reads the asset across accounts
  it("re-fetches with the asset id alone for the cross-account asset scope (GPF-010)", async () => {
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAssetId("asset-2"));

    await waitFor(() =>
      expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
        null,
        "asset-2",
      ),
    );
    await waitFor(() => expect(result.current.scopeLabel).toBe("Microsoft Corp"));
  });

  // Changing the account scope resets the asset scope to All assets
  it("resets the asset scope when the account scope changes", async () => {
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));
    await waitFor(() => expect(result.current.selectedAccountId).toBe("account-1"));
    act(() => result.current.setSelectedAssetId("asset-1"));
    await waitFor(() => expect(result.current.selectedAssetId).toBe("asset-1"));

    act(() => result.current.setSelectedAccountId("account-2"));

    expect(result.current.selectedAssetId).toBeNull();
    await waitFor(() =>
      expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
        "account-2",
        null,
      ),
    );
    // The new account scope is never fetched with the previous asset scope.
    expect(gateway.globalPerformanceGateway.getGlobalPerformance).not.toHaveBeenCalledWith(
      "account-2",
      "asset-1",
    );
  });

  // Returning the account scope to All accounts re-fetches the whole portfolio
  it("re-fetches unscoped when the account scope returns to All accounts", async () => {
    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));
    await waitFor(() => expect(result.current.selectedAccountId).toBe("account-1"));

    act(() => result.current.setSelectedAccountId(null));

    await waitFor(() => expect(result.current.selectedAccountId).toBeNull());
    const calls = vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mock.calls;
    expect(calls[calls.length - 1]).toEqual([null, null]);
  });

  // F27 — a gateway error surfaces as an i18n message with retry
  it("exposes the presented error and retries the fetch", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() =>
      expect(result.current.error).toEqual({
        key: "account_performance.error.database_error",
      }),
    );

    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });
    await act(async () => result.current.retry());

    await waitFor(() => expect(result.current.error).toBeNull());
    expect(result.current.rows).toHaveLength(1);
  });

  // GPF-016 — month view available opens in month view with the most recent year selected
  it("opens in month view with the most recent year when month view is available (GPF-016)", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({
        month_view_available: true,
        monthly: [makeYearRow({ year: 2025, month: 5 }), makeYearRow({ year: 2024, month: 12 })],
      }),
    });

    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() => expect(result.current.viewMode).toBe("month"));
    expect(result.current.selectedYear).toBe(2025);
    expect(result.current.availableYears).toEqual([2025, 2024]);
  });

  // GPF-016 — a remembered "year" preference overrides the month-view default
  it("restores the remembered view mode over the default (GPF-016)", async () => {
    localStorage.setItem("global_perf_view_mode", "year");
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({
        month_view_available: true,
        monthly: [makeYearRow({ year: 2025, month: 5 })],
      }),
    });

    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.viewMode).toBe("year");
  });

  // GPF-016 — toggling the view mode persists the choice on the device
  it("persists the view mode when the user toggles it (GPF-016)", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({
        month_view_available: true,
        monthly: [makeYearRow({ year: 2025, month: 5 })],
      }),
    });

    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.viewMode).toBe("month"));

    act(() => result.current.setViewMode("year"));

    expect(result.current.viewMode).toBe("year");
    expect(localStorage.getItem("global_perf_view_mode")).toBe("year");
  });

  // GPF-016 — a remembered "month" is clamped to year view when month view is gone
  it("falls back to year view when the remembered month view is unavailable (GPF-016)", async () => {
    localStorage.setItem("global_perf_view_mode", "month");

    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.viewMode).toBe("year");
    // The stored preference is left intact for when month view returns.
    expect(localStorage.getItem("global_perf_view_mode")).toBe("month");
  });

  // GPF-015 — the empty portfolio read is exposed as isEmpty
  it("flags the empty portfolio (GPF-015)", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ yearly: [], monthly: [] }),
    });

    const { result } = renderHook(() => useGlobalPerformance());

    await waitFor(() => expect(result.current.isEmpty).toBe(true));
  });

  // F27 — a holdings-fetch failure surfaces via the snackbar, not just the log
  it("surfaces a holdings fetch failure via the snackbar (F27)", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getAccountHoldings).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));

    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith(
        "account_performance.error.database_error",
        "error",
      ),
    );
  });

  // Stale-response guard — racing the two selectors never lets an older read win
  it("ignores a stale response when the account and asset scopes race", async () => {
    let resolveStale: (value: { status: "ok"; data: AccountPerformanceResponse }) => void =
      () => {};
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance)
      // Mount fetch (all accounts) — resolves normally.
      .mockResolvedValueOnce({ status: "ok", data: makeResponse() })
      // Account-scope fetch — held open, resolved last with old data.
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStale = resolve;
        }),
      )
      // Asset-scope fetch — resolves first with the fresh data.
      .mockResolvedValueOnce({
        status: "ok",
        data: makeResponse({ yearly: [makeYearRow({ year: 2025 })] }),
      });

    const { result } = renderHook(() => useGlobalPerformance());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAccountId("account-1"));
    act(() => result.current.setSelectedAssetId("asset-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.rows[0]?.year).toBe(2025);

    await act(async () => {
      resolveStale({
        status: "ok",
        data: makeResponse({ yearly: [makeYearRow({ year: 2020 })] }),
      });
    });

    // The newest response wins: the late account-scoped data is dropped.
    expect(result.current.rows[0]?.year).toBe(2025);
    expect(result.current.isLoading).toBe(false);
  });
});
