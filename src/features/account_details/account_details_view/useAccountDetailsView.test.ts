import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getPerfPeriod, setPerfPeriod } from "@/lib/perfPeriodStorage";
import { useAppStore } from "@/lib/store";
import { useAccountDetailsView } from "./useAccountDetailsView";

const mockBlock = vi.fn();
const mockUnblock = vi.fn();
const mockShowSnackbar = vi.fn();
const mockNavigate = vi.fn();
const mockFetchAssets = vi.fn().mockResolvedValue(undefined);
// Defaults to an error response (most tests don't need holdings); individual
// tests override per-call via `mockResolvedValueOnce` to supply holdings.
const mockGetAccountDetails = vi.fn((..._args: unknown[]) =>
  Promise.resolve({ status: "error", error: { code: "DatabaseError" } }),
);

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en-US" } }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    blockAssetPriceRefresh: (...args: unknown[]) => mockBlock(...args),
    unblockAssetPriceRefresh: (...args: unknown[]) => mockUnblock(...args),
    backfillHoldingPriceHistory: (...args: unknown[]) => mockBackfill(...args),
    getAccountDetails: (...args: unknown[]) => mockGetAccountDetails(...args),
    subscribeToEvents: vi.fn(() => Promise.resolve(() => {})),
  },
  // useAccountDetails reads the asset catalog via this selector; back it with the
  // real store so the setState-driven tests still drive it.
  useCachedAssets: () => useAppStore((state) => state.assets),
}));

const mockBackfill = vi.fn();

describe("useAccountDetailsView — price history backfill (MKT-190/197)", () => {
  beforeEach(() => {
    mockBackfill.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "USD" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("calls the command with the account and asset ids and reports the counts", async () => {
    mockBackfill.mockResolvedValue({ status: "ok", data: { written: 12, already_priced: 3 } });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleBackfillPriceHistory("asset-1");
    });

    expect(mockBackfill).toHaveBeenCalledWith("acc-1", "asset-1");
    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.backfill.success", "success");
  });

  it("reports that nothing was missing as information", async () => {
    mockBackfill.mockResolvedValue({ status: "ok", data: { written: 0, already_priced: 40 } });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleBackfillPriceHistory("asset-1");
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.backfill.nothing_missing", "info");
  });

  it("reports a rejection through the error presenter", async () => {
    mockBackfill.mockResolvedValue({ status: "error", error: { code: "TickerNotResolved" } });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleBackfillPriceHistory("asset-1");
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.backfill.error.TickerNotResolved", "error");
  });

  it("marks the asset as running until the command settles", async () => {
    let settle!: (value: unknown) => void;
    mockBackfill.mockReturnValue(
      new Promise((resolve) => {
        settle = resolve;
      }),
    );
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    let pending!: Promise<void>;
    act(() => {
      pending = result.current.handleBackfillPriceHistory("asset-1");
    });
    expect(result.current.backfillingAssetIds).toEqual(["asset-1"]);

    await act(async () => {
      settle({ status: "ok", data: { written: 1, already_priced: 0 } });
      await pending;
    });
    expect(result.current.backfillingAssetIds).toEqual([]);
  });

  it("surfaces a generic error snackbar and clears the running state when the gateway throws", async () => {
    mockBackfill.mockRejectedValue(new Error("ipc broken"));
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleBackfillPriceHistory("asset-1");
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.Unknown", "error");
    expect(result.current.backfillingAssetIds).toEqual([]);
  });
});

