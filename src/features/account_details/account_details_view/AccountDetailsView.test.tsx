import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AccountDetailsView } from "./AccountDetailsView";

// ── Controlled orchestration hook ───────────────────────────────────────────
const { mockUseAccountDetailsView, mockUseRefreshAccountPrices, mockUsePinnedScrollbar } =
  vi.hoisted(() => ({
    mockUseAccountDetailsView: vi.fn(),
    mockUseRefreshAccountPrices: vi.fn(),
    mockUsePinnedScrollbar: vi.fn(),
  }));

vi.mock("./useAccountDetailsView", () => ({
  useAccountDetailsView: () => mockUseAccountDetailsView(),
}));

vi.mock("./usePinnedScrollbar", () => ({
  usePinnedScrollbar: () => mockUsePinnedScrollbar(),
}));

vi.mock("../refresh_prices/useRefreshAccountPrices", () => ({
  useRefreshAccountPrices: () => mockUseRefreshAccountPrices(),
}));

vi.mock("@tanstack/react-router", () => ({
  useParams: () => ({ accountId: "acc-1" }),
  useNavigate: () => vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

// Stub the child modals / rows so the view renders in isolation (no gateway,
// no Tauri, no nested hooks) — this test exercises AccountDetailsView's own JSX.
vi.mock("../buy_transaction/BuyTransactionModal", () => ({ BuyTransactionModal: () => null }));
vi.mock("../sell_transaction/SellTransactionModal", () => ({ SellTransactionModal: () => null }));
vi.mock("../deposit_transaction/DepositTransactionModal", () => ({
  DepositTransactionModal: () => <div data-testid="deposit-modal-mounted" />,
}));
vi.mock("../withdrawal_transaction/WithdrawalTransactionModal", () => ({
  WithdrawalTransactionModal: () => <div data-testid="withdrawal-modal-mounted" />,
}));
vi.mock("../open_balance/OpenBalanceModal", () => ({ OpenBalanceModal: () => null }));
vi.mock("../price_history/PriceHistoryModal", () => ({ PriceHistoryModal: () => null }));
vi.mock("../dividend_transaction/DividendTransactionModal", () => ({
  DividendTransactionModal: () => <div data-testid="dividend-modal-mounted" />,
}));
vi.mock("../management_fee_transaction/ManagementFeeModal", () => ({
  ManagementFeeModal: () => <div data-testid="management-fee-modal-mounted" />,
}));
vi.mock("../interest_transaction/InterestModal", () => ({
  InterestModal: () => <div data-testid="interest-modal-mounted" />,
}));
vi.mock("../fee_schedule/FeeScheduleModal", () => ({
  FeeScheduleModal: () => <div data-testid="fee-schedule-modal-mounted" />,
}));
vi.mock("./HoldingRow", () => ({ HoldingRow: () => <tr data-testid="holding-row" /> }));
vi.mock("./ClosedHoldingRow", () => ({ ClosedHoldingRow: () => <tr /> }));

const handlers = {
  handleDepositOpen: vi.fn(),
  handleWithdrawalOpen: vi.fn(),
  handleOpenBalanceOpen: vi.fn(),
  handleDividendOpen: vi.fn(),
  handleFreeSharesOpen: vi.fn(),
  handleManagementFeeOpen: vi.fn(),
  handleInterestOpen: vi.fn(),
  handleAddTransaction: vi.fn(),
};

/** What `usePinnedScrollbar` reports; unmeasured by default, as under jsdom (#022). */
const pinnedScrollbar = (overrides: Record<string, unknown> = {}) => ({
  scrollAreaRef: { current: null },
  scrollbarRef: { current: null },
  scrollbar: {
    viewportWidth: 0,
    closedStaysPut: false,
    trackOffset: 0,
    scrollRange: 0,
    spacerWidth: 0,
    visible: false,
    ...overrides,
  },
});

beforeEach(() => {
  mockUsePinnedScrollbar.mockReturnValue(pinnedScrollbar());
});

const makeView = (overrides: Record<string, unknown> = {}) => ({
  isLoading: false,
  error: null,
  retry: vi.fn(),
  summary: {
    accountName: "Main",
    totalGlobalValue: "1.100,00",
    totalManagementFees: "0,00",
    totalNetCashInput: "500,00",
    isEmpty: false,
    isAllClosed: false,
    hasClosedHoldings: false,
  },
  holdings: [],
  closedHoldings: [],
  accountCurrency: "EUR",
  managementFeesEnabled: true,
  hasNonCashActiveHoldings: false,
  hasClosedHoldings: false,
  activeNonCashHoldings: [],
  interestEligibleHoldings: [],
  buyTarget: null,
  sellTarget: null,
  historyTarget: null,
  openBalanceOpen: false,
  depositOpen: false,
  withdrawalOpen: false,
  dividendOpen: false,
  managementFeeOpen: false,
  interestOpen: false,
  feeScheduleTarget: null,
  asOfDate: "",
  asOfDisplayDate: "2024-06-01",
  isAsOf: false,
  setAsOfDate: vi.fn(),
  perfPeriod: "since_start",
  setPerfPeriod: vi.fn(),
  ...handlers,
  handleBuyOpen: vi.fn(),
  handleBuyClose: vi.fn(),
  handleBuySuccess: vi.fn(),
  handleSellOpen: vi.fn(),
  handleSellClose: vi.fn(),
  handleSellSuccess: vi.fn(),
  handlePriceHistory: vi.fn(),
  handleHistoryClose: vi.fn(),
  handleOpenBalanceClose: vi.fn(),
  handleOpenBalanceSuccess: vi.fn(),
  handleDepositClose: vi.fn(),
  handleDepositSuccess: vi.fn(),
  handleWithdrawalClose: vi.fn(),
  handleWithdrawalSuccess: vi.fn(),
  handleDividendClose: vi.fn(),
  handleDividendSuccess: vi.fn(),
  handleManagementFeeClose: vi.fn(),
  handleManagementFeeSuccess: vi.fn(),
  handleInterestClose: vi.fn(),
  handleInterestSuccess: vi.fn(),
  handleFeeScheduleOpen: vi.fn(),
  handleFeeScheduleClose: vi.fn(),
  handleFeeScheduleSuccess: vi.fn(),
  handleTogglePriceRefreshLock: vi.fn(),
  handleBackfillPriceHistory: vi.fn(),
  backfillingAssetIds: [],
  ...overrides,
});

describe("AccountDetailsView — header actions (DIV-012)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseRefreshAccountPrices.mockReturnValue({ isPending: false, refresh: vi.fn() });
    mockUseAccountDetailsView.mockReturnValue(makeView());
  });

  it("renders the record actions as direct buttons — no dropdown trigger", () => {
    render(<AccountDetailsView />);
    // The former consolidated dropdown is gone; actions are direct icon buttons.
    expect(document.querySelector("#account-details-add-menu")).toBeNull();
    expect(document.querySelector("#add-menu-open-balance")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-dividend")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-free-shares")).toBeInTheDocument();
  });

  it("shows Open balance / Dividend / Free shares, with NO cash actions (DIV-012/CSH-019)", () => {
    render(<AccountDetailsView />);
    // CSH-019 — cash Deposit/Withdraw live on the cash row, not the header.
    expect(document.querySelector("#add-menu-deposit")).toBeNull();
    expect(document.querySelector("#add-menu-withdraw")).toBeNull();
    expect(document.querySelector("#add-menu-open-balance")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-dividend")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-free-shares")).toBeInTheDocument();
  });

  it("invokes the dividend handler when the Dividend button is clicked (DIV-010)", () => {
    render(<AccountDetailsView />);
    fireEvent.click(document.querySelector("#add-menu-dividend")!);
    expect(handlers.handleDividendOpen).toHaveBeenCalledTimes(1);
  });

  it("routes the Open balance button to its handler", () => {
    render(<AccountDetailsView />);
    fireEvent.click(document.querySelector("#add-menu-open-balance")!);
    expect(handlers.handleOpenBalanceOpen).toHaveBeenCalledTimes(1);
  });

  it("routes the Free shares button to its handler (FSD-010)", () => {
    render(<AccountDetailsView />);
    fireEvent.click(document.querySelector("#add-menu-free-shares")!);
    expect(handlers.handleFreeSharesOpen).toHaveBeenCalledTimes(1);
  });

  it("mounts the dividend modal only when dividendOpen is true (DIV-010/020)", () => {
    const { rerender } = render(<AccountDetailsView />);
    expect(screen.queryByTestId("dividend-modal-mounted")).toBeNull();

    mockUseAccountDetailsView.mockReturnValue(makeView({ dividendOpen: true }));
    rerender(<AccountDetailsView />);
    expect(screen.getByTestId("dividend-modal-mounted")).toBeInTheDocument();
  });

  it("mounts the deposit modal only while depositOpen is true (CSH-022)", () => {
    const { rerender } = render(<AccountDetailsView />);
    expect(screen.queryByTestId("deposit-modal-mounted")).toBeNull();

    mockUseAccountDetailsView.mockReturnValue(makeView({ depositOpen: true }));
    rerender(<AccountDetailsView />);
    expect(screen.getByTestId("deposit-modal-mounted")).toBeInTheDocument();
  });

  it("mounts the withdrawal modal only while withdrawalOpen is true (CSH-032)", () => {
    const { rerender } = render(<AccountDetailsView />);
    expect(screen.queryByTestId("withdrawal-modal-mounted")).toBeNull();

    mockUseAccountDetailsView.mockReturnValue(makeView({ withdrawalOpen: true }));
    rerender(<AccountDetailsView />);
    expect(screen.getByTestId("withdrawal-modal-mounted")).toBeInTheDocument();
  });

  it("renders the Dividends and Total Return column headers when holdings exist (DIV-072)", () => {
    mockUseAccountDetailsView.mockReturnValue(
      makeView({ hasNonCashActiveHoldings: true, holdings: [{ assetId: "a1" }] }),
    );
    render(<AccountDetailsView />);
    expect(screen.getByText("account_details.column_dividends_received")).toBeInTheDocument();
    expect(screen.getByText("account_details.column_total_return_pct")).toBeInTheDocument();
    expect(screen.getByTestId("holding-row")).toBeInTheDocument();
  });
});

