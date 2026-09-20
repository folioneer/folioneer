import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AccountDetailsResponse,
  AccountPerformanceResponse,
  Event,
  HoldingDetail,
  PerformancePeriod,
} from "@/bindings";

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
import { useAccountPerformance } from "./useAccountPerformance";

// ---- Fixtures ---------------------------------------------------------------

const BRIDGE_DEFAULTS = {
  previous_value: 9_000_000_000,
  cash_flow: 500_000_000,
  asset_flow: 0,
  dividends: 120_000_000,
  pnl: 380_000_000,
} satisfies Partial<PerformancePeriod>;

const makeYearRow = (overrides: Partial<PerformancePeriod> = {}): PerformancePeriod => ({
  year: 2025,
  month: null,
  end_value: 10_000_000_000,
  ...BRIDGE_DEFAULTS,
  period_over_period: { gain: 500_000_000, pct: 5_000_000 },
  year_to_date: null,
  since_inception: { gain: 500_000_000, pct: 5_000_000 },
  annualized_yield: { gain: 500_000_000, pct: 5_000_000 },
  ...overrides,
});

const makeResponse = (
  overrides: Partial<AccountPerformanceResponse> = {},
): AccountPerformanceResponse => ({
  account_name: "My Portfolio",
  currency: "EUR",
  month_view_available: false,
  yearly: [makeYearRow()],
  monthly: [],
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
  account_name: "My Portfolio",
  holdings: [
    makeHolding(),
    makeHolding({ asset_id: "asset-2", asset_name: "Microsoft Corp", asset_reference: "MSFT" }),
  ],
  closed_holdings: [],
  total_holding_count: 2,
  total_cost_basis: 400_000_000,
  total_realized_pnl: 0,
  total_unrealized_pnl: null,
  total_global_value: 0,
  total_dividends_received: 0,
  total_management_fees: 0,
  total_net_cash_input: 0,
  ...overrides,
});

// ---- Tests ------------------------------------------------------------------

describe("useAccountPerformance — asset scope", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });
    vi.mocked(gateway.accountPerformanceGateway.getAccountHoldings).mockResolvedValue({
      status: "ok",
      data: makeDetailsResponse(),
    });
    vi.mocked(gateway.accountPerformanceGateway.subscribeToEvents).mockResolvedValue(() => {});
  });

  // PRF-080 — default scope is the whole account (assetId null)
  it("defaults to the whole account and fetches with a null asset scope (PRF-080)", async () => {
    const { result } = renderHook(() => useAccountPerformance("account-1"));

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.selectedAssetId).toBeNull();
    expect(result.current.selectedAssetName).toBeNull();
    expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledWith(
      "account-1",
      null,
    );
  });

  // PRF-082 — the selector options come from the account's holdings
  it("exposes the account holdings as asset options (PRF-082)", async () => {
    const { result } = renderHook(() => useAccountPerformance("account-1"));

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
    vi.mocked(gateway.accountPerformanceGateway.subscribeToEvents).mockImplementation((cb) => {
      capturedCallback = cb;
      return Promise.resolve(() => {});
    });
    const { result } = renderHook(() => useAccountPerformance("account-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    const before = vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock.calls
      .length;

    await act(async () => {
      capturedCallback?.("SyncCompleted");
    });

    expect(
      vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock.calls.length,
    ).toBe(before + 1);
  });

  // PRF-080 — selecting an asset re-fetches with the scoped id
  it("re-fetches with the asset id when a scope is selected (PRF-080)", async () => {
    const { result } = renderHook(() => useAccountPerformance("account-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAssetId("asset-2"));

    await waitFor(() =>
      expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledWith(
        "account-1",
        "asset-2",
      ),
    );
    expect(result.current.selectedAssetId).toBe("asset-2");
    await waitFor(() => expect(result.current.selectedAssetName).toBe("Microsoft Corp"));
  });

  // PRF-080 — selecting "All assets" returns to the whole-account read
  it("re-fetches unscoped when the selection returns to All assets (PRF-080)", async () => {
    const { result } = renderHook(() => useAccountPerformance("account-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAssetId("asset-1"));
    await waitFor(() => expect(result.current.selectedAssetId).toBe("asset-1"));

    act(() => result.current.setSelectedAssetId(null));

    await waitFor(() => expect(result.current.selectedAssetId).toBeNull());
    const calls = vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock.calls;
    expect(calls[calls.length - 1]).toEqual(["account-1", null]);
  });

  // PRF-080 — the scope is session- and account-local: a new account starts unscoped
  it("resets the scope to All assets when the account changes (PRF-080)", async () => {
    const { result, rerender } = renderHook(
      ({ accountId }: { accountId: string }) => useAccountPerformance(accountId),
      { initialProps: { accountId: "account-1" } },
    );
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.setSelectedAssetId("asset-1"));
    await waitFor(() => expect(result.current.selectedAssetId).toBe("asset-1"));

    rerender({ accountId: "account-2" });

    expect(result.current.selectedAssetId).toBeNull();
    await waitFor(() =>
      expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledWith(
        "account-2",
        null,
      ),
    );
    // The new account is never fetched with the previous account's scope.
    expect(gateway.accountPerformanceGateway.getAccountPerformance).not.toHaveBeenCalledWith(
      "account-2",
      "asset-1",
    );
  });

  // F27 — a holdings-fetch failure surfaces via the snackbar, not just the log
  it("surfaces a holdings fetch failure via the snackbar (F27)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountHoldings).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    renderHook(() => useAccountPerformance("account-1"));

    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith(
        "account_performance.error.database_error",
        "error",
      ),
    );
  });

  // Stale-response guard — an older in-flight read never overwrites a newer one
  it("ignores a stale response that resolves after a newer fetch", async () => {
    let resolveStale: (value: { status: "ok"; data: AccountPerformanceResponse }) => void =
      () => {};
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance)
      // Mount fetch (whole account) — held open, resolved last with old data.
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStale = resolve;
        }),
      )
      // Scoped fetch — resolves first with the fresh data.
      .mockResolvedValueOnce({
        status: "ok",
        data: makeResponse({ yearly: [makeYearRow({ year: 2025 })] }),
      });

    const { result } = renderHook(() => useAccountPerformance("account-1"));

    act(() => result.current.setSelectedAssetId("asset-2"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.rows[0]?.year).toBe(2025);

    await act(async () => {
      resolveStale({
        status: "ok",
        data: makeResponse({ yearly: [makeYearRow({ year: 2020 })] }),
      });
    });

    // The newest response wins: the late unscoped data is dropped.
    expect(result.current.rows[0]?.year).toBe(2025);
    expect(result.current.isLoading).toBe(false);
  });
});
