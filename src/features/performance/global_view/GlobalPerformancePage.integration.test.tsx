import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  Account,
  AccountDetailsResponse,
  AccountPerformanceResponse,
  Asset,
  HoldingDetail,
  PerformancePeriod,
} from "@/bindings";
import { useAppStore } from "@/lib/store";
import * as gateway from "../gateway";
import { GlobalPerformancePage } from "./GlobalPerformancePage";

// Mock the gateway so no real Tauri calls fire (F27, docs/test_convention.md § Mocking gateway modules)
vi.mock("../gateway");

// Mock the router — the page renders back/add-transaction links
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  Link: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// Identity i18n — t(key) === key so tests assert on stable keys (F24)
// t must be referentially stable across renders (like the real memoized t):
// it sits in the hook's effect dependency lists, and a fresh function per
// render would re-run those effects forever.
vi.mock("react-i18next", () => {
  const t = (key: string) => key;
  return {
    useTranslation: () => ({ t, i18n: { language: "en-US" } }),
  };
});

// ---- Fixtures ---------------------------------------------------------------

const makeMetric = (gain = 1_000_000_000, pct: number | null = 8_000_000) => ({
  gain,
  pct,
});

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
  period_over_period: makeMetric(),
  year_to_date: null,
  since_inception: makeMetric(2_000_000_000, 20_000_000),
  annualized_yield: makeMetric(2_000_000_000, 10_000_000),
  ...overrides,
});

const makeMonthRow = (
  month: number,
  overrides: Partial<PerformancePeriod> = {},
): PerformancePeriod => ({
  year: 2025,
  month,
  end_value: 10_000_000_000,
  ...BRIDGE_DEFAULTS,
  period_over_period: makeMetric(),
  year_to_date: makeMetric(350_000_000, 3_500_000),
  since_inception: makeMetric(2_000_000_000, 20_000_000),
  annualized_yield: null,
  ...overrides,
});