describe("AccountDetailsView — add-transaction FAB (ACD-035/036)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseRefreshAccountPrices.mockReturnValue({ isPending: false, refresh: vi.fn() });
  });

  it("renders a single add-transaction FAB and triggers handleAddTransaction on click", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView());
    render(<AccountDetailsView />);

    const fab = screen.getByRole("button", { name: "account_details.add_transaction" });
    fireEvent.click(fab);
    expect(handlers.handleAddTransaction).toHaveBeenCalledTimes(1);
  });

  it("shows the FAB in the empty state with no inline CTA button", () => {
    // No non-cash active holdings → the asset-positions empty message renders
    // (the always-present cash row is excluded from the count, CSH-098).
    mockUseAccountDetailsView.mockReturnValue(
      makeView({
        summary: { ...makeView().summary, isEmpty: true },
        hasNonCashActiveHoldings: false,
      }),
    );
    render(<AccountDetailsView />);

    expect(screen.getByText("account_details.empty_no_positions")).toBeInTheDocument();
    // The only add-transaction affordance is the FAB (exactly one).
    expect(screen.getAllByRole("button", { name: "account_details.add_transaction" })).toHaveLength(
      1,
    );
  });

  it("shows 'All positions closed' when closed holdings exist but no active non-cash (ACD-034)", () => {
    mockUseAccountDetailsView.mockReturnValue(
      makeView({
        summary: { ...makeView().summary, isAllClosed: true },
        hasNonCashActiveHoldings: false,
        hasClosedHoldings: true,
      }),
    );
    render(<AccountDetailsView />);
    expect(screen.getByText("account_details.empty_all_closed")).toBeInTheDocument();
    expect(screen.queryByText("account_details.empty_no_positions")).toBeNull();
  });
});

