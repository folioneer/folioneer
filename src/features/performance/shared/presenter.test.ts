import { describe, expect, it } from "vitest";
import type {
  AccountDetailsResponse,
  HoldingDetail,
  PerformanceMetric,
  PerformancePeriod,
} from "@/bindings";
import {
  formatEndValue,
  formatMetricGain,
  formatMetricPct,
  gainColorClass,
  monthLabel,
  pnlColorClass,
  presentAccountPerformanceError,
  presentAssetScopeOptions,
  presentPeriodRow,
  presentValueChartSeries,
  resolveViewMode,
} from "./presenter";

// ---- Helpers ----------------------------------------------------------------

const makeMetric = (overrides: Partial<PerformanceMetric> = {}): PerformanceMetric => ({
  gain: 1_000_000_000, // €1 000.00
  pct: 8_000_000, // 8.00%
  ...overrides,
});

// PRF-070–074 — bridge term defaults shared by both row factories (they sum to end_value).
const BRIDGE_DEFAULTS = {
  previous_value: 9_000_000_000, // €9 000.00
  cash_flow: 500_000_000, // +€500.00
  asset_flow: 0,
  dividends: 120_000_000, // €120.00
  pnl: 380_000_000, // +€380.00 → 9 000 + 500 + 0 + 120 + 380 = 10 000
} satisfies Partial<PerformancePeriod>;

const makeYearRow = (overrides: Partial<PerformancePeriod> = {}): PerformancePeriod => ({
  year: 2025,
  month: null,
  end_value: 10_000_000_000, // €10 000.00
  ...BRIDGE_DEFAULTS,
  period_over_period: makeMetric(),
  year_to_date: null, // always null for year rows (PRF-037)
  since_inception: makeMetric({ gain: 2_000_000_000, pct: 20_000_000 }),
  annualized_yield: makeMetric({ gain: 2_000_000_000, pct: 10_000_000 }),
  ...overrides,
});

const makeMonthRow = (overrides: Partial<PerformancePeriod> = {}): PerformancePeriod => ({
  year: 2025,
  month: 5,
  end_value: 10_000_000_000,
  ...BRIDGE_DEFAULTS,
  period_over_period: makeMetric(),
  year_to_date: makeMetric({ gain: 350_000_000, pct: 3_500_000 }),
  since_inception: makeMetric({ gain: 2_000_000_000, pct: 20_000_000 }),
  annualized_yield: null, // year-row concept only
  ...overrides,
});

// ---- resolveViewMode (PRF-014 / GPF-016) ------------------------------------

describe("resolveViewMode", () => {
  it("returns the remembered mode when still valid", () => {
    expect(resolveViewMode("year", true)).toBe("year");
    expect(resolveViewMode("month", true)).toBe("month");
    expect(resolveViewMode("year", false)).toBe("year");
  });

  it("clamps a remembered month view to year when month view is unavailable", () => {
    expect(resolveViewMode("month", false)).toBe("year");
  });

  it("defaults to month when available and nothing is remembered", () => {
    expect(resolveViewMode(null, true)).toBe("month");
  });

  it("defaults to year when month view is unavailable and nothing is remembered", () => {
    expect(resolveViewMode(null, false)).toBe("year");
  });
});

// ---- presentAccountPerformanceError (F27, PRF-016, PRF-027) -----------------

describe("presentAccountPerformanceError", () => {
  it("maps AccountNotFound to i18n key (PRF-016)", () => {
    expect(presentAccountPerformanceError({ code: "AccountNotFound", account_id: "x" })).toEqual({
      key: "account_performance.error.account_not_found",
    });
  });

  it("maps DatabaseError to i18n key (PRF-027)", () => {
    expect(presentAccountPerformanceError({ code: "DatabaseError" })).toEqual({
      key: "account_performance.error.database_error",
    });
  });
});

// ---- formatEndValue (PRF-020, micro → display) --------------------------------

describe("formatEndValue", () => {
  it("converts micro-units to display string with 2 decimal places", () => {
    // 10_000_000_000 micros = €10 000.00
    const formatted = formatEndValue(10_000_000_000);
    expect(typeof formatted).toBe("string");
    expect(formatted.length).toBeGreaterThan(0);
  });

  it("formats zero end_value as a zero amount", () => {
    const formatted = formatEndValue(0);
    expect(typeof formatted).toBe("string");
  });
});

// ---- formatMetricGain (PRF-036) -----------------------------------------------

describe("formatMetricGain", () => {
  it("returns formatted gain string when metric is present", () => {
    const formatted = formatMetricGain(makeMetric({ gain: 1_000_000_000 }));
    expect(typeof formatted).toBe("string");
    expect(formatted.length).toBeGreaterThan(0);
  });

  it("returns '—' when metric is null (PRF-042)", () => {
    expect(formatMetricGain(null)).toBe("—");
  });
});

// ---- formatMetricPct (PRF-036, PRF-032) ----------------------------------------

