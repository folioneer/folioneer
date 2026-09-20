import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  AccountDetailsResponse,
  AccountPerformanceResponse,
  Event,
  HoldingDetail,
  PerformancePeriod,
} from "@/bindings";
import * as gateway from "../gateway";
import { AccountPerformancePage } from "./AccountPerformancePage";

// Mock the gateway so no real Tauri calls fire (F27, docs/test_convention.md § Mocking gateway modules)
vi.mock("../gateway");

// Mock the router — AccountPerformancePage reads accountId from route params
vi.mock("@tanstack/react-router", () => ({
  useParams: () => ({ accountId: "account-1" }),
  useNavigate: () => vi.fn(),
  Link: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// Identity i18n — t(key) === key so tests assert on stable keys (F24).
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

// PRF-070–074 — bridge term defaults shared by both row factories.
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
  account_name: "My Portfolio",
  currency: "EUR",
  month_view_available: true,
  yearly: [makeYearRow()],
  monthly: [makeMonthRow(5), makeMonthRow(4), makeMonthRow(3), makeMonthRow(2), makeMonthRow(1)],
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
    makeHolding({ asset_id: "system-cash-EUR", asset_name: "Cash" }),
    makeHolding(),
    makeHolding({ asset_id: "asset-2", asset_name: "Microsoft Corp", asset_reference: "MSFT" }),
  ],
  closed_holdings: [],
  total_holding_count: 3,
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

describe("AccountPerformancePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    vi.mocked(gateway.accountPerformanceGateway.getAccountHoldings).mockResolvedValue({
      status: "ok",
      data: makeDetailsResponse(),
    });
  });

  // PRF-050 — loading skeleton displayed while fetch is in-flight
  it("renders a loading skeleton while data is being fetched (PRF-050)", async () => {
    // Never resolves during this test — keeps the component in loading state
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockReturnValue(
      new Promise(() => {}),
    );

    render(<AccountPerformancePage />);

    // loading indicator must be in the DOM before the promise resolves
    expect(screen.getByTestId("account-performance-loading")).toBeInTheDocument();
  });

  // PRF-051 — empty state + Add Transaction affordance when no transactions
  it("renders empty state with Add Transaction affordance when account has no data (PRF-051)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ yearly: [], monthly: [] }),
    });

    render(<AccountPerformancePage />);

    expect(await screen.findByTestId("account-performance-empty")).toBeInTheDocument();
    expect(screen.getByTestId("account-performance-add-transaction")).toBeInTheDocument();
  });

  // PRF-052 — error state rendered with Retry button on gateway failure
  it("renders error state with Retry button on gateway error (PRF-052)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    render(<AccountPerformancePage />);

    expect(await screen.findByTestId("account-performance-error")).toBeInTheDocument();
    expect(screen.getByTestId("account-performance-retry")).toBeInTheDocument();
  });

  // PRF-052 — Retry button triggers a re-fetch
  it("re-fetches when Retry is clicked (PRF-052)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "error",
      error: { code: "DatabaseError" },
    });

    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-retry");

    await userEvent.click(screen.getByTestId("account-performance-retry"));

    expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledTimes(2);
  });

  // PRF-053 — back navigation link to Account Details is present
  it("renders a back navigation link to Account Details (PRF-053)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });

    render(<AccountPerformancePage />);

    expect(await screen.findByTestId("account-performance-back")).toBeInTheDocument();
  });

  // PRF-011 — view-mode toggle present when month_view_available is true
  it("renders the month/year view-mode toggle when month view is available (PRF-011, PRF-013)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);

    expect(await screen.findByTestId("account-performance-view-toggle")).toBeInTheDocument();
  });

  // PRF-013 — toggle hidden or disabled when month_view_available is false
  it("hides or disables the view-mode toggle when month view is not available (PRF-013)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: false, monthly: [] }),
    });

    render(<AccountPerformancePage />);

    await screen.findByTestId("account-performance-table");

    // Toggle must be absent from the DOM OR be disabled
    const toggle = screen.queryByTestId("account-performance-view-toggle");
    if (toggle) {
      expect(toggle).toBeDisabled();
    } else {
      expect(toggle).toBeNull();
    }
  });

  // PRF-014 — page opens in month view when month_view_available is true
  it("opens in month view by default when month view is available (PRF-014)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);

    await screen.findByTestId("account-performance-table");

    // Year selector is only present in month view (PRF-015)
    expect(screen.getByTestId("account-performance-year-selector")).toBeInTheDocument();
  });

  // PRF-014 — page opens in year view when month view is not available
  it("opens in year view by default when month view is not available (PRF-014)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: false, monthly: [] }),
    });

    render(<AccountPerformancePage />);

    await screen.findByTestId("account-performance-table");

    // Year selector must not be present in year view
    expect(screen.queryByTestId("account-performance-year-selector")).not.toBeInTheDocument();
  });

  // PRF-014 — a remembered "year" preference overrides the month-view default
  it("restores the remembered view mode over the default (PRF-014)", async () => {
    localStorage.setItem("perf_view_mode_account-1", "year");
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-table");

    // Year view restored despite month view being available → no year selector.
    expect(screen.queryByTestId("account-performance-year-selector")).not.toBeInTheDocument();
  });

  // PRF-014 — toggling the view mode persists the choice per account
  it("persists the view mode when the user toggles it (PRF-014)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-year-selector"); // month view active

    await userEvent.click(screen.getByTestId("account-performance-view-toggle-year"));

    expect(localStorage.getItem("perf_view_mode_account-1")).toBe("year");
  });

  // PRF-014 — a remembered "month" is clamped to year view when month view is gone
  it("falls back to year view when the remembered month view is unavailable (PRF-014)", async () => {
    localStorage.setItem("perf_view_mode_account-1", "month");
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: false, monthly: [] }),
    });

    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-table");

    expect(screen.queryByTestId("account-performance-year-selector")).not.toBeInTheDocument();
  });

  // PRF-015 — year selector is present in month view
  it("renders a year selector in month view (PRF-015)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);

    expect(await screen.findByTestId("account-performance-year-selector")).toBeInTheDocument();
  });

  // PRF-037 — YTD column absent in year view
  it("does not render the YTD column in year view (PRF-037)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: false, monthly: [] }),
    });

    render(<AccountPerformancePage />);

    await screen.findByTestId("account-performance-table");

    // The YTD column header must be absent in year view
    expect(screen.queryByTestId("account-performance-col-ytd")).not.toBeInTheDocument();
  });

  // PRF-037 — YTD column present in month view
  it("renders the YTD column in month view (PRF-037)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);

    // In month view the YTD column header must be present
    expect(await screen.findByTestId("account-performance-col-ytd")).toBeInTheDocument();
  });

  // PRF-011 — switching from month view to year view removes the YTD column
  it("removes the YTD column when the user switches to year view (PRF-011, PRF-037)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);

    // Wait for month view to be active (YTD column visible)
    await screen.findByTestId("account-performance-col-ytd");

    // Switch to year view
    await userEvent.click(screen.getByTestId("account-performance-view-toggle-year"));

    expect(screen.queryByTestId("account-performance-col-ytd")).not.toBeInTheDocument();
  });

  // T3 — annualized-yield column present in year view, absent in month view
  it("renders the annualized-yield column only in year view (T3)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);

    // Month view is the default when available → no annualized column.
    await screen.findByTestId("account-performance-year-selector");
    expect(screen.queryByTestId("account-performance-col-annualized")).not.toBeInTheDocument();

    // Switching to year view reveals the annualized column and its per-row cell.
    await userEvent.click(screen.getByTestId("account-performance-view-toggle-year"));
    expect(screen.getByTestId("account-performance-col-annualized")).toBeInTheDocument();
    expect(screen.getByTestId("account-performance-annualized-2025")).toHaveTextContent("%");
  });

  // PRF-041 — rows rendered as-is (most-recent first from backend)
  it("renders period rows in the order returned by the backend (PRF-041)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({
        month_view_available: false,
        monthly: [],
        yearly: [makeYearRow({ year: 2025 }), makeYearRow({ year: 2024 })],
      }),
    });

    render(<AccountPerformancePage />);

    await screen.findByTestId("account-performance-table");

    const rows = screen.getAllByTestId(/^account-performance-row-/);
    // Most recent year (2025) must appear before 2024
    expect(rows[0]).toHaveAttribute("data-testid", "account-performance-row-2025");
    expect(rows[1]).toHaveAttribute("data-testid", "account-performance-row-2024");
  });

  // Value and percentage render in dedicated columns (not combined in one cell).
  it("renders performance value and percentage in separate columns", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({
        month_view_available: false,
        monthly: [],
        yearly: [makeYearRow({ year: 2025 })],
      }),
    });

    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-table");

    const value = screen.getByTestId("account-performance-pop-value-2025");
    const pct = screen.getByTestId("account-performance-pop-pct-2025");
    // The value cell carries the money amount with no percent sign; the pct cell carries the %.
    expect(value).not.toHaveTextContent("%");
    expect(pct).toHaveTextContent("%");
    // Since-Inception splits the same way.
    expect(screen.getByTestId("account-performance-since-value-2025")).not.toHaveTextContent("%");
    expect(screen.getByTestId("account-performance-since-pct-2025")).toHaveTextContent("%");
  });

  // PRF-060 — re-fetch subscribes to TransactionUpdated, AssetPriceUpdated, AccountUpdated
  it("calls getAccountPerformance on mount (PRF-060 setup)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });

    render(<AccountPerformancePage />);

    await screen.findByTestId("account-performance-table");

    expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledWith(
      "account-1",
      null,
    );
  });

  // PRF-080 / PRF-082 — asset selector offers All assets + one option per non-cash holding
  it("renders the asset selector with All assets and the non-cash holdings (PRF-080, PRF-082)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });

    render(<AccountPerformancePage />);

    const selector = await screen.findByTestId("account-performance-asset-selector");
    const options = Array.from(selector.querySelectorAll("option"));
    // All assets default + Apple + Microsoft; the cash line is never offered (PRF-082).
    expect(options.map((option) => option.getAttribute("value"))).toEqual([
      "",
      "asset-1",
      "asset-2",
    ]);
    expect(options[0]).toHaveTextContent("account_performance.asset_selector_all");
    expect(selector).toHaveValue("");
  });

  // PRF-080 — selecting an asset dispatches a scoped fetch and titles the page with the asset
  it("re-fetches scoped and shows the asset name in the title on selection (PRF-080)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });

    render(<AccountPerformancePage />);
    const selector = await screen.findByTestId("account-performance-asset-selector");

    await userEvent.selectOptions(selector, "asset-1");

    expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledWith(
      "account-1",
      "asset-1",
    );
    expect(await screen.findByTestId("account-performance-scoped-asset-name")).toHaveTextContent(
      "Apple Inc",
    );

    // Returning to All assets removes the asset name from the title. The scoped
    // fetch remounted the content area, so the selector is re-queried.
    await userEvent.selectOptions(
      await screen.findByTestId("account-performance-asset-selector"),
      "",
    );
    expect(gateway.accountPerformanceGateway.getAccountPerformance).toHaveBeenCalledWith(
      "account-1",
      null,
    );
    expect(screen.queryByTestId("account-performance-scoped-asset-name")).not.toBeInTheDocument();
  });

  // PRF-011 / PRF-015 — the view-mode toggle and year selector keep working while scoped
  it("keeps the view-mode toggle and year selector working in scoped mode (PRF-011, PRF-015)", async () => {
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse({ month_view_available: true }),
    });

    render(<AccountPerformancePage />);
    const selector = await screen.findByTestId("account-performance-asset-selector");

    await userEvent.selectOptions(selector, "asset-2");
    await screen.findByTestId("account-performance-scoped-asset-name");

    // Month view is active → year selector present alongside the asset selector.
    expect(screen.getByTestId("account-performance-year-selector")).toBeInTheDocument();

    // The toggle still switches to year view; the scope stays applied.
    await userEvent.click(screen.getByTestId("account-performance-view-toggle-year"));
    expect(screen.queryByTestId("account-performance-year-selector")).not.toBeInTheDocument();
    expect(screen.getByTestId("account-performance-scoped-asset-name")).toHaveTextContent(
      "Microsoft Corp",
    );
  });

  // PRF-060 — an emitted TransactionUpdated event triggers a re-fetch
  it("re-fetches when a TransactionUpdated event is received (PRF-060)", async () => {
    let capturedCallback: ((type: Event["type"]) => void) | null = null;
    vi.mocked(gateway.accountPerformanceGateway.subscribeToEvents).mockImplementation(
      (cb: (type: Event["type"]) => void) => {
        capturedCallback = cb;
        return Promise.resolve(() => {});
      },
    );
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });

    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-table");
    const callsBefore = vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock
      .calls.length;

    await act(async () => {
      capturedCallback?.("TransactionUpdated");
    });

    expect(
      vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock.calls.length,
    ).toBeGreaterThan(callsBefore);
  });

  // MKT-181 — per-asset price events are coalesced while a bulk fetch is active;
  // the view reloads once on AssetPriceFetchCompleted.
  it("skips AssetPriceUpdated during an active fetch, reloads on completion (MKT-181)", async () => {
    const { useAppStore } = await import("@/lib/store");
    let capturedCallback: ((type: Event["type"]) => void) | null = null;
    vi.mocked(gateway.accountPerformanceGateway.subscribeToEvents).mockImplementation(
      (cb: (type: Event["type"]) => void) => {
        capturedCallback = cb;
        return Promise.resolve(() => {});
      },
    );
    vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mockResolvedValue({
      status: "ok",
      data: makeResponse(),
    });

    useAppStore.setState({ priceFetch: { active: true, done: 1, total: 3 } });
    render(<AccountPerformancePage />);
    await screen.findByTestId("account-performance-table");
    const callsBefore = vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock
      .calls.length;

    await act(async () => {
      capturedCallback?.("AssetPriceUpdated");
    });
    expect(
      vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock.calls.length,
    ).toBe(callsBefore);

    useAppStore.setState({ priceFetch: { active: false, done: 0, total: 0 } });
    await act(async () => {
      capturedCallback?.("AssetPriceFetchCompleted");
    });
    expect(
      vi.mocked(gateway.accountPerformanceGateway.getAccountPerformance).mock.calls.length,
    ).toBe(callsBefore + 1);
  });
});