describe("useAccountDetailsView — price-refresh lock toggle (MKT-156/157)", () => {
  beforeEach(() => {
    mockBlock.mockReset();
    mockUnblock.mockReset();
    mockShowSnackbar.mockReset();
    mockFetchAssets.mockClear();
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "USD" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("calls blockAssetPriceRefresh, refetches assets, and surfaces a success snackbar when toggling an unlocked asset", async () => {
    mockBlock.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleTogglePriceRefreshLock("asset-1", false);
    });

    expect(mockBlock).toHaveBeenCalledWith("asset-1");
    expect(mockUnblock).not.toHaveBeenCalled();
    expect(mockFetchAssets).toHaveBeenCalledTimes(1);
    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.lock.success_blocked", "success");
  });

  it("calls unblockAssetPriceRefresh when the asset is currently locked", async () => {
    mockUnblock.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleTogglePriceRefreshLock("asset-1", true);
    });

    expect(mockUnblock).toHaveBeenCalledWith("asset-1");
    expect(mockBlock).not.toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalledWith("mkt.lock.success_unblocked", "success");
  });

  it("surfaces a typed error snackbar when the backend rejects", async () => {
    mockBlock.mockResolvedValue({ status: "error", error: { code: "CashAssetNotEditable" } });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleTogglePriceRefreshLock("cash-id", false);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.CashAssetNotEditable", "error");
    expect(mockFetchAssets).not.toHaveBeenCalled();
  });

  it("surfaces a generic error snackbar when the gateway throws", async () => {
    mockBlock.mockRejectedValue(new Error("ipc broken"));
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));

    await act(async () => {
      await result.current.handleTogglePriceRefreshLock("asset-1", false);
    });

    expect(mockShowSnackbar).toHaveBeenCalledWith("error.Unknown", "error");
  });
});

// ---------------------------------------------------------------------------
// DIV-012 — Header "Record" menu: dividend modal state in useAccountDetailsView.
// The AccountDetailsView component replaces three standalone header buttons
// with a consolidated "Record" dropdown; the dividend modal open/close/success
// state is managed here. The view-level menu composition (button ids, item
// routing) is a render concern covered in AccountDetailsView.test.tsx.
// ---------------------------------------------------------------------------

describe("useAccountDetailsView — dividend modal state (DIV-012)", () => {
  beforeEach(() => {
    mockBlock.mockReset();
    mockUnblock.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  // DIV-012 — dividendOpen is initially false
  it("dividendOpen starts as false", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.dividendOpen).toBe(false);
  });

  // DIV-012 — handleDividendOpen sets dividendOpen to true
  it("handleDividendOpen sets dividendOpen to true (DIV-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleDividendOpen());
    expect(result.current.dividendOpen).toBe(true);
  });

  // DIV-012 — handleDividendClose resets dividendOpen to false
  it("handleDividendClose resets dividendOpen to false (DIV-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleDividendOpen());
    act(() => result.current.handleDividendClose());
    expect(result.current.dividendOpen).toBe(false);
  });

  // DIV-012 — handleDividendSuccess closes the modal and triggers a data re-fetch
  it("handleDividendSuccess closes modal and calls retry (DIV-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleDividendOpen());

    act(() => result.current.handleDividendSuccess());

    expect(result.current.dividendOpen).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// FSD-012 — Header "Record" menu: free-shares modal state in useAccountDetailsView.
// Mirrors the DIV-012 dividend pattern: freeSharesOpen starts false, flips
// true on handleFreeSharesOpen, resets to false on close/success.
// ---------------------------------------------------------------------------

describe("useAccountDetailsView — free-shares modal state (FSD-012)", () => {
  beforeEach(() => {
    mockBlock.mockReset();
    mockUnblock.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  // FSD-012 — freeSharesOpen starts as false
  it("freeSharesOpen starts as false (FSD-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.freeSharesOpen).toBe(false);
  });

  // FSD-012 — handleFreeSharesOpen sets freeSharesOpen to true
  it("handleFreeSharesOpen sets freeSharesOpen to true (FSD-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleFreeSharesOpen());
    expect(result.current.freeSharesOpen).toBe(true);
  });

  // FSD-012 — handleFreeSharesClose resets freeSharesOpen to false
  it("handleFreeSharesClose resets freeSharesOpen to false (FSD-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleFreeSharesOpen());
    act(() => result.current.handleFreeSharesClose());
    expect(result.current.freeSharesOpen).toBe(false);
  });

  // FSD-012 — handleFreeSharesSuccess closes the modal
  it("handleFreeSharesSuccess closes the modal (FSD-012)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleFreeSharesOpen());
    act(() => result.current.handleFreeSharesSuccess());
    expect(result.current.freeSharesOpen).toBe(false);
  });
});

