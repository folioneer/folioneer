import { configure, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, AccountSummary, PortfolioTotal } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { formatAccountRowTotalUnrealizedPnl } from "../shared/presenter";
import { AccountTable } from "./AccountTable";

// New metric cells use stable `id` attributes (F25, consistent with the existing
// account-row ids); resolve getByTestId against `id`.
configure({ testIdAttribute: "id" });

// Strip i18n — keys come through unchanged for stable assertions.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key} ${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

const mockSummaries = vi.fn<() => AccountSummary[]>();
const mockIsLoading = vi.fn<() => boolean>(() => false);
const mockError = vi.fn<() => unknown>(() => null);
const mockRefetch = vi.fn();
const mockTotal = vi.fn<() => PortfolioTotal | null>(() => null);

vi.mock("../useAccountSummaries", () => ({
  useAccountSummaries: () => ({
    summaries: mockSummaries(),
    portfolioTotal: mockTotal(),
    isLoading: mockIsLoading(),
    error: mockError(),
    refetch: mockRefetch,
  }),
}));

vi.mock("../useAccounts", () => ({
  useAccounts: () => ({
    deleteAccount: vi.fn(),
    getAccountDeletionSummary: vi.fn(),
  }),
}));

// Stub out the child modals; we're testing the table chrome, not the dialogs.
vi.mock("../edit_account_modal/EditAccountModal", () => ({
  EditAccountModal: () => null,
}));
vi.mock("@/ui/components/modal/Dialog", async () => {
  const actual = (await vi.importActual("@/ui/components/modal/Dialog")) as Record<string, unknown>;
  return { ...actual, ConfirmationDialog: () => null };
});

// Force English number formatting so the assertion matches across locales.
vi.mock("@/lib/microUnits", () => ({
  microToFormatted: (micros: number, decimals = 3) =>
    (micros / 1_000_000).toLocaleString("en-US", {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    }),
}));

const makeSummary = (overrides: Partial<AccountSummary> = {}): AccountSummary => ({
  id: "acc-1",
  name: "Main EUR",
  currency: "EUR",
  update_frequency: "ManualMonth",
  total_global_value: 100_000_000,
  total_unrealized_pnl: null,
  ytd_performance_pct: null,
  has_inconsistent_holding: false,
  ...overrides,
});