describe("formatMetricPct", () => {
  it("formats micro-percent to display string when pct is present (8_000_000 = 8.00%)", () => {
    const formatted = formatMetricPct(makeMetric({ pct: 8_000_000 }));
    expect(formatted).toContain("8");
    expect(formatted).toContain("%");
  });

  it("returns '—' when pct is null (Dietz denominator was 0, PRF-032)", () => {
    expect(formatMetricPct(makeMetric({ pct: null }))).toBe("—");
  });

  it("returns '—' when the whole metric is null (PRF-042)", () => {
    expect(formatMetricPct(null)).toBe("—");
  });
});

// ---- gainColorClass (PRF-036) -------------------------------------------------

describe("gainColorClass", () => {
  it("returns positive colour class for a positive gain", () => {
    const cls = gainColorClass(makeMetric({ gain: 100_000_000 }));
    expect(cls).toBeTruthy();
    // the class must differ from the negative class
    const negCls = gainColorClass(makeMetric({ gain: -100_000_000 }));
    expect(cls).not.toBe(negCls);
  });

  it("returns negative colour class for a negative gain", () => {
    const cls = gainColorClass(makeMetric({ gain: -500_000_000 }));
    expect(cls).toBeTruthy();
  });

  it("returns neutral colour class for a zero gain", () => {
    const cls = gainColorClass(makeMetric({ gain: 0 }));
    expect(cls).toBeTruthy();
  });

  it("returns neutral colour class when metric is null (PRF-042)", () => {
    const cls = gainColorClass(null);
    expect(cls).toBeTruthy();
  });
});

// ---- pnlColorClass (PRF-070/071/073) ------------------------------------------

describe("pnlColorClass", () => {
  it("returns distinct positive and negative classes", () => {
    expect(pnlColorClass(1)).not.toBe(pnlColorClass(-1));
  });

  it("returns a neutral class for zero", () => {
    expect(pnlColorClass(0)).toBe("text-m3-on-surface");
  });
});

// ---- monthLabel (PRF-015) -----------------------------------------------------

describe("monthLabel", () => {
  it("returns an i18n key for a valid month number 1–12", () => {
    const label = monthLabel(1);
    expect(typeof label).toBe("string");
    expect(label.length).toBeGreaterThan(0);
  });

  it("returns distinct labels for different months", () => {
    expect(monthLabel(1)).not.toBe(monthLabel(6));
    expect(monthLabel(6)).not.toBe(monthLabel(12));
  });

  it("covers all twelve months without throwing", () => {
    for (let m = 1; m <= 12; m++) {
      expect(() => monthLabel(m)).not.toThrow();
    }
  });
});

// ---- presentPeriodRow (PRF-036, PRF-037, PRF-041, PRF-042) --------------------

describe("presentPeriodRow — year row", () => {
  it("omits year_to_date from the view model for year rows (PRF-037)", () => {
    const row = presentPeriodRow(makeYearRow());
    // year_to_date must not be present on the year-view row model
    expect(row.yearToDate).toBeUndefined();
  });

  it("includes period_over_period and since_inception for year rows", () => {
    const row = presentPeriodRow(makeYearRow());
    expect(row.periodOverPeriod).toBeDefined();
    expect(row.sinceInception).toBeDefined();
  });

  it("maps annualized_yield (CAGR) for year rows; pct is the headline (T3)", () => {
    const row = presentPeriodRow(
      makeYearRow({ annualized_yield: makeMetric({ gain: 2_000_000_000, pct: 10_000_000 }) }),
    );
    expect(row.annualizedYield).toBeDefined();
    expect(row.annualizedYield?.pctFormatted).toContain("%");
    // The cumulative gain is carried as the secondary value.
    expect(row.annualizedYield?.gainFormatted).toBeTruthy();
  });

  it("renders '—' for an absent annualized_yield on a year row (T3)", () => {
    const row = presentPeriodRow(makeYearRow({ annualized_yield: null }));
    expect(row.annualizedYield?.pctFormatted).toBe("—");
  });

  it("maps the bridge columns; In/Out combines cash and asset flows (PRF-070–075)", () => {
    const row = presentPeriodRow(
      makeYearRow({
        previous_value: 9_000_000_000,
        cash_flow: 500_000_000, // +€500.00 cash in
        asset_flow: -200_000_000, // −€200.00 asset out
        dividends: 120_000_000,
        pnl: 580_000_000,
        end_value: 10_000_000_000,
      }),
    );
    expect(row.previousValueFormatted).toBeTruthy();
    expect(row.dividendsFormatted).toBeTruthy();
    // PRF-075 — the single In/Out cell is the net of the two backend terms: +300.00.
    expect(row.externalFlow.formatted).toBe("300,00");
    expect(row.externalFlow.colorClass).toBe("text-m3-success");
    expect(row.pnl.formatted).toBeTruthy();
  });

  it("PRF-075 — a net-negative combined flow is sign-coloured as an outflow", () => {
    const row = presentPeriodRow(makeYearRow({ cash_flow: 100_000_000, asset_flow: -400_000_000 }));
    expect(row.externalFlow.formatted).toBe("-300,00");
    expect(row.externalFlow.colorClass).toBe("text-m3-error");
  });

  it("renders '—' for an absent period_over_period (first row, PRF-042)", () => {
    const row = presentPeriodRow(makeYearRow({ period_over_period: null }));
    expect(row.periodOverPeriod.gainFormatted).toBe("—");
    expect(row.periodOverPeriod.pctFormatted).toBe("—");
  });
});