describe("useAccountDetailsView — management-fee modal state (FEE-010)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("managementFeeOpen starts false and flips on open", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.managementFeeOpen).toBe(false);
    act(() => result.current.handleManagementFeeOpen());
    expect(result.current.managementFeeOpen).toBe(true);
  });

  it("handleManagementFeeClose and handleManagementFeeSuccess close the modal", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleManagementFeeOpen());
    act(() => result.current.handleManagementFeeClose());
    expect(result.current.managementFeeOpen).toBe(false);
    act(() => result.current.handleManagementFeeOpen());
    act(() => result.current.handleManagementFeeSuccess());
    expect(result.current.managementFeeOpen).toBe(false);
  });
});

describe("useAccountDetailsView — interest modal state (INT-010)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("interestOpen starts false and flips on open", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.interestOpen).toBe(false);
    act(() => result.current.handleInterestOpen());
    expect(result.current.interestOpen).toBe(true);
  });

  it("handleInterestClose and handleInterestSuccess close the modal", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleInterestOpen());
    act(() => result.current.handleInterestClose());
    expect(result.current.interestOpen).toBe(false);
    act(() => result.current.handleInterestOpen());
    act(() => result.current.handleInterestSuccess());
    expect(result.current.interestOpen).toBe(false);
  });
});

describe("useAccountDetailsView — fee-schedule modal target (FEE-011)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("feeScheduleTarget starts null and captures asset id + name on open", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.feeScheduleTarget).toBeNull();
    act(() => result.current.handleFeeScheduleOpen("asset-9", "Vanguard ETF"));
    expect(result.current.feeScheduleTarget).toEqual({
      assetId: "asset-9",
      assetName: "Vanguard ETF",
    });
  });

  it("handleFeeScheduleClose and handleFeeScheduleSuccess clear the target", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.handleFeeScheduleOpen("asset-9", "Vanguard ETF"));
    act(() => result.current.handleFeeScheduleClose());
    expect(result.current.feeScheduleTarget).toBeNull();
    act(() => result.current.handleFeeScheduleOpen("asset-9", "Vanguard ETF"));
    act(() => result.current.handleFeeScheduleSuccess());
    expect(result.current.feeScheduleTarget).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// DIV-011/020 — activeNonCashHoldings exposes only active, non-cash holdings
// (quantity > 0) as candidates for the dividend modal's asset selector.
// ---------------------------------------------------------------------------
const makeHoldingDetail = (overrides: Record<string, unknown> = {}) => ({
  asset_id: "asset-1",
  asset_name: "Asset One",
  asset_reference: "A1",
  quantity: 1_000_000,
  average_price: 1_000_000,
  cost_basis: 1_000_000,
  realized_pnl: 0,
  asset_currency: "EUR",
  current_price: null,
  current_price_date: null,
  current_price_source: null,
  unrealized_pnl: null,
  performance_pct: null,
  dividends_received: 0,
  total_return_pct: null,
  note_text: null,
  note_threshold_price: null,
  note_threshold_direction: null,
  note_alarm_triggered: false,
  period_performance: {
    ytd: null,
    one_year: null,
    two_years: null,
    five_years: null,
    ten_years: null,
  },
  inconsistency: null,
  ...overrides,
});