// #005 — Actions is the first column of both holdings tables, before Asset.
describe("AccountDetailsView — column order (#005)", () => {
  it("heads the active and closed tables with Actions, then Asset", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ hasClosedHoldings: true }));
    render(<AccountDetailsView />);
    const toggle = document.querySelector("#account-closed-positions-toggle");
    expect(toggle).toBeInTheDocument();
    if (toggle?.getAttribute("aria-expanded") === "false") fireEvent.click(toggle);

    const tables = document.querySelectorAll("table");
    expect(tables).toHaveLength(2);
    for (const table of tables) {
      const headers = [...table.querySelectorAll("thead th")].map((th) => th.textContent);
      expect(headers.slice(0, 2)).toEqual([
        "transaction.column_actions",
        "account_details.column_asset",
      ]);
    }
  });
});

// #021 — the closed table has few columns, so its Asset column must not take a share of
// the spare width; jsdom has no layout, so this asserts the shrink-to-fit classes and the
// committed screenshots show the width.
describe("AccountDetailsView — closed table Asset column (#021)", () => {
  it("sizes the closed table's Asset header to its content, leaving the active table's alone", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ hasClosedHoldings: true }));
    render(<AccountDetailsView />);
    const toggle = document.querySelector("#account-closed-positions-toggle");
    expect(toggle).toBeInTheDocument();
    if (toggle?.getAttribute("aria-expanded") === "false") fireEvent.click(toggle);

    const tables = document.querySelectorAll("table");
    expect(tables).toHaveLength(2);
    const [activeTable, closedTable] = tables;
    const activeAssetHeader = activeTable?.querySelector("thead th:nth-child(2)");
    const closedAssetHeader = closedTable?.querySelector("thead th:nth-child(2)");
    expect(closedAssetHeader).toHaveTextContent("account_details.column_asset");
    expect(closedAssetHeader).toHaveClass("w-px", "whitespace-nowrap");
    expect(activeAssetHeader).not.toHaveClass("w-px");
  });
});

