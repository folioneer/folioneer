import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Transaction } from "@/bindings";
import type { TransactionRowViewModel } from "../shared/presenter";
import { AccountJournalPage } from "./AccountJournalPage";

// ── Controlled orchestration hook ───────────────────────────────────────────
const { mockUseAccountJournal, mockCancelTransaction } = vi.hoisted(() => ({
  mockUseAccountJournal: vi.fn(),
  mockCancelTransaction: vi.fn(),
}));

vi.mock("./useAccountJournal", () => ({ useAccountJournal: () => mockUseAccountJournal() }));

vi.mock("../useTransactions", () => ({
  useTransactions: () => ({ cancelTransaction: mockCancelTransaction }),
}));

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => vi.fn(),
  Link: ({ children, to: _to, params: _params, ...rest }: Record<string, unknown>) => (
    <a {...rest}>{children as ReactNode}</a>
  ),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "fr" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));

// Stub the generic edit modal — its mount is asserted, not its internals.
vi.mock("../edit_transaction_modal/EditTransactionModal", () => ({
  EditTransactionModal: () => <div data-testid="edit-modal" />,
}));

const mockShowSnackbar = vi.fn();
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

const rowVm = (over: Partial<TransactionRowViewModel> = {}): TransactionRowViewModel => ({
  id: "tx-1",
  accountId: "acc-1",
  assetId: "asset-1",
  assetName: "Apple",
  accountName: "Main",
  type: "Purchase",
  date: "2024-06-14",
  quantity: "10.000",
  unitPrice: "100.000",
  exchangeRate: "1.000",
  fees: "0.000",
  totalAmount: "1000.000",
  note: null,
  realizedPnl: null,
  realizedPnlRaw: null,
  ...over,
});

const tx = (over: Partial<Transaction> = {}): Transaction =>
  ({ id: "tx-1", account_id: "acc-1", asset_id: "asset-1", ...over }) as Transaction;

const makeHook = (over: Record<string, unknown> = {}) => ({
  accountId: "acc-1",
  isLoading: false,
  error: null,
  sortDirection: "desc" as const,
  filters: { assetId: "", type: "", amountMin: "", amountMax: "" },
  setFilter: vi.fn(),
  clearFilters: vi.fn(),
  toggleSortDirection: vi.fn(),
  assetFilterOptions: [{ value: "asset-1", label: "Apple" }],
  typeFilterOptions: [{ value: "Purchase", label: "Purchase" }],
  filteredSortedRows: [rowVm()],
  transactionById: new Map<string, Transaction>([["tx-1", tx()]]),
  hasTransactions: true,
  refresh: vi.fn().mockResolvedValue(undefined),
  ...over,
});

describe("AccountJournalPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseAccountJournal.mockReturnValue(makeHook());
  });

  it("renders loading skeletons while loading", () => {
    mockUseAccountJournal.mockReturnValue(makeHook({ isLoading: true }));
    const { container } = render(<AccountJournalPage />);
    expect(container.querySelectorAll(".animate-pulse .h-10").length).toBeGreaterThan(0);
  });

  it("renders an error with a retry that refreshes", () => {
    const refresh = vi.fn();
    mockUseAccountJournal.mockReturnValue(makeHook({ error: { key: "x" }, refresh }));
    render(<AccountJournalPage />);
    expect(screen.getByText("transaction.error_load")).toBeInTheDocument();
    fireEvent.click(screen.getByText("action.retry"));
    expect(refresh).toHaveBeenCalled();
  });

  it("renders the empty state when the account has no transactions", () => {
    mockUseAccountJournal.mockReturnValue(
      makeHook({ hasTransactions: false, filteredSortedRows: [] }),
    );
    render(<AccountJournalPage />);
    expect(screen.getByText("transaction.journal_empty")).toBeInTheDocument();
  });

  it("renders the no-match state when filters exclude everything", () => {
    mockUseAccountJournal.mockReturnValue(
      makeHook({ hasTransactions: true, filteredSortedRows: [] }),
    );
    render(<AccountJournalPage />);
    expect(screen.getByText("transaction.journal_no_match")).toBeInTheDocument();
  });

  it("renders the table with the asset column when transactions are present", () => {
    render(<AccountJournalPage />);
    expect(screen.getByText("transaction.column_asset")).toBeInTheDocument();
    // The row's Asset cell shows the asset name (distinct from the filter option).
    expect(document.getElementById("txl-asset-tx-1")).toHaveTextContent("Apple");
  });

  it("routes a filter change through the hook", () => {
    const setFilter = vi.fn();
    mockUseAccountJournal.mockReturnValue(makeHook({ setFilter }));
    render(<AccountJournalPage />);
    fireEvent.change(screen.getByLabelText("transaction.filter_asset_label"), {
      target: { value: "asset-1" },
    });
    expect(setFilter).toHaveBeenCalledWith("assetId", "asset-1");
  });

  it("deletes a transaction: confirm calls cancelTransaction then refreshes", async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    mockCancelTransaction.mockResolvedValue({ error: null });
    mockUseAccountJournal.mockReturnValue(makeHook({ refresh }));
    render(<AccountJournalPage />);

    fireEvent.click(screen.getByRole("button", { name: "action.delete" }));
    const dialog = screen.getByText("transaction.delete_confirm_title").closest("div");
    expect(dialog).toBeTruthy();
    await fireEvent.click(screen.getByText("action.confirm"));

    expect(mockCancelTransaction).toHaveBeenCalledWith("tx-1", "acc-1");
    await vi.waitFor(() => expect(refresh).toHaveBeenCalled());
    expect(mockShowSnackbar).toHaveBeenCalledWith("transaction.success_deleted", "success");
  });

  it("surfaces an error snackbar when delete fails", async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    mockCancelTransaction.mockResolvedValue({ error: { code: "DatabaseError" } });
    mockUseAccountJournal.mockReturnValue(makeHook({ refresh }));
    render(<AccountJournalPage />);

    fireEvent.click(screen.getByRole("button", { name: "action.delete" }));
    await fireEvent.click(screen.getByText("action.confirm"));

    expect(mockCancelTransaction).toHaveBeenCalledWith("tx-1", "acc-1");
    await vi.waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("transaction.error_generic", "error"),
    );
    expect(refresh).not.toHaveBeenCalled();
  });

  it("opens the generic edit modal for a Purchase row", () => {
    render(<AccountJournalPage />);
    fireEvent.click(screen.getByRole("button", { name: "action.edit" }));
    expect(screen.getByTestId("edit-modal")).toBeInTheDocument();
  });
});