describe("useAccountDetailsView — activeNonCashHoldings filter (DIV-011/020)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("includes only active non-cash holdings (excludes cash + zero-quantity)", async () => {
    mockGetAccountDetails.mockResolvedValueOnce({
      status: "ok",
      data: {
        account_name: "Main",
        holdings: [
          makeHoldingDetail({
            asset_id: "system-cash-eur",
            asset_name: "Cash EUR",
            quantity: 500_000_000,
          }),
          makeHoldingDetail({ asset_id: "asset-zero", asset_name: "Zero Co", quantity: 0 }),
          makeHoldingDetail({
            asset_id: "asset-active",
            asset_name: "Active Co",
            asset_currency: "USD",
            quantity: 2_000_000,
          }),
        ],
        closed_holdings: [],
        total_holding_count: 3,
        total_cost_basis: 0,
        total_realized_pnl: 0,
        total_unrealized_pnl: null,
        total_global_value: 0,
        total_dividends_received: 0,
      },
    } as never);

    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    expect(result.current.activeNonCashHoldings).toEqual([
      { assetId: "asset-active", assetName: "Active Co", assetCurrency: "USD" },
    ]);
  });

  // INT-020/023 — the interest candidates are the cash line plus the active
  // non-cash holdings whose asset is flagged interest_bearing (AST-024);
  // zero-quantity non-cash assets stay excluded even when flagged.
  it("interestEligibleHoldings includes the cash line and only flagged active non-cash holdings", async () => {
    useAppStore.setState({
      assets: [
        { id: "asset-zero", interest_bearing: true },
        { id: "asset-active", interest_bearing: true },
      ] as never,
    });
    mockGetAccountDetails.mockResolvedValueOnce({
      status: "ok",
      data: {
        account_name: "Main",
        holdings: [
          makeHoldingDetail({
            asset_id: "system-cash-eur",
            asset_name: "Cash EUR",
            quantity: 0,
          }),
          makeHoldingDetail({ asset_id: "asset-zero", asset_name: "Zero Co", quantity: 0 }),
          makeHoldingDetail({
            asset_id: "asset-active",
            asset_name: "Active Co",
            asset_currency: "USD",
            quantity: 2_000_000,
          }),
        ],
        closed_holdings: [],
        total_holding_count: 3,
        total_cost_basis: 0,
        total_realized_pnl: 0,
        total_unrealized_pnl: null,
        total_global_value: 0,
        total_dividends_received: 0,
      },
    } as never);

    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    expect(result.current.interestEligibleHoldings).toEqual([
      { assetId: "system-cash-eur", assetName: "Cash EUR", assetCurrency: "EUR" },
      { assetId: "asset-active", assetName: "Active Co", assetCurrency: "USD" },
    ]);
  });

  // INT-020 / AST-024 — a non-flagged non-cash holding is excluded from the
  // interest candidates even with quantity > 0; the cash line stays eligible.
  it("interestEligibleHoldings excludes unflagged non-cash holdings with quantity > 0", async () => {
    useAppStore.setState({
      assets: [
        { id: "asset-flagged", interest_bearing: true },
        { id: "asset-unflagged", interest_bearing: false },
      ] as never,
    });
    mockGetAccountDetails.mockResolvedValueOnce({
      status: "ok",
      data: {
        account_name: "Main",
        holdings: [
          makeHoldingDetail({
            asset_id: "system-cash-eur",
            asset_name: "Cash EUR",
            quantity: 500_000_000,
          }),
          makeHoldingDetail({
            asset_id: "asset-flagged",
            asset_name: "Flagged Co",
            quantity: 2_000_000,
          }),
          makeHoldingDetail({
            asset_id: "asset-unflagged",
            asset_name: "Unflagged Co",
            quantity: 3_000_000,
          }),
        ],
        closed_holdings: [],
        total_holding_count: 3,
        total_cost_basis: 0,
        total_realized_pnl: 0,
        total_unrealized_pnl: null,
        total_global_value: 0,
        total_dividends_received: 0,
      },
    } as never);

    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    expect(result.current.interestEligibleHoldings).toEqual([
      { assetId: "system-cash-eur", assetName: "Cash EUR", assetCurrency: "EUR" },
      { assetId: "asset-flagged", assetName: "Flagged Co", assetCurrency: "EUR" },
    ]);
  });
});

// ---------------------------------------------------------------------------
// As-of read-only mode: selecting a past date sets isAsOf and no-ops every
// mutation open-handler; clearing the date returns to the live, mutable view.
// ---------------------------------------------------------------------------