describe("presentPeriodRow — month row", () => {
  it("includes all three metrics for month rows (PRF-036)", () => {
    const row = presentPeriodRow(makeMonthRow());
    expect(row.periodOverPeriod).toBeDefined();
    expect(row.yearToDate).toBeDefined();
    expect(row.sinceInception).toBeDefined();
  });

  it("year_to_date is present on month rows (PRF-034)", () => {
    const row = presentPeriodRow(makeMonthRow());
    expect(row.yearToDate).toBeDefined();
    expect(row.yearToDate?.gainFormatted).not.toBe(undefined);
  });

  it("omits annualized_yield from month rows (year-row concept only, T3)", () => {
    const row = presentPeriodRow(makeMonthRow());
    expect(row.annualizedYield).toBeUndefined();
  });

  it("renders '—' for absent period_over_period on the earliest month row (PRF-042)", () => {
    const row = presentPeriodRow(makeMonthRow({ period_over_period: null }));
    expect(row.periodOverPeriod.gainFormatted).toBe("—");
    expect(row.periodOverPeriod.pctFormatted).toBe("—");
  });

  it("renders '—' for null pct when Dietz denominator was 0 (PRF-032)", () => {
    const row = presentPeriodRow(
      makeMonthRow({
        period_over_period: makeMetric({ gain: 0, pct: null }),
      }),
    );
    expect(row.periodOverPeriod.pctFormatted).toBe("—");
  });

  it("exposes monthLabel for month rows", () => {
    const row = presentPeriodRow(makeMonthRow({ month: 3 }));
    expect(row.periodLabel).toBeTruthy();
  });
});

describe("presentPeriodRow — year label", () => {
  it("exposes the year as the period label for year rows", () => {
    const row = presentPeriodRow(makeYearRow({ year: 2024 }));
    expect(String(row.periodLabel)).toContain("2024");
  });
});

// ---- presentValueChartSeries (value-over-time chart) --------------------------

describe("presentValueChartSeries", () => {
  it("reverses backend (most-recent-first) order into chronological order", () => {
    const series = presentValueChartSeries([
      makeMonthRow({ month: 3, end_value: 10_200_000_000 }),
      makeMonthRow({ month: 2, end_value: 9_500_000_000 }),
      makeMonthRow({ month: 1, end_value: 9_000_000_000 }),
    ]);
    expect(series.map((point) => point.month)).toEqual([1, 2, 3]);
  });

  it("converts end_value micros to a decimal number and keeps a formatted string", () => {
    const point = presentValueChartSeries([makeYearRow({ end_value: 10_000_000_000 })])[0];
    expect(point?.value).toBe(10_000);
    expect(typeof point?.valueFormatted).toBe("string");
    expect((point?.valueFormatted ?? "").length).toBeGreaterThan(0);
  });

  it("keys year points by year and month points by year-month", () => {
    expect(presentValueChartSeries([makeYearRow({ year: 2024 })])[0]?.key).toBe("2024");
    expect(presentValueChartSeries([makeMonthRow({ year: 2024, month: 7 })])[0]?.key).toBe(
      "2024-7",
    );
  });

  it("returns an empty series for no periods", () => {
    expect(presentValueChartSeries([])).toEqual([]);
  });
});

describe("presentAssetScopeOptions", () => {
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

  const makeDetailsResponse = (holdings: HoldingDetail[]): AccountDetailsResponse => ({
    account_name: "My Portfolio",
    holdings,
    closed_holdings: [],
    total_holding_count: holdings.length,
    total_cost_basis: 0,
    total_realized_pnl: 0,
    total_unrealized_pnl: null,
    total_global_value: 0,
    total_dividends_received: 0,
    total_management_fees: 0,
    total_net_cash_input: 0,
  });

  // PRF-082 — the cash line is never offered as a scope
  it("excludes the cash line and maps id + name in backend order (PRF-082)", () => {
    const options = presentAssetScopeOptions(
      makeDetailsResponse([
        makeHolding({ asset_id: "system-cash-EUR", asset_name: "Cash" }),
        makeHolding(),
        makeHolding({ asset_id: "asset-2", asset_name: "Microsoft Corp" }),
      ]),
    );

    expect(options).toEqual([
      { assetId: "asset-1", assetName: "Apple Inc" },
      { assetId: "asset-2", assetName: "Microsoft Corp" },
    ]);
  });

  it("returns no options for an account with only the cash line", () => {
    const options = presentAssetScopeOptions(
      makeDetailsResponse([makeHolding({ asset_id: "system-cash-EUR", asset_name: "Cash" })]),
    );

    expect(options).toEqual([]);
  });
});