// #001 — jsdom has no layout: these assert the pinning classes; the scrolling itself
// is checked on the real app by e2e/account_details/holdings_scrollbar.test.ts.
// #001 — Actions and Asset stay pinned while the tables scroll sideways, and both
// tables scroll in the view's one content area, so the header sticks and a single
// horizontal scrollbar stays in view.
describe("AccountDetailsView — pinned columns (#001)", () => {
  const renderWithClosedOpen = () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ hasClosedHoldings: true }));
    render(<AccountDetailsView />);
    const toggle = document.querySelector("#account-closed-positions-toggle");
    expect(toggle).toBeInTheDocument();
    if (toggle?.getAttribute("aria-expanded") === "false") fireEvent.click(toggle);
    const tables = document.querySelectorAll("table");
    expect(tables).toHaveLength(2);
    return { toggle, tables };
  };

  it("pins the Actions and Asset headers of both tables, and nothing after them", () => {
    const { tables } = renderWithClosedOpen();
    for (const table of tables) {
      const headers = table.querySelectorAll("thead th");
      expect(headers[0]).toHaveClass("sticky", "left-0");
      expect(headers[1]).toHaveClass("sticky", "left-[224px]");
      expect(headers[2]).not.toHaveClass("left-0");
      // Above the pinned body cells (z-10), which scroll up under the header.
      expect(table.querySelector("thead")).toHaveClass("z-20");
    }
  });

  it("scrolls both tables in the one content area rather than a box of their own", () => {
    const { tables } = renderWithClosedOpen();
    expect(tables[0]?.parentElement).not.toHaveClass("m3-table-container");
    const scrollers = [...tables].map((table) => table.closest(".overflow-auto"));
    expect(scrollers[0]).not.toBeNull();
    expect(scrollers[0]).toBe(scrollers[1]);
    // The active table's wrapper grows to its table, so row backgrounds span the whole
    // scrolled width; the column grows with it, which gives the closed section room to
    // stay put (#022).
    expect(tables[0]?.parentElement).toHaveClass("w-max", "min-w-full");
    expect(tables[0]?.parentElement?.parentElement).toHaveClass("w-max", "min-w-full");
  });

  // #022 — jsdom has no layout: these assert what the view does with the geometry its
  // hook reports; the scrolling itself is proven by the E2E scenario.
  it("#022 holds the closed section at the window's width and keeps it put when it fits", () => {
    mockUsePinnedScrollbar.mockReturnValue(
      pinnedScrollbar({ viewportWidth: 1100, closedStaysPut: true }),
    );
    renderWithClosedOpen();
    const section = document.querySelector("#closed-holdings-section");
    expect(section).toHaveClass("sticky", "left-0", "min-w-max");
    expect(section).toHaveStyle({ width: "1100px" });
  });

  it("#022 lets a closed section too wide for the window scroll with the rest", () => {
    mockUsePinnedScrollbar.mockReturnValue(
      pinnedScrollbar({ viewportWidth: 700, closedStaysPut: false }),
    );
    renderWithClosedOpen();
    const section = document.querySelector("#closed-holdings-section");
    expect(section).toHaveStyle({ width: "700px" });
    expect(section).not.toHaveClass("sticky");
  });

  it("#022 starts the scrollbar's track where the pinned block ends", () => {
    mockUsePinnedScrollbar.mockReturnValue(
      pinnedScrollbar({ viewportWidth: 1100, trackOffset: 480, spacerWidth: 1420, visible: true }),
    );
    renderWithClosedOpen();
    const scrollbar = document.querySelector("#holdings-scrollbar");
    expect(scrollbar).not.toHaveAttribute("hidden");
    expect(scrollbar).toHaveStyle({ marginLeft: "480px" });
    expect(scrollbar?.firstElementChild).toHaveStyle({ width: "1420px" });
    expect(document.querySelector("#holdings-scroll-area")).toHaveClass("pinned-scroll-area");
  });

  it("#022 hides the scrollbar, from assistive tech always and from sight when nothing scrolls", () => {
    renderWithClosedOpen();
    const scrollbar = document.querySelector("#holdings-scrollbar");
    expect(scrollbar).toHaveAttribute("aria-hidden", "true");
    expect(scrollbar).toHaveAttribute("hidden");
  });

  it("keeps the closed-positions label in view while the tables scroll sideways", () => {
    const { toggle } = renderWithClosedOpen();
    expect(toggle?.textContent).toContain("account_details.closed_positions_header");
    expect(toggle?.firstElementChild).toHaveClass("sticky", "left-6");
  });
});

