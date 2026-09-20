import { describe, expect, it } from "vitest";
import type {
  AccountDetailsResponse,
  AssetPriceSource,
  ClosedHoldingDetail,
  HoldingDetail,
} from "@/bindings";
import {
  assetPriceMutationErrorToI18n,
  dividendErrorToI18n,
  formatFxStaleness,
  formatSource,
  formatStaleness,
  freeSharesErrorToI18n,
  type HoldingRowViewModel,
  holdingNoteErrorToI18n,
  interestErrorToI18n,
  managementFeeErrorToI18n,
  performanceColumnKey,
  priceHistoryBackfillErrorToI18n,
  priceHistoryBackfillOutcomeToMessage,
  priceRefreshLockErrorToI18n,
  selectPerformanceCell,
  toAccountSummary,
  toClosedHoldingRow,
  toHoldingRow,
  toPriceableAssets,
} from "./presenter";

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

const makeClosedHolding = (overrides: Partial<ClosedHoldingDetail> = {}): ClosedHoldingDetail => ({
  asset_id: "asset-2",
  asset_name: "Closed Corp",
  asset_reference: "CLSD",
  realized_pnl: 0,
  dividends_received: 0,
  last_sold_date: "2024-12-31",
  ...overrides,
});

const makeResponse = (overrides: Partial<AccountDetailsResponse> = {}): AccountDetailsResponse => ({
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

describe("toHoldingRow", () => {
  it("formats fractional quantity trimming trailing zeros", () => {
    const row = toHoldingRow(makeHolding({ quantity: 1_500_000 }));
    expect(row.quantity).toBe("1,5");
  });

  it("formats a whole quantity with no decimals", () => {
    const row = toHoldingRow(makeHolding({ quantity: 12_000_000 }));
    expect(row.quantity).toBe("12");
  });

  it("formats averagePrice with 2 decimals when value is 10 or above", () => {
    const row = toHoldingRow(makeHolding({ average_price: 150_000_000 }));
    expect(row.averagePrice).toBe("150,00");
  });

  it("formats averagePrice with 3 decimals when value is below 10", () => {
    const row = toHoldingRow(makeHolding({ average_price: 7_125_000 }));
    expect(row.averagePrice).toBe("7,125");
  });

  it("formats averagePrice with 2 decimals at exactly 10 (boundary)", () => {
    const row = toHoldingRow(makeHolding({ average_price: 10_000_000 }));
    expect(row.averagePrice).toBe("10,00");
  });

  it("formats currentValue as current_price × quantity with 2 decimals (MKT-143)", () => {
    const row = toHoldingRow(makeHolding({ current_price: 150_000_000, quantity: 2_000_000 }));
    expect(row.currentValue).toBe("300,00");
  });

  it("shows currentValue as a dash when no price is recorded (MKT-143)", () => {
    const row = toHoldingRow(makeHolding({ current_price: null }));
    expect(row.currentValue).toBe("—");
  });

  it("maps asset metadata fields correctly", () => {
    const row = toHoldingRow(makeHolding());
    expect(row.assetId).toBe("asset-1");
    expect(row.assetName).toBe("Apple Inc");
    expect(row.assetReference).toBe("AAPL");
  });

  it("formats realizedPnl with 2 decimals (SEL-042)", () => {
    const row = toHoldingRow(makeHolding({ realized_pnl: 5_000_000 }));
    expect(row.realizedPnl).toBe("5,00");
    expect(row.realizedPnlRaw).toBe(5_000_000);
  });

  it("passes quantityMicro as raw value for sell modal (SEL-010)", () => {
    const row = toHoldingRow(makeHolding({ quantity: 3_500_000 }));
    expect(row.quantityMicro).toBe(3_500_000);
  });

  // FXR-090 — no FX rate date → no staleness label
  it("derives no fxStaleness when fx_rate_date is null", () => {
    const row = toHoldingRow(makeHolding({ fx_rate_date: null }));
    expect(row.fxStaleness).toBeNull();
  });

  // FXR-090 — an FX rate date → a currency staleness label
  it("derives an fxStaleness label from fx_rate_date", () => {
    const row = toHoldingRow(makeHolding({ fx_rate_date: "2020-01-01" }));
    expect(row.fxStaleness?.key).toMatch(/^currency\.rate_staleness_/);
  });
});

describe("toAccountSummary", () => {
  it("isEmpty true when total_holding_count is 0", () => {
    const summary = toAccountSummary(makeResponse({ total_holding_count: 0, holdings: [] }));
    expect(summary.isEmpty).toBe(true);
    expect(summary.isAllClosed).toBe(false);
  });

  it("isAllClosed true when holdings exist but active list is empty (ACD-034)", () => {
    const summary = toAccountSummary(
      makeResponse({
        total_holding_count: 2,
        holdings: [],
        closed_holdings: [makeClosedHolding()],
      }),
    );
    expect(summary.isEmpty).toBe(false);
    expect(summary.isAllClosed).toBe(true);
  });

  it("neither isEmpty nor isAllClosed when active holdings present", () => {
    const summary = toAccountSummary(makeResponse());
    expect(summary.isEmpty).toBe(false);
    expect(summary.isAllClosed).toBe(false);
  });
});

describe("toClosedHoldingRow", () => {
  // ACD-044 — closed holding detail maps to a view model row
  it("maps closed holding metadata fields (ACD-044)", () => {
    const row = toClosedHoldingRow(makeClosedHolding());
    expect(row.assetId).toBe("asset-2");
    expect(row.assetName).toBe("Closed Corp");
    expect(row.assetReference).toBe("CLSD");
  });

  // ACD-049 — realized P&L formatted to 2 decimal places
  it("formats realizedPnl with 2 decimals (ACD-049)", () => {
    const row = toClosedHoldingRow(makeClosedHolding({ realized_pnl: 15_000_000 }));
    expect(row.realizedPnl).toBe("15,00");
  });

  // ACD-049 — raw realized P&L exposed for sign-based colour styling
  it("exposes realizedPnlRaw as micro-unit number (ACD-049)", () => {
    const row = toClosedHoldingRow(makeClosedHolding({ realized_pnl: -5_000_000 }));
    expect(row.realizedPnlRaw).toBe(-5_000_000);
  });

  // ACD-049 — last_sold_date passed through verbatim
  it("passes lastSoldDate through verbatim (ACD-049)", () => {
    const row = toClosedHoldingRow(makeClosedHolding({ last_sold_date: "2025-06-15" }));
    expect(row.lastSoldDate).toBe("2025-06-15");
  });

  // DIV-073 — dividends received formatted + raw exposed
  it("formats dividendsReceived with 2 decimals and exposes raw (DIV-073)", () => {
    const row = toClosedHoldingRow(makeClosedHolding({ dividends_received: 5_000_000 }));
    expect(row.dividendsReceived).toBe("5,00");
    expect(row.dividendsReceivedRaw).toBe(5_000_000);
  });

  // Total revenues = realized P&L + dividends
  it("computes totalRevenues as realized P&L + dividends", () => {
    const row = toClosedHoldingRow(
      makeClosedHolding({ realized_pnl: 15_000_000, dividends_received: 5_000_000 }),
    );
    expect(row.totalRevenues).toBe("20,00");
    expect(row.totalRevenuesRaw).toBe(20_000_000);
  });

  // ACD-050 — empty closed_holdings list → hasClosedHoldings false
  it("toAccountSummary hasClosedHoldings is false when closed_holdings is empty (ACD-050)", () => {
    const summary = toAccountSummary(makeResponse({ closed_holdings: [] }));
    expect(summary.hasClosedHoldings).toBe(false);
  });

  // ACD-044/ACD-048 — hasClosedHoldings is true when closed_holdings is non-empty
  it("toAccountSummary hasClosedHoldings is true when closed_holdings is non-empty (ACD-044)", () => {
    const summary = toAccountSummary(makeResponse({ closed_holdings: [makeClosedHolding()] }));
    expect(summary.hasClosedHoldings).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Market-price presenter stubs (MKT-NNN)
// All assertions below are intentionally failing — implement presenter.ts to fix.
// Types used: HoldingDetail, AccountDetailsResponse, HoldingRowViewModel, AccountSummaryViewModel
// ---------------------------------------------------------------------------

describe("toHoldingRow — market price fields (MKT)", () => {
  // MKT-010 — "Enter price" action available on active holding rows
  it("MKT-010 — canEnterPrice is true on active holding rows", () => {
    const row = toHoldingRow(makeHolding());
    expect(row.canEnterPrice).toBe(true);
  });

  // MKT-030 — current price column: formatted price with adaptive precision
  it("MKT-030 — currentPrice is 'present' with formatted value when current_price is set", () => {
    const row = toHoldingRow(makeHolding({ current_price: 150_500_000 }));
    expect(row.currentPrice).toEqual({ kind: "present", formatted: "150,50" });
  });

  it("MKT-030 — currentPrice uses 3 decimals when value is below 10", () => {
    const row = toHoldingRow(makeHolding({ current_price: 4_500_000 }));
    expect(row.currentPrice).toEqual({ kind: "present", formatted: "4,500" });
  });

  // MKT-032 — diagnostic 'missing_ticker' when asset_reference is empty
  it("MKT-032 — currentPrice is 'missing_ticker' when current_price is null and asset_reference is empty", () => {
    const row = toHoldingRow(makeHolding({ current_price: null, asset_reference: "" }));
    expect(row.currentPrice).toEqual({ kind: "missing_ticker" });
  });

  // MKT-032 — diagnostic 'no_price_available' when reference present but no price
  it("MKT-032 — currentPrice is 'no_price_available' when current_price is null and asset_reference is set", () => {
    const row = toHoldingRow(makeHolding({ current_price: null, asset_reference: "AAPL" }));
    expect(row.currentPrice).toEqual({ kind: "no_price_available" });
  });

  // MKT-030 — currentPriceDate passed through for "as of {date}" label; null when no price
  it("MKT-030 — currentPriceDate is the ISO date string when present, null otherwise", () => {
    const withDate = toHoldingRow(makeHolding({ current_price_date: "2026-04-25" }));
    expect(withDate.currentPriceDate).toBe("2026-04-25");
    const noDate = toHoldingRow(makeHolding({ current_price_date: null }));
    expect(noDate.currentPriceDate).toBeNull();
  });

  // MKT-032 — "—" in unrealized P&L column when unrealized_pnl is null
  it("MKT-032 — unrealizedPnl is '—' when unrealized_pnl is null", () => {
    const row = toHoldingRow(makeHolding({ unrealized_pnl: null }));
    expect(row.unrealizedPnl).toBe("—");
  });

  // MKT-032 — "—" in performance % column when performance_pct is null
  it("MKT-032 — performancePct is '—' when performance_pct is null", () => {
    const row = toHoldingRow(makeHolding({ performance_pct: null }));
    expect(row.performancePct).toBe("—");
  });

  // MKT-034 — currency mismatch: unrealized_pnl null but current_price non-null
  it("MKT-034 — unrealizedPnl is '—' and performancePct is '—' when unrealized_pnl is null but current_price is set", () => {
    const row = toHoldingRow(
      makeHolding({
        current_price: 110_000_000,
        unrealized_pnl: null,
        performance_pct: null,
      }),
    );
    expect(row.unrealizedPnl).toBe("—");
    expect(row.performancePct).toBe("—");
  });

  // MKT-034 — currentPrice still formatted on currency mismatch
  it("MKT-034 — currentPrice is formatted even when unrealized_pnl is null (currency mismatch)", () => {
    const row = toHoldingRow(makeHolding({ current_price: 110_000_000, unrealized_pnl: null }));
    expect(row.currentPrice).toEqual({ kind: "present", formatted: "110,00" });
  });
});

// ---------------------------------------------------------------------------
// Cash tracking — CSH-090/091/094/098
// ---------------------------------------------------------------------------

const makeCashHolding = (overrides: Partial<HoldingDetail> = {}): HoldingDetail =>
  makeHolding({
    asset_id: "system-cash-eur",
    asset_name: "Cash EUR",
    asset_reference: "EUR",
    quantity: 500_000_000,
    average_price: 1_000_000,
    cost_basis: 500_000_000,
    realized_pnl: 0,
    asset_currency: "EUR",
    current_price: null,
    current_price_date: null,
    unrealized_pnl: null,
    performance_pct: null,
    ...overrides,
  });

describe("toHoldingRow — cash variant (CSH-090/091)", () => {
  // CSH-090 — system Cash Asset id starts with "system-cash-"; presenter detects via prefix
  it("flags isCash when asset_id starts with system-cash-", () => {
    const row = toHoldingRow(makeCashHolding());
    expect(row.isCash).toBe(true);
  });

  it("non-cash holdings have isCash false", () => {
    const row = toHoldingRow(makeHolding());
    expect(row.isCash).toBe(false);
  });

  // CSH-091 — cash row has no cost basis / average price / realized P&L cells
  it("cash row leaves averagePrice / currentValue / realizedPnl blank", () => {
    const row = toHoldingRow(makeCashHolding());
    expect(row.averagePrice).toBe("");
    expect(row.currentValue).toBe("");
    expect(row.realizedPnl).toBe("");
  });

  // CSH-091 — cash row has no Buy/Sell/Inspect actions, no price entry
  it("cash row disables canEnterPrice and clears market-price cells", () => {
    const row = toHoldingRow(makeCashHolding());
    expect(row.canEnterPrice).toBe(false);
    expect(row.currentPrice).toEqual({ kind: "present", formatted: "" });
    expect(row.unrealizedPnl).toBe("");
    expect(row.performancePct).toBe("");
  });

  // CSH-090 — quantity rendered as a 2-decimal amount (currency), not 6-decimal qty
  it("formats cash quantity with 2 decimals", () => {
    const row = toHoldingRow(makeCashHolding({ quantity: 250_500_000 }));
    expect(row.quantity).toBe("250,50");
  });
});

describe("toAccountSummary — cash totals (CSH-094/098)", () => {
  // CSH-094 — totalGlobalValue passed through, formatted with 2 decimals
  it("formats totalGlobalValue with 2 decimals", () => {
    const summary = toAccountSummary(makeResponse({ total_global_value: 250_000_000 }));
    expect(summary.totalGlobalValue).toBe("250,00");
    expect(summary.totalGlobalValueRaw).toBe(250_000_000);
  });

  // CSH-019/095 — hasCashHolding reflects presence of a cash holding with quantity > 0
  it("hasCashHolding true when holdings include a non-zero cash row", () => {
    const summary = toAccountSummary(
      makeResponse({
        holdings: [makeHolding(), makeCashHolding()],
        total_holding_count: 2,
      }),
    );
    expect(summary.hasCashHolding).toBe(true);
  });

  it("hasCashHolding false when no cash holding present", () => {
    const summary = toAccountSummary(makeResponse());
    expect(summary.hasCashHolding).toBe(false);
  });

  // CSH-098 — cash row excluded from isEmpty / isAllClosed gating. Under eager cash a
  // fresh account always has the Cash Holding (total_holding_count >= 1), so it must
  // read "No positions yet" (isEmpty) — NOT "All positions closed" (isAllClosed).
  it("fresh cash-only account is isEmpty, not isAllClosed (CSH-098)", () => {
    const summary = toAccountSummary(
      makeResponse({
        holdings: [makeCashHolding()],
        total_holding_count: 1,
        closed_holdings: [],
      }),
    );
    expect(summary.isEmpty).toBe(true);
    expect(summary.isAllClosed).toBe(false);
  });

  it("isAllClosed true when only cash is active but closed holdings exist", () => {
    const summary = toAccountSummary(
      makeResponse({
        holdings: [makeCashHolding()],
        total_holding_count: 2,
        closed_holdings: [makeClosedHolding()],
      }),
    );
    expect(summary.isAllClosed).toBe(true);
    expect(summary.isEmpty).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// formatStaleness — pure helper (MKT-140)
// ---------------------------------------------------------------------------

describe("formatStaleness", () => {
  const today = new Date("2026-05-17");

  // MKT-140 — no date → null (caller renders no label)
  it("returns null when currentPriceDate is null", () => {
    expect(formatStaleness(null, today)).toBeNull();
  });

  // MKT-140 — same day → today i18n key
  it("returns the today i18n key when currentPriceDate equals today", () => {
    expect(formatStaleness("2026-05-17", today)).toEqual({ key: "mkt.staleness_today" });
  });

  // MKT-140 — one day ago → days_ago i18n key with days=1
  it("returns the days_ago i18n key with days=1 when currentPriceDate is one day before today", () => {
    expect(formatStaleness("2026-05-16", today)).toEqual({
      key: "mkt.staleness_days_ago",
      params: { days: 1 },
    });
  });

  // MKT-140 — multiple days ago → days_ago i18n key with days=N
  it("returns the days_ago i18n key with days=7 when currentPriceDate is seven days before today", () => {
    expect(formatStaleness("2026-05-10", today)).toEqual({
      key: "mkt.staleness_days_ago",
      params: { days: 7 },
    });
  });

  // MKT-140 — large delta (e.g. stale data after holiday)
  it("returns the days_ago i18n key with days=30 when currentPriceDate is thirty days before today", () => {
    expect(formatStaleness("2026-04-17", today)).toEqual({
      key: "mkt.staleness_days_ago",
      params: { days: 30 },
    });
  });
});

// ---------------------------------------------------------------------------
// formatFxStaleness — pure helper (FXR-090)
// ---------------------------------------------------------------------------

describe("formatFxStaleness", () => {
  const today = new Date("2026-05-17");

  // FXR-090 — no FX rate date → null (no staleness label shown)
  it("returns null when fxRateDate is null", () => {
    expect(formatFxStaleness(null, today)).toBeNull();
  });

  // FXR-090 — rate dated today → the currency "today" key
  it("returns the rate_staleness_today key when the rate is from today", () => {
    expect(formatFxStaleness("2026-05-17", today)).toEqual({
      key: "currency.rate_staleness_today",
    });
  });

  // FXR-090 — older rate → the days_old key with the day delta
  it("returns the rate_staleness_days_old key with days=4 when the rate is four days old", () => {
    expect(formatFxStaleness("2026-05-13", today)).toEqual({
      key: "currency.rate_staleness_days_old",
      params: { days: 4 },
    });
  });

  // FXR-090 — unparseable date → null
  it("returns null when fxRateDate is not a valid date", () => {
    expect(formatFxStaleness("not-a-date", today)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// formatSource — pure helper (MKT-141, MKT-142)
// ---------------------------------------------------------------------------

describe("formatSource", () => {
  // MKT-142 — null source (no price recorded) → null
  it("returns null when source is null", () => {
    expect(formatSource(null)).toBeNull();
  });

  // MKT-101 — Manual source → i18n key
  it("returns mkt.source_manual for Manual source", () => {
    const source: AssetPriceSource = "Manual";
    expect(formatSource(source)).toBe("mkt.source_manual");
  });

  // MKT-102 — YahooFinance source → i18n key
  it("returns mkt.source_yahoo for YahooFinance source", () => {
    const source: AssetPriceSource = "YahooFinance";
    expect(formatSource(source)).toBe("mkt.source_yahoo");
  });
});

// ---------------------------------------------------------------------------
// toHoldingRow — staleness + source label derived fields (MKT-140, MKT-142)
// ---------------------------------------------------------------------------

describe("toHoldingRow — staleness and sourceLabel fields (MKT-140, MKT-142)", () => {
  // MKT-140 — staleness field: no price → null
  it("staleness is null when current_price_date is null", () => {
    const row = toHoldingRow(makeHolding({ current_price_date: null }));
    expect(row.staleness).toBeNull();
  });

  // MKT-142 — sourceLabel field: no price → null
  it("sourceLabel is null when current_price_source is null", () => {
    const row = toHoldingRow(makeHolding({ current_price_source: null }));
    expect(row.sourceLabel).toBeNull();
  });

  // MKT-142 — sourceLabel: Manual source → i18n key
  it("sourceLabel is mkt.source_manual when current_price_source is Manual", () => {
    const row = toHoldingRow(
      makeHolding({ current_price_source: "Manual", current_price: 100_000_000 }),
    );
    expect(row.sourceLabel).toBe("mkt.source_manual");
  });

  // MKT-142 — sourceLabel: YahooFinance source → i18n key
  it("sourceLabel is mkt.source_yahoo when current_price_source is YahooFinance", () => {
    const row = toHoldingRow(
      makeHolding({ current_price_source: "YahooFinance", current_price: 100_000_000 }),
    );
    expect(row.sourceLabel).toBe("mkt.source_yahoo");
  });
});

// F27 layer-3 presenter — variant coverage across the AssetError price surface.
// One payload-bearing variant (`InvalidDateFormat { date }`) gets interpolation;
// the rest fall through to the flat error key.
describe("assetPriceMutationErrorToI18n", () => {
  it("InvalidDateFormat interpolates the offending date payload", () => {
    expect(
      assetPriceMutationErrorToI18n({ code: "InvalidDateFormat", date: "2024/13/45" }),
    ).toEqual({
      key: "error.InvalidDateFormat",
      vars: { date: "2024/13/45" },
    });
  });

  it("AssetNotFound (carries id payload) maps to its flat key", () => {
    expect(assetPriceMutationErrorToI18n({ code: "AssetNotFound", id: "asset-1" })).toEqual({
      key: "error.AssetNotFound",
    });
  });

  it("PriceNotFound (carries asset_id + date payload) maps to its flat key", () => {
    expect(
      assetPriceMutationErrorToI18n({
        code: "PriceNotFound",
        asset_id: "asset-1",
        date: "2024-01-15",
      }),
    ).toEqual({ key: "error.PriceNotFound" });
  });

  it.each([
    "DatabaseError",
    "NotPositive",
    "NonFinite",
    "DateInFuture",
  ] as const)("%s unit variant maps to its flat error key", (code) => {
    expect(assetPriceMutationErrorToI18n({ code })).toEqual({ key: `error.${code}` });
  });
});

// MKT-156 — F27 presenter for the price-refresh lock commands' error surface
describe("priceRefreshLockErrorToI18n", () => {
  it("AssetNotFound (carries id payload) maps to its flat key", () => {
    expect(priceRefreshLockErrorToI18n({ code: "AssetNotFound", id: "asset-1" })).toEqual({
      key: "error.AssetNotFound",
    });
  });

  it.each([
    "CashAssetNotEditable",
    "DatabaseError",
  ] as const)("%s unit variant maps to its flat error key", (code) => {
    expect(priceRefreshLockErrorToI18n({ code })).toEqual({ key: `error.${code}` });
  });
});

// ---------------------------------------------------------------------------
// dividendErrorToI18n — F27 presenter for record_dividend error surface (DIV)
// Reachable codes per contract: AccountNotFound, DatabaseError (AccountError);
// AssetNotFound, AssetNotHeld, DividendOnCashAsset (DividendTask);
// AmountNotPositive, ExchangeRateNotPositive, DateInFuture, DateTooOld, InvalidDate
// (AccountError). Unknown codes fall to error.Unknown.
// ---------------------------------------------------------------------------

describe("dividendErrorToI18n", () => {
  it("AccountNotFound (carries account_id payload) maps to error.AccountNotFound", () => {
    expect(dividendErrorToI18n({ code: "AccountNotFound", account_id: "acc-1" })).toEqual({
      key: "error.AccountNotFound",
    });
  });

  it.each([
    "DatabaseError",
    "AssetNotFound",
    "AssetNotHeld",
    "DividendOnCashAsset",
    "AmountNotPositive",
    "ExchangeRateNotPositive",
    "DateInFuture",
    "DateTooOld",
    "InvalidDate",
  ] as const)("%s maps to its flat error key", (code) => {
    expect(dividendErrorToI18n({ code })).toEqual({ key: `error.${code}` });
  });

  it("an unrecognised code falls through to error.Unknown", () => {
    // Cast needed because TypeScript narrows to known union members — this
    // exercises the default branch that guards against future wire codes.
    expect(dividendErrorToI18n({ code: "SomeUnknownCode" } as never)).toEqual({
      key: "error.Unknown",
    });
  });
});

// ---------------------------------------------------------------------------
// freeSharesErrorToI18n — F27 presenter for the free-shares error surfaces (FSD).
// Create path (FreeSharesError): AccountNotFound, AssetNotFound, AssetNotHeld,
// FreeSharesOnCashAsset, QuantityNotPositive, InvalidDate, DateInFuture,
// DateTooOld, DatabaseError. Edit path (AccountError via
// correct_transaction): e.g. CascadingOversell, TransactionNotFound. Every flat
// { code } maps to error.{code}.
// ---------------------------------------------------------------------------

describe("freeSharesErrorToI18n", () => {
  it.each([
    "AccountNotFound",
    "AssetNotFound",
    "AssetNotHeld",
    "FreeSharesOnCashAsset",
    "QuantityNotPositive",
    "InvalidDate",
    "DateInFuture",
    "DateTooOld",
    "DatabaseError",
  ] as const)("create-path %s maps to its flat error key", (code) => {
    expect(freeSharesErrorToI18n({ code } as never)).toEqual({ key: `error.${code}` });
  });

  it.each([
    "CascadingOversell",
    "TransactionNotFound",
  ] as const)("edit-path %s maps to its flat error key", (code) => {
    expect(freeSharesErrorToI18n({ code } as never)).toEqual({ key: `error.${code}` });
  });

  it.each([
    "NegativeQuantity",
    "NegativeAveragePrice",
  ] as const)("keyless holding-internal code %s falls back to error.Unknown", (code) => {
    expect(freeSharesErrorToI18n({ code } as never)).toEqual({ key: "error.Unknown" });
  });
});

// ---------------------------------------------------------------------------
// interestErrorToI18n — F27 presenter for the interest error surfaces (INT).
// Create path (InterestError): AccountNotFound, AssetNotFound, AssetNotHeld,
// InterestNotEligible, InterestAmountInvalid, PercentageNotPositive,
// PercentageAboveHundred, QuantityNotPositive, InvalidDate, DateInFuture,
// DateTooOld, DatabaseError.
// Edit path (AccountError via correct_transaction): e.g. CascadingOversell,
// TransactionNotFound. Every flat { code } maps to error.{code}.
// ---------------------------------------------------------------------------

describe("interestErrorToI18n (INT-021/011/040)", () => {
  it.each([
    "AccountNotFound",
    "AssetNotFound",
    "AssetNotHeld",
    "InterestNotEligible",
    "InterestAmountInvalid",
    "PercentageNotPositive",
    "PercentageAboveHundred",
    "QuantityNotPositive",
    "InvalidDate",
    "DateInFuture",
    "DateTooOld",
    "DatabaseError",
  ] as const)("create-path %s maps to its flat error key", (code) => {
    expect(interestErrorToI18n({ code } as never)).toEqual({ key: `error.${code}` });
  });

  it.each([
    "CascadingOversell",
    "TransactionNotFound",
  ] as const)("edit-path %s maps to its flat error key", (code) => {
    expect(interestErrorToI18n({ code } as never)).toEqual({ key: `error.${code}` });
  });

  it.each([
    "NegativeQuantity",
    "NegativeAveragePrice",
  ] as const)("keyless holding-internal code %s falls back to error.Unknown", (code) => {
    expect(interestErrorToI18n({ code } as never)).toEqual({ key: "error.Unknown" });
  });
});

describe("managementFeeErrorToI18n (FEE-021/011/027)", () => {
  // FEE-028 — the rejection carries the held quantity; the message shows it formatted.
  it("passes the held quantity of ResultingQuantityNotBelowHeld as a formatted var", () => {
    expect(
      managementFeeErrorToI18n({
        code: "ResultingQuantityNotBelowHeld",
        held_quantity: 25_000_000,
      }),
    ).toEqual({ key: "error.ResultingQuantityNotBelowHeld", vars: { held: "25" } });
  });

  it.each([
    "AssetNotFound",
    "AssetNotHeld",
    "ManagementFeeOnCashAsset",
    "PercentageNotPositive",
    "PercentageAboveHundred",
    "CascadingOversell",
    "QuantityNotPositive",
    "RateNotPositive",
    "RateAboveHundred",
    "EndBeforeStart",
    "ScheduleAlreadyExists",
    "ScheduleNotFound",
    "DatabaseError",
  ] as const)("%s maps to its flat error key", (code) => {
    expect(managementFeeErrorToI18n({ code } as never)).toEqual({ key: `error.${code}` });
  });
});

describe("holdingNoteErrorToI18n (HNO-011/020/021)", () => {
  it.each([
    "NoteTextEmpty",
    "NoteTextTooLong",
    "ThresholdNotPositive",
    "ThresholdIncomplete",
    "NoteOnCashAsset",
    "NoteOnUnheldAsset",
    "AccountNotFound",
    "AssetNotFound",
    "DatabaseError",
  ] as const)("%s maps to its flat error key", (code) => {
    expect(holdingNoteErrorToI18n({ code } as never)).toEqual({ key: `error.${code}` });
  });

  it.each([
    "NegativeQuantity",
    "NegativeAveragePrice",
  ] as const)("keyless holding-internal code %s falls back to error.Unknown", (code) => {
    expect(holdingNoteErrorToI18n({ code } as never)).toEqual({ key: "error.Unknown" });
  });
});

describe("toHoldingRow / toAccountSummary — management fees (FEE-052/053)", () => {
  it("formats a holding's cumulative management fees", () => {
    const row = toHoldingRow(makeHolding({ management_fees: 4_250_000 }));
    expect(row.managementFees).toBe("4,25");
  });

  it("leaves the cash row's management fees blank", () => {
    const row = toHoldingRow(makeHolding({ asset_id: "system-cash-EUR" }));
    expect(row.managementFees).toBe("");
  });

  it("formats the account total management fees", () => {
    const summary = toAccountSummary(makeResponse({ total_management_fees: 12_500_000 }));
    expect(summary.totalManagementFees).toBe("12,50");
    expect(summary.totalManagementFeesRaw).toBe(12_500_000);
  });
});

// ---------------------------------------------------------------------------
// toHoldingRow — DIV-072: dividendsReceived and totalReturnPct new fields
// ---------------------------------------------------------------------------

describe("toHoldingRow — dividend fields (DIV-072)", () => {
  // DIV-072 — dividendsReceived always shown, formatted with 2 decimals
  it("formats dividendsReceived with 2 decimals when non-zero (DIV-072)", () => {
    const row = toHoldingRow(makeHolding({ dividends_received: 50_000_000 }));
    expect(row.dividendsReceived).toBe("50,00");
  });

  it("formats dividendsReceived as '0,00' when zero (DIV-070)", () => {
    const row = toHoldingRow(makeHolding({ dividends_received: 0 }));
    expect(row.dividendsReceived).toBe("0,00");
  });

  // DIV-072 — totalReturnPct: formatted with 2 decimals + % suffix when non-null
  it("formats totalReturnPct with 2 decimals and % suffix when non-null (DIV-071)", () => {
    const row = toHoldingRow(makeHolding({ total_return_pct: 8_250_000 }));
    expect(row.totalReturnPct).toBe("8,25%");
  });

  // DIV-072 — totalReturnPct: '—' when null (same conditions as performance_pct)
  it("totalReturnPct is '—' when total_return_pct is null (DIV-072)", () => {
    const row = toHoldingRow(makeHolding({ total_return_pct: null }));
    expect(row.totalReturnPct).toBe("—");
  });

  // Cash row — dividendsReceived and totalReturnPct are blank (not applicable)
  it("cash row has blank dividendsReceived and totalReturnPct", () => {
    const row = toHoldingRow(makeCashHolding());
    expect(row.dividendsReceived).toBe("");
    expect(row.totalReturnPct).toBe("");
  });
});

describe("toPriceableAssets", () => {
  const row = (over: Partial<HoldingRowViewModel>): HoldingRowViewModel =>
    ({
      assetId: "a",
      assetName: "A",
      assetCurrency: "EUR",
      canEnterPrice: true,
      ...over,
    }) as HoldingRowViewModel;

  it("keeps only canEnterPrice holdings, mapped to the combobox shape (MKT-011)", () => {
    const rows = [
      row({ assetId: "a1", assetName: "Apple", assetCurrency: "EUR", canEnterPrice: true }),
      row({ assetId: "cash", assetName: "Cash", assetCurrency: "EUR", canEnterPrice: false }),
      row({ assetId: "a2", assetName: "Tesla", assetCurrency: "USD", canEnterPrice: true }),
    ];
    expect(toPriceableAssets(rows)).toEqual([
      { assetId: "a1", assetName: "Apple", assetCurrency: "EUR" },
      { assetId: "a2", assetName: "Tesla", assetCurrency: "USD" },
    ]);
  });
});

describe("toHoldingRow — weight % (ACD-052)", () => {
  it("formats the weight of a priced holding against the Global Value", () => {
    // 220.00 of a 1000.00 account → 22.00%
    const row = toHoldingRow(makeHolding({ market_value: 220_000_000 }), 1_000_000_000);
    expect(row.weightPct).toBe("22,00%");
  });

  it("renders the dash when the holding has no market value", () => {
    const row = toHoldingRow(makeHolding({ market_value: null }), 1_000_000_000);
    expect(row.weightPct).toBe("—");
  });

  it("renders the dash when the Global Value is zero", () => {
    const row = toHoldingRow(makeHolding({ market_value: 220_000_000 }), 0);
    expect(row.weightPct).toBe("—");
  });

  it("computes the cash row weight from its balance", () => {
    // cash 250.00 of a 1000.00 account → 25.00%
    const row = toHoldingRow(
      makeHolding({
        asset_id: "system-cash-eur",
        quantity: 250_000_000,
        market_value: 250_000_000,
      }),
      1_000_000_000,
    );
    expect(row.isCash).toBe(true);
    expect(row.weightPct).toBe("25,00%");
  });
});

describe("toAccountSummary — net cash input (ACD-053)", () => {
  it("formats totalNetCashInput from the response", () => {
    const summary = toAccountSummary(makeResponse({ total_net_cash_input: 120_000_000 }));
    expect(summary.totalNetCashInput).toBe("120,00");
  });

  it("formats a negative net cash input with its sign", () => {
    const summary = toAccountSummary(makeResponse({ total_net_cash_input: -30_000_000 }));
    expect(summary.totalNetCashInput).toContain("30,00");
    expect(summary.totalNetCashInput).toMatch(/^-/);
  });
});

describe("toHoldingRow — fee schedule rate (FEE-074)", () => {
  it("formats the active schedule's annual rate", () => {
    const row = toHoldingRow(makeHolding({ fee_rate_percent_micros: 1_500_000 }));
    expect(row.feeRatePct).toBe("1,50%");
  });

  it("is null when the holding has no active schedule", () => {
    const row = toHoldingRow(makeHolding({ fee_rate_percent_micros: null }));
    expect(row.feeRatePct).toBeNull();
  });
});

describe("toHoldingRow — windowed performance (ACD-054/057)", () => {
  it("formats each computable window through the performance-% pipeline", () => {
    const row = toHoldingRow(
      makeHolding({
        period_performance: {
          ytd: 5_250_000,
          one_year: -1_000_000,
          two_years: 12_345_678,
          five_years: 0,
          ten_years: 100_000_000,
        },
      }),
    );
    expect(row.periodPerformance.ytd).toEqual({ formatted: "5,25%", raw: 5_250_000 });
    expect(row.periodPerformance.one_year).toEqual({ formatted: "-1,00%", raw: -1_000_000 });
    expect(row.periodPerformance.two_years).toEqual({ formatted: "12,35%", raw: 12_345_678 });
    expect(row.periodPerformance.five_years).toEqual({ formatted: "0,00%", raw: 0 });
    expect(row.periodPerformance.ten_years).toEqual({ formatted: "100,00%", raw: 100_000_000 });
  });

  it("renders a dash for a window that is not computable (ACD-057)", () => {
    const row = toHoldingRow(makeHolding());
    for (const window of ["ytd", "one_year", "two_years", "five_years", "ten_years"] as const) {
      expect(row.periodPerformance[window]).toEqual({ formatted: "—", raw: null });
    }
  });

  it("keeps every window blank for the cash row (ACD-054)", () => {
    const row = toHoldingRow(makeHolding({ asset_id: "system-cash-eur" }));
    for (const window of ["ytd", "one_year", "two_years", "five_years", "ten_years"] as const) {
      expect(row.periodPerformance[window]).toEqual({ formatted: "", raw: null });
    }
  });
});

describe("selectPerformanceCell (ACD-054)", () => {
  const row = toHoldingRow(
    makeHolding({
      current_price: 150_000_000,
      unrealized_pnl: -20_000_000,
      performance_pct: -10_000_000,
      period_performance: {
        ytd: 5_250_000,
        one_year: -1_000_000,
        two_years: 2_000_000,
        five_years: 5_000_000,
        ten_years: null,
      },
    }),
  );

  it("since_start returns the existing figure colored by the unrealized-P&L sign", () => {
    expect(selectPerformanceCell(row, "since_start")).toEqual({
      formatted: "-10,00%",
      raw: -20_000_000,
    });
  });

  it("each window maps to its own period_performance field", () => {
    expect(selectPerformanceCell(row, "ytd")).toEqual({ formatted: "5,25%", raw: 5_250_000 });
    expect(selectPerformanceCell(row, "one_year")).toEqual({
      formatted: "-1,00%",
      raw: -1_000_000,
    });
    expect(selectPerformanceCell(row, "two_years")).toEqual({
      formatted: "2,00%",
      raw: 2_000_000,
    });
    expect(selectPerformanceCell(row, "five_years")).toEqual({
      formatted: "5,00%",
      raw: 5_000_000,
    });
    expect(selectPerformanceCell(row, "ten_years")).toEqual({ formatted: "—", raw: null });
  });
});

describe("performanceColumnKey (ACD-054)", () => {
  it("keeps the plain performance header for since_start", () => {
    expect(performanceColumnKey("since_start")).toBe("account_details.column_performance_pct");
  });

  it("suffixes the header key with the selected window", () => {
    expect(performanceColumnKey("ytd")).toBe("account_details.column_performance_pct_ytd");
    expect(performanceColumnKey("one_year")).toBe(
      "account_details.column_performance_pct_one_year",
    );
    expect(performanceColumnKey("two_years")).toBe(
      "account_details.column_performance_pct_two_years",
    );
    expect(performanceColumnKey("five_years")).toBe(
      "account_details.column_performance_pct_five_years",
    );
    expect(performanceColumnKey("ten_years")).toBe(
      "account_details.column_performance_pct_ten_years",
    );
  });
});

// ---------------------------------------------------------------------------
// toHoldingRow — SYN-040/CFR-042: derived holding inconsistency carried into
// the row view model, with an i18n message for display (F27).
// ---------------------------------------------------------------------------

describe("toHoldingRow — inconsistency (SYN-040, CFR-042)", () => {
  it("carries inconsistency as null and inconsistencyLabel as null when the holding is consistent", () => {
    const row = toHoldingRow(makeHolding({ inconsistency: null }));
    expect(row.inconsistency).toBeNull();
    expect(row.inconsistencyLabel).toBeNull();
  });

  it("carries an Oversold inconsistency through with a formatted i18n message", () => {
    const row = toHoldingRow(
      makeHolding({ inconsistency: { Oversold: { quantity: -3_000_000 } } }),
    );
    expect(row.inconsistency).toEqual({ Oversold: { quantity: -3_000_000 } });
    expect(row.inconsistencyLabel).toEqual({
      key: "sync.inconsistency.oversold",
      vars: { quantity: "-3" },
    });
  });

  it("carries a CashOverdrawn inconsistency on the cash row through with a formatted i18n message", () => {
    const row = toHoldingRow(
      makeCashHolding({ inconsistency: { CashOverdrawn: { amount: -50_250_000 } } }),
    );
    expect(row.isCash).toBe(true);
    expect(row.inconsistency).toEqual({ CashOverdrawn: { amount: -50_250_000 } });
    expect(row.inconsistencyLabel).toEqual({
      key: "sync.inconsistency.cash_overdrawn",
      vars: { amount: "-50,25" },
    });
  });
});

describe("toHoldingRow — note fields (HNO-041)", () => {
  it("passes through a null note as no note and no alarm", () => {
    const row = toHoldingRow(makeHolding());
    expect(row.noteText).toBeNull();
    expect(row.noteHasAlarm).toBe(false);
    expect(row.noteAlarmTriggered).toBe(false);
  });

  it("carries the note text without an alarm when no threshold is stored", () => {
    const row = toHoldingRow(makeHolding({ note_text: "plain reminder" }));
    expect(row.noteText).toBe("plain reminder");
    expect(row.noteHasAlarm).toBe(false);
    expect(row.noteAlarmTriggered).toBe(false);
  });

  it("flags an armed alarm from the threshold pair (not triggered)", () => {
    const row = toHoldingRow(
      makeHolding({
        note_text: "alert note",
        note_threshold_price: 150_000_000,
        note_threshold_direction: "Below",
        note_alarm_triggered: false,
      }),
    );
    expect(row.noteText).toBe("alert note");
    expect(row.noteHasAlarm).toBe(true);
    expect(row.noteAlarmTriggered).toBe(false);
  });

  it("flags a triggered alarm (HNO-030)", () => {
    const row = toHoldingRow(
      makeHolding({
        note_text: "alert note",
        note_threshold_price: 150_000_000,
        note_threshold_direction: "Above",
        note_alarm_triggered: true,
      }),
    );
    expect(row.noteHasAlarm).toBe(true);
    expect(row.noteAlarmTriggered).toBe(true);
  });

  it("cash row never carries a note (HNO-011)", () => {
    const row = toHoldingRow(makeCashHolding());
    expect(row.noteText).toBeNull();
    expect(row.noteHasAlarm).toBe(false);
    expect(row.noteAlarmTriggered).toBe(false);
  });
});

// MKT-197 — the backfill outcome and its rejections, as snackbar messages.
describe("priceHistoryBackfillOutcomeToMessage (MKT-197)", () => {
  it("states both counts as a success when at least one close was written", () => {
    expect(priceHistoryBackfillOutcomeToMessage({ written: 12, already_priced: 3 })).toEqual({
      key: "mkt.backfill.success",
      vars: { written: 12, already_priced: 3 },
      variant: "success",
    });
  });

  it("states that nothing was left to fill as information when no close was written", () => {
    expect(priceHistoryBackfillOutcomeToMessage({ written: 0, already_priced: 40 })).toEqual({
      key: "mkt.backfill.nothing_missing",
      variant: "info",
    });
  });
});

describe("priceHistoryBackfillErrorToI18n (MKT-196)", () => {
  it.each([
    "AssetNeverHeld",
    "PriceRefreshBlocked",
    "TickerNotResolved",
    "ProviderUnreachable",
  ] as const)("%s maps to its backfill-specific key", (code) => {
    expect(priceHistoryBackfillErrorToI18n({ code })).toEqual({
      key: `mkt.backfill.error.${code}`,
    });
  });

  it("AccountNotFound (carries account_id payload) maps to its flat key", () => {
    expect(
      priceHistoryBackfillErrorToI18n({ code: "AccountNotFound", account_id: "acc-1" }),
    ).toEqual({ key: "error.AccountNotFound" });
  });

  it("AssetNotFound (carries id payload) maps to its flat key", () => {
    expect(priceHistoryBackfillErrorToI18n({ code: "AssetNotFound", id: "asset-1" })).toEqual({
      key: "error.AssetNotFound",
    });
  });

  it.each([
    "CashAssetNotEditable",
    "Archived",
    "DatabaseError",
  ] as const)("%s maps to its flat error key", (code) => {
    expect(priceHistoryBackfillErrorToI18n({ code })).toEqual({ key: `error.${code}` });
  });
});