const makeResponse = (
  overrides: Partial<AccountPerformanceResponse> = {},
): AccountPerformanceResponse => ({
  account_name: "",
  currency: "EUR",
  month_view_available: true,
  yearly: [makeYearRow()],
  monthly: [makeMonthRow(5), makeMonthRow(4), makeMonthRow(3)],
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

const makeDetailsResponse = (): AccountDetailsResponse => ({
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
});

// ---- Tests ------------------------------------------------------------------

describe("GlobalPerformancePage", () => {
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

  it("renders a loading skeleton while data is being fetched", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockReturnValue(
      new Promise(() => {}),
    );

    render(<GlobalPerformancePage />);

    expect(screen.getByTestId("global-performance-loading")).toBeInTheDocument();
  });

  // GPF-015 — empty portfolio state with Add Transaction affordance
  it("renders the empty state when the portfolio has no data (GPF-015)", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ yearly: [], monthly: [], month_view_available: false }),
    });

    render(<GlobalPerformancePage />);

    expect(await screen.findByTestId("global-performance-empty")).toBeInTheDocument();
    expect(screen.getByTestId("global-performance-add-transaction")).toBeInTheDocument();
  });

  // F27 — error state with Retry re-fetching
  it("renders the error state and re-fetches on Retry", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    render(<GlobalPerformancePage />);
    await screen.findByTestId("global-performance-error");

    await userEvent.click(screen.getByTestId("global-performance-retry"));

    expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledTimes(2);
  });

  it("renders the back navigation to the accounts overview", async () => {
    render(<GlobalPerformancePage />);

    expect(await screen.findByTestId("global-performance-back")).toBeInTheDocument();
  });

  // GPF-011 — the reporting currency of the figures is visible in the header
  it("shows the response currency in the title (GPF-011)", async () => {
    render(<GlobalPerformancePage />);

    expect(await screen.findByTestId("global-performance-currency")).toHaveTextContent("EUR");
  });

  // GPF-010 — the account selector offers All accounts plus every catalog account
  it("renders the account selector with All accounts and the catalog accounts (GPF-010)", async () => {
    render(<GlobalPerformancePage />);

    const selector = await screen.findByTestId("global-performance-account-selector");
    const options = Array.from(selector.querySelectorAll("option"));
    expect(options.map((option) => option.getAttribute("value"))).toEqual([
      "",
      "account-1",
      "account-2",
    ]);
    expect(options[0]).toHaveTextContent("global_performance.account_selector_all");
    expect(selector).toHaveValue("");
  });

  // All-accounts scope — the asset selector offers the catalog without cash or archived assets
  it("renders the asset selector from the catalog, excluding cash and archived assets", async () => {
    render(<GlobalPerformancePage />);

    const selector = await screen.findByTestId("global-performance-asset-selector");
    const options = Array.from(selector.querySelectorAll("option"));
    expect(options.map((option) => option.getAttribute("value"))).toEqual([
      "",
      "asset-1",
      "asset-2",
    ]);
    expect(options[0]).toHaveTextContent("account_performance.asset_selector_all");
    expect(selector).toHaveValue("");
  });

  // GPF-010 / GPF-011 — selecting an account scopes the fetch and titles the page
  it("re-fetches scoped and shows the account name in the title on selection (GPF-011)", async () => {
    render(<GlobalPerformancePage />);
    const selector = await screen.findByTestId("global-performance-account-selector");

    await userEvent.selectOptions(selector, "account-1");

    expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
      "account-1",
      null,
    );
    expect(await screen.findByTestId("global-performance-scope-label")).toHaveTextContent(
      "Broker One",
    );
  });

  // Account scope — the asset selector switches to the account's non-cash holdings
  it("offers the scoped account's holdings in the asset selector", async () => {
    render(<GlobalPerformancePage />);
    const accountSelector = await screen.findByTestId("global-performance-account-selector");

    await userEvent.selectOptions(accountSelector, "account-1");

    await screen.findByTestId("global-performance-scope-label");
    const assetSelector = await screen.findByTestId("global-performance-asset-selector");
    const options = Array.from(assetSelector.querySelectorAll("option"));
    // All assets default + the account's single non-cash holding.
    expect(options.map((option) => option.getAttribute("value"))).toEqual(["", "asset-1"]);
  });

  // Changing the account scope resets the asset selector to All assets
  it("resets the asset scope to All assets when the account scope changes", async () => {
    render(<GlobalPerformancePage />);

    await userEvent.selectOptions(
      await screen.findByTestId("global-performance-asset-selector"),
      "asset-2",
    );
    expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
      null,
      "asset-2",
    );

    await userEvent.selectOptions(
      await screen.findByTestId("global-performance-account-selector"),
      "account-1",
    );

    expect(gateway.globalPerformanceGateway.getGlobalPerformance).toHaveBeenCalledWith(
      "account-1",
      null,
    );
    expect(await screen.findByTestId("global-performance-asset-selector")).toHaveValue("");
  });

  // The period table and value chart render for the portfolio read
  it("renders the performance table and value chart", async () => {
    render(<GlobalPerformancePage />);

    expect(await screen.findByTestId("global-performance-table")).toBeInTheDocument();
    expect(screen.getByTestId("global-performance-value-chart")).toBeInTheDocument();
    expect(screen.getByTestId("global-performance-row-2025-5")).toBeInTheDocument();
  });

  // GPF-014 — view-mode toggle switches between month and year views
  it("switches views with the toggle and hides the year selector in year view (GPF-014)", async () => {
    render(<GlobalPerformancePage />);

    // Month view is the default when available → year selector + YTD column present.
    expect(await screen.findByTestId("global-performance-year-selector")).toBeInTheDocument();
    expect(screen.getByTestId("global-performance-col-ytd")).toBeInTheDocument();

    await userEvent.click(screen.getByTestId("global-performance-view-toggle-year"));

    expect(screen.queryByTestId("global-performance-year-selector")).not.toBeInTheDocument();
    expect(screen.queryByTestId("global-performance-col-ytd")).not.toBeInTheDocument();
    expect(screen.getByTestId("global-performance-col-annualized")).toBeInTheDocument();
  });

  // The year selector filters the month rows to the selected year
  it("filters month rows by the selected year", async () => {
    vi.mocked(gateway.globalPerformanceGateway.getGlobalPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({
        monthly: [makeMonthRow(2), makeMonthRow(12, { year: 2024 })],
      }),
    });

    render(<GlobalPerformancePage />);
    const yearSelector = await screen.findByTestId("global-performance-year-selector");

    // Most recent year (2025) is preselected → only its month row shows.
    expect(screen.getByTestId("global-performance-row-2025-2")).toBeInTheDocument();
    expect(screen.queryByTestId("global-performance-row-2024-12")).not.toBeInTheDocument();

    await userEvent.selectOptions(yearSelector, "2024");

    expect(screen.getByTestId("global-performance-row-2024-12")).toBeInTheDocument();
    expect(screen.queryByTestId("global-performance-row-2025-2")).not.toBeInTheDocument();
  });
});