describe("AccountDetailsView — read-only as-of view", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseRefreshAccountPrices.mockReturnValue({ isPending: false, refresh: vi.fn() });
  });

  it("renders the as-of date selector in the header", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView());
    render(<AccountDetailsView />);
    expect(document.querySelector("#account-details-as-of-date")).toBeInTheDocument();
  });

  it("shows the as-of banner + reset and hides the mutating controls when isAsOf", () => {
    const setAsOfDate = vi.fn();
    mockUseAccountDetailsView.mockReturnValue(
      makeView({ isAsOf: true, asOfDate: "2024-06-01", setAsOfDate }),
    );
    render(<AccountDetailsView />);

    // Banner + reset present.
    expect(document.querySelector("#account-details-as-of-banner")).toBeInTheDocument();
    const reset = document.querySelector("#account-details-as-of-reset")!;
    expect(reset).toBeInTheDocument();
    fireEvent.click(reset);
    expect(setAsOfDate).toHaveBeenCalledWith("");

    // Mutating controls hidden.
    expect(document.querySelector("#account-details-refresh-prices")).toBeNull();
    expect(document.querySelector("#add-menu-open-balance")).toBeNull();
    expect(document.querySelector("#add-menu-dividend")).toBeNull();
    expect(document.querySelector("#add-menu-free-shares")).toBeNull();
    expect(document.querySelector("#account-details-add-transaction-fab")).toBeNull();
  });

  it("keeps the mutating controls in the live view (isAsOf false)", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ isAsOf: false }));
    render(<AccountDetailsView />);
    expect(document.querySelector("#account-details-as-of-banner")).toBeNull();
    expect(document.querySelector("#account-details-refresh-prices")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-open-balance")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-dividend")).toBeInTheDocument();
    expect(document.querySelector("#add-menu-free-shares")).toBeInTheDocument();
    expect(document.querySelector("#account-details-add-transaction-fab")).toBeInTheDocument();
  });
});

describe("AccountDetailsView — performance period selector (ACD-054)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseRefreshAccountPrices.mockReturnValue({ isPending: false, refresh: vi.fn() });
  });

  it("renders the period selector in the live view with the six options", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView());
    render(<AccountDetailsView />);
    const select = document.querySelector<HTMLSelectElement>("#account-details-perf-period");
    expect(select).toBeInTheDocument();
    expect(Array.from(select?.options ?? []).map((option) => option.value)).toEqual([
      "since_start",
      "ytd",
      "one_year",
      "two_years",
      "five_years",
      "ten_years",
    ]);
  });

  it("hides the period selector in the read-only as-of view", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ isAsOf: true, asOfDate: "2024-06-01" }));
    render(<AccountDetailsView />);
    expect(document.querySelector("#account-details-perf-period")).toBeNull();
  });

  it("routes a selection change to setPerfPeriod", () => {
    const setPerfPeriod = vi.fn();
    mockUseAccountDetailsView.mockReturnValue(makeView({ setPerfPeriod }));
    render(<AccountDetailsView />);
    fireEvent.change(document.querySelector("#account-details-perf-period")!, {
      target: { value: "ytd" },
    });
    expect(setPerfPeriod).toHaveBeenCalledWith("ytd");
  });

  it("keeps the plain Performance column header for since_start", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView());
    render(<AccountDetailsView />);
    expect(screen.getByText("account_details.column_performance_pct")).toBeInTheDocument();
  });

  it("switches the Performance column header to the selected period", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ perfPeriod: "ytd" }));
    render(<AccountDetailsView />);
    expect(screen.getByText("account_details.column_performance_pct_ytd")).toBeInTheDocument();
    expect(screen.queryByText("account_details.column_performance_pct")).toBeNull();
  });
});