describe("AccountTable — Global Value column (ACC-021)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsLoading.mockReturnValue(false);
    mockError.mockReturnValue(null);
  });

  it("renders the Global Value column header in the table", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByText("account.column_global_value")).toBeInTheDocument();
  });

  it("formats each row's value with currency suffix", () => {
    mockSummaries.mockReturnValue([
      makeSummary({
        id: "a",
        name: "EUR Account",
        currency: "EUR",
        total_global_value: 47_250_000_000,
      }),
      makeSummary({
        id: "b",
        name: "USD Account",
        currency: "USD",
        total_global_value: 132_000_000_000,
      }),
    ]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    expect(screen.getByText("47,250.00")).toBeInTheDocument();
    expect(screen.getByText("132,000.00")).toBeInTheDocument();
    // Per-row currency suffix renders next to each value.
    expect(screen.getAllByText("EUR")).not.toHaveLength(0);
    expect(screen.getAllByText("USD")).not.toHaveLength(0);
  });

  it("sorts rows by total_global_value when the header is clicked, toggling asc/desc", () => {
    mockSummaries.mockReturnValue([
      makeSummary({ id: "a", name: "Big", total_global_value: 9_000_000_000 }),
      makeSummary({ id: "b", name: "Small", total_global_value: 1_000_000 }),
    ]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    // Default sort is by name asc → "Big" first.
    const firstBodyRow = () => screen.getAllByRole("row")[1] as HTMLElement;
    const valueHeader = () =>
      screen.getByText("account.column_global_value").closest("th") as HTMLElement;
    expect(firstBodyRow().textContent).toContain("Big");
    expect(valueHeader().getAttribute("aria-sort")).toBe("none");

    // First click → ascending by value → "Small" first; aria-sort="ascending".
    fireEvent.click(screen.getByText("account.column_global_value"));
    expect(firstBodyRow().textContent).toContain("Small");
    expect(valueHeader().getAttribute("aria-sort")).toBe("ascending");

    // Second click flips to descending → "Big" first again; aria-sort="descending".
    fireEvent.click(screen.getByText("account.column_global_value"));
    expect(firstBodyRow().textContent).toContain("Big");
    expect(valueHeader().getAttribute("aria-sort")).toBe("descending");
  });

  it("renders 0 as the value when an account has no holdings", () => {
    mockSummaries.mockReturnValue([
      makeSummary({ id: "empty", name: "Empty", currency: "JPY", total_global_value: 0 }),
    ]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    // The value cell should contain "0.00" formatted and the JPY currency suffix.
    const row = screen.getAllByRole("row")[1] as HTMLElement;
    expect(within(row).getByText("0.00")).toBeInTheDocument();
    expect(within(row).getByText("JPY")).toBeInTheDocument();
  });
});

// ACC-026 — Bank column (after Name), value resolved from the account catalog
describe("AccountTable — Bank column (ACC-026)", () => {
  const makeCatalogAccount = (id: string, bank_name: string): Account => ({
    id,
    name: "Main EUR",
    bank_name,
    currency: "EUR",
    update_frequency: "ManualMonth",
    management_fees_enabled: false,
  });

  beforeEach(() => {
    vi.clearAllMocks();
    mockIsLoading.mockReturnValue(false);
    mockError.mockReturnValue(null);
  });

  afterEach(() => {
    useAppStore.setState({ accounts: [] });
  });

  it("renders the Bank column header with its stable id", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    const header = screen.getByText("account.column_bank_name").closest("th") as HTMLElement;
    expect(header).toBeInTheDocument();
    expect(header.getAttribute("id")).toBe("account-column-bank");
  });

  it("renders the bank name resolved from the account catalog", () => {
    useAppStore.setState({ accounts: [makeCatalogAccount("a", "Fortuneo")] });
    mockSummaries.mockReturnValue([makeSummary({ id: "a" })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    expect(screen.getByTestId("account-bank-name-a")).toHaveTextContent("Fortuneo");
  });

  it("renders '—' (dash) when the bank name is unset", () => {
    useAppStore.setState({ accounts: [makeCatalogAccount("b", "")] });
    mockSummaries.mockReturnValue([makeSummary({ id: "b" })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    expect(screen.getByTestId("account-bank-name-b")).toHaveTextContent("—");
  });

  it("Bank column header is sortable (aria-sort changes on click)", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    const header = screen.getByText("account.column_bank_name").closest("th") as HTMLElement;
    expect(header.getAttribute("aria-sort")).toBe("none");

    fireEvent.click(screen.getByText("account.column_bank_name"));
    expect(header.getAttribute("aria-sort")).toBe("ascending");

    fireEvent.click(screen.getByText("account.column_bank_name"));
    expect(header.getAttribute("aria-sort")).toBe("descending");
  });
});

// ACC-023 — Unrealized P&L column (after Global Value)
describe("AccountTable — Unrealized P&L column (ACC-023)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsLoading.mockReturnValue(false);
    mockError.mockReturnValue(null);
  });

  it("renders the Unrealized P&L column header in the table", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByText("account.column_unrealized_pnl")).toBeInTheDocument();
  });

  it("renders formatted P&L value for an account with total_unrealized_pnl set", () => {
    // 1_250_000 micros = 1.25 in account currency (via mocked microToFormatted → en-US)
    mockSummaries.mockReturnValue([makeSummary({ id: "a", total_unrealized_pnl: 1_250_000 })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    // The presenter formats 1_250_000 micros to "1.25"; the mocked microToFormatted
    // (en-US, 2 decimals) produces "1.25".
    expect(screen.getByTestId("account-unrealized-pnl-a")).toHaveTextContent("1.25");
  });

  it("renders '—' (dash) when total_unrealized_pnl is null", () => {
    mockSummaries.mockReturnValue([makeSummary({ id: "b", total_unrealized_pnl: null })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByTestId("account-unrealized-pnl-b")).toHaveTextContent("—");
  });

  it("P&L column header is sortable (aria-sort changes on click)", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    const header = screen.getByText("account.column_unrealized_pnl").closest("th") as HTMLElement;
    expect(header.getAttribute("aria-sort")).toBe("none");

    fireEvent.click(screen.getByText("account.column_unrealized_pnl"));
    expect(header.getAttribute("aria-sort")).toBe("ascending");

    fireEvent.click(screen.getByText("account.column_unrealized_pnl"));
    expect(header.getAttribute("aria-sort")).toBe("descending");
  });
});