describe("useAccountDetailsView — as-of read-only mode", () => {
  beforeEach(() => {
    mockNavigate.mockReset();
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("isAsOf is false by default (live view)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.isAsOf).toBe(false);
  });

  it("selecting a past date enters as-of mode and blocks every mutation handler", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.setAsOfDate("2020-01-01"));
    expect(result.current.isAsOf).toBe(true);

    act(() => result.current.handleDividendOpen());
    act(() => result.current.handleFreeSharesOpen());
    act(() => result.current.handleDepositOpen());
    act(() => result.current.handleWithdrawalOpen());
    act(() => result.current.handleOpenBalanceOpen());
    act(() =>
      result.current.handleBuyOpen({
        accountName: "Main",
        assetId: "a1",
        assetName: "A1",
        assetCurrency: "EUR",
        showExchangeRate: false,
      }),
    );
    act(() =>
      result.current.handleSellOpen({
        accountName: "Main",
        assetId: "a1",
        assetName: "A1",
        assetCurrency: "EUR",
        showExchangeRate: false,
        holdingQuantityMicro: 1_000_000,
      }),
    );

    expect(result.current.dividendOpen).toBe(false);
    expect(result.current.freeSharesOpen).toBe(false);
    expect(result.current.depositOpen).toBe(false);
    expect(result.current.withdrawalOpen).toBe(false);
    expect(result.current.openBalanceOpen).toBe(false);
    expect(result.current.buyTarget).toBeNull();
    expect(result.current.sellTarget).toBeNull();
  });

  it("clearing the date returns to the live view", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.setAsOfDate("2020-01-01"));
    expect(result.current.isAsOf).toBe(true);
    act(() => result.current.setAsOfDate(""));
    expect(result.current.isAsOf).toBe(false);
  });

  // handleAddTransaction navigates (URL-driven modal) rather than setting state;
  // in as-of mode it must be a no-op (no navigate call).
  it("blocks handleAddTransaction in as-of mode (no navigate)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.setAsOfDate("2020-01-01"));
    expect(result.current.isAsOf).toBe(true);

    act(() => result.current.handleAddTransaction());

    expect(mockNavigate).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// ACD-054 — performance-column period: since-start default, per-account
// persistence, and the as-of pin to since-start (windowed returns are a
// live-view metric).
// ---------------------------------------------------------------------------

describe("useAccountDetailsView — performance period (ACD-054)", () => {
  beforeEach(() => {
    localStorage.clear();
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("defaults to since_start when no preference is stored", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.perfPeriod).toBe("since_start");
  });

  it("initializes from the stored per-account preference", () => {
    setPerfPeriod("acc-1", "ytd");
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.perfPeriod).toBe("ytd");
  });

  it("setPerfPeriod updates the state and persists the choice", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.setPerfPeriod("five_years"));
    expect(result.current.perfPeriod).toBe("five_years");
    expect(getPerfPeriod("acc-1")).toBe("five_years");
  });

  it("pins the period to since_start in the as-of view without losing the stored choice", () => {
    setPerfPeriod("acc-1", "one_year");
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.perfPeriod).toBe("one_year");

    act(() => result.current.setAsOfDate("2020-01-01"));
    expect(result.current.perfPeriod).toBe("since_start");

    act(() => result.current.setAsOfDate(""));
    expect(result.current.perfPeriod).toBe("one_year");
  });

  it("setPerfPeriod is inert in the as-of view (no state change, no persistence)", () => {
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    act(() => result.current.setAsOfDate("2020-01-01"));

    act(() => result.current.setPerfPeriod("ten_years"));

    expect(result.current.perfPeriod).toBe("since_start");
    expect(getPerfPeriod("acc-1")).toBeNull();
  });
});