describe("AccountDetailsView — management fees (FEE-010/011/053)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseRefreshAccountPrices.mockReturnValue({ isPending: false, refresh: vi.fn() });
    mockUseAccountDetailsView.mockReturnValue(makeView());
  });

  it("shows the total management fees figure (FEE-053)", () => {
    render(<AccountDetailsView />);
    const total = document.querySelector("#account-details-total-management-fees");
    expect(total).toHaveTextContent("0,00");
  });

  // ACD-053 — net cash input since inception in the header
  it("shows the net cash input figure (ACD-053)", () => {
    render(<AccountDetailsView />);
    const total = document.querySelector("#account-details-total-net-cash-input");
    expect(total).toHaveTextContent("500,00");
  });

  it("routes the Management fee button to its handler (FEE-010)", () => {
    render(<AccountDetailsView />);
    fireEvent.click(document.querySelector("#add-menu-management-fee")!);
    expect(handlers.handleManagementFeeOpen).toHaveBeenCalledTimes(1);
  });

  // FEE-076 — a disabled account hides every % fee surface
  it("hides the fee button, header total and column when the account has fees disabled (FEE-076)", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ managementFeesEnabled: false }));
    render(<AccountDetailsView />);
    expect(document.querySelector("#add-menu-management-fee")).toBeNull();
    expect(document.querySelector("#account-details-total-management-fees")).toBeNull();
    expect(screen.queryByText("account_details.column_management_fees")).toBeNull();
  });

  // FEE-076 — an enabled account keeps the fee column header
  it("shows the Management Fees column header when the account has fees enabled (FEE-076)", () => {
    render(<AccountDetailsView />);
    expect(screen.getByText("account_details.column_management_fees")).toBeInTheDocument();
  });

  it("mounts the management-fee modal when managementFeeOpen is true (FEE-010)", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ managementFeeOpen: true }));
    render(<AccountDetailsView />);
    expect(screen.getByTestId("management-fee-modal-mounted")).toBeInTheDocument();
  });

  it("mounts the fee-schedule modal when a target is set (FEE-011)", () => {
    mockUseAccountDetailsView.mockReturnValue(
      makeView({ feeScheduleTarget: { assetId: "a1", assetName: "ETF" } }),
    );
    render(<AccountDetailsView />);
    expect(screen.getByTestId("fee-schedule-modal-mounted")).toBeInTheDocument();
  });

  it("hides the Management fee button in the read-only as-of view", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ isAsOf: true, asOfDate: "2024-06-01" }));
    render(<AccountDetailsView />);
    expect(document.querySelector("#add-menu-management-fee")).toBeNull();
  });
});

describe("AccountDetailsView — interest (INT-010/050)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseRefreshAccountPrices.mockReturnValue({ isPending: false, refresh: vi.fn() });
    mockUseAccountDetailsView.mockReturnValue(makeView());
  });

  it("routes the Interest button to its handler (INT-010)", () => {
    render(<AccountDetailsView />);
    fireEvent.click(document.querySelector("#add-menu-interest")!);
    expect(handlers.handleInterestOpen).toHaveBeenCalledTimes(1);
  });

  // INT-050 — the interest surfaces are NOT gated by the fee parameter
  it("keeps the Interest button when the account has management fees disabled (INT-050)", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ managementFeesEnabled: false }));
    render(<AccountDetailsView />);
    expect(document.querySelector("#add-menu-interest")).toBeInTheDocument();
  });

  it("mounts the interest modal when interestOpen is true (INT-010)", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ interestOpen: true }));
    render(<AccountDetailsView />);
    expect(screen.getByTestId("interest-modal-mounted")).toBeInTheDocument();
  });

  it("hides the Interest button in the read-only as-of view (INT-010)", () => {
    mockUseAccountDetailsView.mockReturnValue(makeView({ isAsOf: true, asOfDate: "2024-06-01" }));
    render(<AccountDetailsView />);
    expect(document.querySelector("#add-menu-interest")).toBeNull();
  });
});