// ACC-024 — YTD Performance column (after Unrealized P&L)
describe("AccountTable — YTD Performance column (ACC-024)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsLoading.mockReturnValue(false);
    mockError.mockReturnValue(null);
  });

  it("renders the YTD Performance column header in the table", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByText("account.column_ytd_performance")).toBeInTheDocument();
  });

  it("renders a positive YTD percent with leading '+' sign", () => {
    // 8_000_000 micro-percent = +8.00%
    mockSummaries.mockReturnValue([makeSummary({ id: "a", ytd_performance_pct: 8_000_000 })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByTestId("account-ytd-pct-a")).toHaveTextContent("+8.00%");
  });

  it("renders a negative YTD percent with '-' sign", () => {
    // -3_700_000 micro-percent = -3.70%
    mockSummaries.mockReturnValue([makeSummary({ id: "b", ytd_performance_pct: -3_700_000 })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByTestId("account-ytd-pct-b")).toHaveTextContent("-3.70%");
  });

  it("renders '—' (dash) when ytd_performance_pct is null", () => {
    mockSummaries.mockReturnValue([makeSummary({ id: "c", ytd_performance_pct: null })]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.getByTestId("account-ytd-pct-c")).toHaveTextContent("—");
  });

  it("YTD column header is sortable (aria-sort changes on click)", () => {
    mockSummaries.mockReturnValue([makeSummary()]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    const header = screen.getByText("account.column_ytd_performance").closest("th") as HTMLElement;
    expect(header.getAttribute("aria-sort")).toBe("none");

    fireEvent.click(screen.getByText("account.column_ytd_performance"));
    expect(header.getAttribute("aria-sort")).toBe("ascending");

    fireEvent.click(screen.getByText("account.column_ytd_performance"));
    expect(header.getAttribute("aria-sort")).toBe("descending");
  });
});

const makeTotal = (overrides: Partial<PortfolioTotal> = {}): PortfolioTotal => ({
  total_global_value: 109_510_400_000,
  total_unrealized_pnl: 4_857_600_000,
  currency: "EUR",
  incomplete: false,
  ...overrides,
});

describe("AccountTable — portfolio total row (ACC-027–032)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsLoading.mockReturnValue(false);
    mockError.mockReturnValue(null);
    mockSummaries.mockReturnValue([makeSummary()]);
    mockTotal.mockReturnValue(makeTotal());
  });

  it("closes the list with the portfolio total in the reference currency, YTD left blank", () => {
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    const total = screen.getByTestId("account-portfolio-total");
    expect(within(total).getByText("account.total")).toBeInTheDocument();
    const value = screen.getByTestId("account-portfolio-total-value");
    expect(value).toHaveTextContent("109,510.40");
    expect(value).toHaveTextContent("EUR");
    expect(screen.getByTestId("account-portfolio-total-unrealized-pnl")).toHaveTextContent(
      formatAccountRowTotalUnrealizedPnl(4_857_600_000),
    );
    expect(within(total).queryByText(/%/)).not.toBeInTheDocument();
    expect(screen.queryByTestId("account-portfolio-total-incomplete")).not.toBeInTheDocument();
  });

  it("marks the total incomplete when an unconvertible account hides a figure", () => {
    mockTotal.mockReturnValue(makeTotal({ incomplete: true }));
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);

    expect(screen.getByTestId("account-portfolio-total-incomplete")).toHaveTextContent(
      "account.total_incomplete",
    );
  });

  it("hides the total row while a search filter is active", () => {
    render(<AccountTable searchTerm="Main" onAccountClick={vi.fn()} />);

    expect(screen.getByTestId("account-row-acc-1")).toBeInTheDocument();
    expect(screen.queryByTestId("account-portfolio-total")).not.toBeInTheDocument();
  });

  it("renders no total row while loading, on a load error, or without accounts", () => {
    mockIsLoading.mockReturnValue(true);
    const loading = render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.queryByTestId("account-portfolio-total")).not.toBeInTheDocument();
    loading.unmount();

    mockIsLoading.mockReturnValue(false);
    mockError.mockReturnValue({ key: "error.Unknown" });
    const failed = render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.queryByTestId("account-portfolio-total")).not.toBeInTheDocument();
    failed.unmount();

    mockError.mockReturnValue(null);
    mockSummaries.mockReturnValue([]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    expect(screen.queryByTestId("account-portfolio-total")).not.toBeInTheDocument();
  });

  it("keeps the total row below the accounts, outside the sortable rows", () => {
    mockSummaries.mockReturnValue([
      makeSummary({ id: "a", name: "Big", total_global_value: 9_000_000_000 }),
      makeSummary({ id: "b", name: "Small", total_global_value: 1_000_000 }),
    ]);
    render(<AccountTable searchTerm="" onAccountClick={vi.fn()} />);
    const valueHeader = () => screen.getByText("account.column_global_value");
    // ACC-031 — last in both sort directions, and never a focusable account row.
    for (const direction of ["ascending", "descending"]) {
      fireEvent.click(valueHeader());
      expect(valueHeader().closest("th")?.getAttribute("aria-sort")).toBe(direction);
      const total = screen.getByTestId("account-portfolio-total");
      expect(total.closest("tfoot")).not.toBeNull();
      expect(total.closest("tbody")).toBeNull();
      expect(total).not.toHaveAttribute("tabindex");
    }
  });
});
