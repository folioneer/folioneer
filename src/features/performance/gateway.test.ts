import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import type {
  AccountDetailsResponse,
  AccountError,
  AccountPerformanceResponse,
  Event,
  HoldingDetail,
  PerformancePeriod,
} from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { accountPerformanceGateway, globalPerformanceGateway } = await import("./gateway");

describe("performance gateways — subscribeToEvents (TD-011)", () => {
  it("passes the callback the generated event discriminant, not a bare string", () => {
    expectTypeOf(accountPerformanceGateway.subscribeToEvents)
      .parameter(0)
      .parameter(0)
      .toEqualTypeOf<Event["type"]>();
    expectTypeOf(globalPerformanceGateway.subscribeToEvents)
      .parameter(0)
      .parameter(0)
      .toEqualTypeOf<Event["type"]>();
  });
});

// PRF-070–074 — bridge term defaults (the gateway passes these through untouched).
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

const makeMonthRow = (overrides: Partial<PerformancePeriod> = {}): PerformancePeriod => ({
  year: 2025,
  month: 5,
  end_value: 10_000_000_000,
  ...BRIDGE_DEFAULTS,
  period_over_period: { gain: 200_000_000, pct: 2_000_000 },
  year_to_date: { gain: 350_000_000, pct: 3_500_000 },
  since_inception: { gain: 500_000_000, pct: 5_000_000 },
  annualized_yield: null,
  ...overrides,
});

const makeResponse = (
  overrides: Partial<AccountPerformanceResponse> = {},
): AccountPerformanceResponse => ({
  account_name: "My Portfolio",
  currency: "EUR",
  month_view_available: true,
  yearly: [makeYearRow()],
  monthly: [makeMonthRow()],
  ...overrides,
});

describe("accountPerformanceGateway — getAccountPerformance", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // PRF-016 happy path — gateway passes the ok result through unchanged (F27)
  it("passes through ok result with full AccountPerformanceResponse", async () => {
    const response = makeResponse();
    mockInvoke.mockResolvedValue(response);

    const result = await accountPerformanceGateway.getAccountPerformance("account-1", null);

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.data.account_name).toBe("My Portfolio");
      expect(result.data.currency).toBe("EUR");
      expect(result.data.month_view_available).toBe(true);
      expect(result.data.yearly).toHaveLength(1);
      expect(result.data.monthly).toHaveLength(1);
    }
    expect(mockInvoke).toHaveBeenCalledWith("get_account_performance", {
      accountId: "account-1",
      assetId: null,
    });
  });

  // F27 — gateway does NOT throw; it returns the error result unchanged
  // PRF-016 — AccountNotFound when account_id does not correspond to an existing account
  it("passes through AccountNotFound error result (PRF-016)", async () => {
    const err: AccountError = {
      code: "AccountNotFound",
      account_id: "no-such-account",
    };
    mockInvoke.mockRejectedValue(err);

    const result = await accountPerformanceGateway.getAccountPerformance("no-such-account", null);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("AccountNotFound");
    }
    expect(mockInvoke).toHaveBeenCalledWith("get_account_performance", {
      accountId: "no-such-account",
      assetId: null,
    });
  });

  // PRF-027 — DatabaseError when the read fails during computation
  it("passes through DatabaseError result (PRF-027)", async () => {
    const err: AccountError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountPerformanceGateway.getAccountPerformance("account-1", null);

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("DatabaseError");
    }
  });

  // PRF-043 — empty result (no transactions) is an ok result with empty arrays
  it("passes through ok result with empty yearly and monthly arrays (PRF-043)", async () => {
    const response = makeResponse({ yearly: [], monthly: [] });
    mockInvoke.mockResolvedValue(response);

    const result = await accountPerformanceGateway.getAccountPerformance("account-1", null);

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.data.yearly).toHaveLength(0);
      expect(result.data.monthly).toHaveLength(0);
    }
  });

  // PRF-013 — month_view_available=false passes through unchanged
  it("passes through month_view_available=false for ManualMonth/ManualYear accounts (PRF-013)", async () => {
    const response = makeResponse({ month_view_available: false, monthly: [] });
    mockInvoke.mockResolvedValue(response);

    const result = await accountPerformanceGateway.getAccountPerformance("account-1", null);

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.data.month_view_available).toBe(false);
      expect(result.data.monthly).toHaveLength(0);
    }
  });

  // PRF-080 — the asset scope is forwarded to the command unchanged
  it("passes the asset scope through to the command (PRF-080)", async () => {
    mockInvoke.mockResolvedValue(makeResponse());

    const result = await accountPerformanceGateway.getAccountPerformance("account-1", "asset-1");

    expect(result.status).toBe("ok");
    expect(mockInvoke).toHaveBeenCalledWith("get_account_performance", {
      accountId: "account-1",
      assetId: "asset-1",
    });
  });
});

// ---- getAccountHoldings -----------------------------------------------------

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
  ...overrides,
});

describe("accountPerformanceGateway — getAccountHoldings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // PRF-082 — the holdings read backing the asset selector passes through unchanged (F27),
  // always for today (asOfDate null)
  it("passes through ok result with the account holdings (PRF-082)", async () => {
    const response = makeDetailsResponse();
    mockInvoke.mockResolvedValue(response);

    const result = await accountPerformanceGateway.getAccountHoldings("account-1");

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.data.holdings).toHaveLength(1);
      expect(result.data.holdings[0]?.asset_id).toBe("asset-1");
    }
    expect(mockInvoke).toHaveBeenCalledWith("get_account_details", {
      accountId: "account-1",
      asOfDate: null,
    });
  });

  // F27 — gateway does NOT throw; it returns the error result unchanged
  it("passes through AccountNotFound error result", async () => {
    const err: AccountError = {
      code: "AccountNotFound",
      account_id: "no-such-account",
    };
    mockInvoke.mockRejectedValue(err);

    const result = await accountPerformanceGateway.getAccountHoldings("no-such-account");

    expect(result.status).toBe("error");
    if (result.status === "error") {
      expect(result.error.code).toBe("AccountNotFound");
    }
  });
});