describe("useAccountDetailsView — management fees gate (FEE-076)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      fetchAssets: mockFetchAssets,
    } as never);
  });

  it("derives managementFeesEnabled from the account catalog", () => {
    useAppStore.setState({
      accounts: [
        { id: "acc-1", name: "Main", currency: "EUR", management_fees_enabled: true },
      ] as never,
    });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.managementFeesEnabled).toBe(true);
  });

  it("is false for a disabled account and for an unknown account", () => {
    useAppStore.setState({
      accounts: [
        { id: "acc-1", name: "Main", currency: "EUR", management_fees_enabled: false },
      ] as never,
    });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    expect(result.current.managementFeesEnabled).toBe(false);

    const { result: unknown } = renderHook(() => useAccountDetailsView("acc-missing"));
    expect(unknown.current.managementFeesEnabled).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// SPL-061 — split modal target: built from the holding detail so the modal's
// preview and price prefill consume the raw quantity/average/latest price.
// ---------------------------------------------------------------------------
describe("useAccountDetailsView — split modal target (SPL-061)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  const seedHoldings = () =>
    mockGetAccountDetails.mockResolvedValueOnce({
      status: "ok",
      data: {
        account_name: "Main",
        holdings: [
          makeHoldingDetail({
            asset_id: "asset-split",
            asset_name: "Alphabet Inc",
            quantity: 10_000_000,
            average_price: 150_000_000,
            current_price: 150_000_000,
          }),
        ],
        closed_holdings: [],
        total_holding_count: 1,
        total_cost_basis: 0,
        total_realized_pnl: 0,
        total_unrealized_pnl: null,
        total_global_value: 0,
        total_dividends_received: 0,
      },
    } as never);

  it("splitTarget starts null and captures the holding's raw figures on open", async () => {
    seedHoldings();
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    expect(result.current.splitTarget).toBeNull();
    act(() => result.current.handleSplitOpen("asset-split"));
    expect(result.current.splitTarget).toEqual({
      assetId: "asset-split",
      assetName: "Alphabet Inc",
      holdingQuantityMicro: 10_000_000,
      averagePriceMicro: 150_000_000,
      currentPriceMicro: 150_000_000,
    });
  });

  it("handleSplitOpen is a no-op for an unknown asset", async () => {
    seedHoldings();
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    act(() => result.current.handleSplitOpen("asset-missing"));
    expect(result.current.splitTarget).toBeNull();
  });

  it("handleSplitClose and handleSplitSuccess clear the target", async () => {
    seedHoldings();
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    act(() => result.current.handleSplitOpen("asset-split"));
    act(() => result.current.handleSplitClose());
    expect(result.current.splitTarget).toBeNull();

    act(() => result.current.handleSplitOpen("asset-split"));
    act(() => result.current.handleSplitSuccess());
    expect(result.current.splitTarget).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// HNO-042 — holding-note modal target: built from the raw holding detail so
// the modal prefills from the stored note (text + alarm pair, HNO-020).
// ---------------------------------------------------------------------------
describe("useAccountDetailsView — holding-note modal target (HNO-042)", () => {
  beforeEach(() => {
    useAppStore.setState({
      assets: [],
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
      fetchAssets: mockFetchAssets,
    } as never);
  });

  const seedNoteHoldings = (noteOverrides: Record<string, unknown> = {}) =>
    mockGetAccountDetails.mockResolvedValueOnce({
      status: "ok",
      data: {
        account_name: "Main",
        holdings: [
          makeHoldingDetail({
            asset_id: "asset-noted",
            asset_name: "Air Liquide",
            asset_currency: "EUR",
            ...noteOverrides,
          }),
        ],
        closed_holdings: [],
        total_holding_count: 1,
        total_cost_basis: 0,
        total_realized_pnl: 0,
        total_unrealized_pnl: null,
        total_global_value: 0,
        total_dividends_received: 0,
      },
    } as never);

  it("builds a create-mode target (existing null) when the holding has no note", async () => {
    seedNoteHoldings();
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    expect(result.current.holdingNoteTarget).toBeNull();
    act(() => result.current.handleHoldingNoteOpen("asset-noted"));
    expect(result.current.holdingNoteTarget).toEqual({
      assetId: "asset-noted",
      assetName: "Air Liquide",
      assetCurrency: "EUR",
      existing: null,
    });
  });

  it("builds an edit-mode target carrying the stored note and alarm pair", async () => {
    seedNoteHoldings({
      note_text: "buy 7 shares below 150",
      note_threshold_price: 150_000_000,
      note_threshold_direction: "Below",
      note_alarm_triggered: true,
    });
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    act(() => result.current.handleHoldingNoteOpen("asset-noted"));
    expect(result.current.holdingNoteTarget).toEqual({
      assetId: "asset-noted",
      assetName: "Air Liquide",
      assetCurrency: "EUR",
      existing: {
        text: "buy 7 shares below 150",
        thresholdPrice: 150_000_000,
        thresholdDirection: "Below",
      },
    });
  });

  it("handleHoldingNoteOpen is a no-op for an unknown asset", async () => {
    seedNoteHoldings();
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    act(() => result.current.handleHoldingNoteOpen("asset-missing"));
    expect(result.current.holdingNoteTarget).toBeNull();
  });

  it("handleHoldingNoteClose and handleHoldingNoteSuccess clear the target", async () => {
    seedNoteHoldings();
    const { result } = renderHook(() => useAccountDetailsView("acc-1"));
    await act(async () => {});

    act(() => result.current.handleHoldingNoteOpen("asset-noted"));
    act(() => result.current.handleHoldingNoteClose());
    expect(result.current.holdingNoteTarget).toBeNull();

    act(() => result.current.handleHoldingNoteOpen("asset-noted"));
    act(() => result.current.handleHoldingNoteSuccess());
    expect(result.current.holdingNoteTarget).toBeNull();
  });
});
