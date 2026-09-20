import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AccountDetailsResponse,
  AccountError,
  AccountPerformanceResponse,
  HoldingDetail,
  PerformancePeriod,
} from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { globalPerformanceGateway } = await import("./gateway");

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
  month_view_available: true,
  yearly: [makeYearRow()],
  monthly: [],
  ...overrides,
});

describe("globalPerformanceGateway — getGlobalPerformance", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // GPF-010 — the all-accounts scope is a (null, null) call
  it("passes the all-accounts scope through to the command (GPF-010)", async () => {
    const response = makeResponse();
    mockInvoke.mockResolvedValue(response);

    const result = await globalPerformanceGateway.getGlobalPerformance(null, null);

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      // GPF-011 — cross-account scope reports in EUR with an empty account_name
      expect(result.data.currency).toBe("EUR");
      expect(result.data.account_name).toBe("");
    }
    expect(mockInvoke).toHaveBeenCalledWith("get_global_performance", {
      accountId: null,
      assetId: null,
    });
  });

  // GPF-010 — the single-account scope forwards the account id
  it("passes the account scope through to the command (GPF-010)", async () => {
    mockInvoke.mockResolvedValue(makeResponse({ account_name: "My Portfolio" }));

    const result = await globalPerformanceGateway.getGlobalPerformance("account-1", null);

    expect(result.status).toBe("ok");
    expect(mockInvoke).toHaveBeenCalledWith("get_global_performance", {
      accountId: "account-1",
      assetId: null,
    });
  });

  // GPF-010 — the scoped-position scope forwards both ids
  it("passes the account + asset scope through to the command (GPF-010)", async () => {
    mockInvoke.mockResolvedValue(makeResponse({ account_name: "My Portfolio" }));

    await globalPerformanceGateway.getGlobalPerformance("account-1", "asset-1");

    expect(mockInvoke).toHaveBeenCalledWith("get_global_performance", {
      accountId: "account-1",
      assetId: "asset-1",
    });
  });

  // GPF-010 — the cross-account asset scope forwards the asset id alone
  it("passes the asset-across-accounts scope through to the command (GPF-010)", async () => {
    mockInvoke.mockResolvedValue(makeResponse());

    await globalPerformanceGateway.getGlobalPerformance(null, "asset-1");

    expect(mockInvoke).toHaveBeenCalledWith("get_global_performance", {
      accountId: null,
      assetId: "asset-1",
    });
  });

  // F27 — gateway does NOT throw; it returns the error result unchanged
  it("passes through DatabaseError result", async () => {
    const err: AccountError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);

    const result = await globalPerformanceGateway.getGlobalPerformance(null, null);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("DatabaseError");
    }
  });

  // GPF-015 — an empty portfolio is an ok result with empty arrays
  it("passes through ok result with empty yearly and monthly arrays (GPF-015)", async () => {
    mockInvoke.mockResolvedValue(
      makeResponse({ yearly: [], monthly: [], month_view_available: false }),
    );

    const result = await globalPerformanceGateway.getGlobalPerformance(null, null);

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.data.yearly).toHaveLength(0);
      expect(result.data.monthly).toHaveLength(0);
      expect(result.data.month_view_available).toBe(false);
    }
  });
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

const makeDetailsResponse = (): AccountDetailsResponse => ({
  account_name: "My Portfolio",
  holdings: [makeHolding()],
  closed_holdings: [],
  total_holding_count: 1,
  total_cost_basis: 200_000_000,
  total_realized_pnl: 0,
  total_unrealized_pnl: null,
  total_global_value: 0,
  total_dividends_received: 0,
  total_management_fees: 0,
  total_net_cash_input: 0,
});

describe("globalPerformanceGateway — getAccountHoldings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // The holdings read backing the account-scoped asset selector, always for today (asOfDate null)
  it("passes through the account holdings read", async () => {
    mockInvoke.mockResolvedValue(makeDetailsResponse());

    const result = await globalPerformanceGateway.getAccountHoldings("account-1");

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.data.holdings[0]?.asset_id).toBe("asset-1");
    }
    expect(mockInvoke).toHaveBeenCalledWith("get_account_details", {
      accountId: "account-1",
      asOfDate: null,
    });
  });

  // F27 — gateway does NOT throw; it returns the error result unchanged
  it("passes through AccountNotFound error result", async () => {
    const err: AccountError = { code: "AccountNotFound", account_id: "no-such-account" };
    mockInvoke.mockRejectedValue(err);

    const result = await globalPerformanceGateway.getAccountHoldings("no-such-account");

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("AccountNotFound");
    }
  });
});
