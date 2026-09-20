import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TransactionListPage } from "./TransactionListPage";

const { mockNavigate, mockUseTransactionList, mockCancelTransaction } = vi.hoisted(() => ({
  mockNavigate: vi.fn(),
  mockUseTransactionList: vi.fn(),
  mockCancelTransaction: vi.fn(),
}));

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => mockNavigate,
  useParams: () => ({ accountId: "acc-1", assetId: "system-cash-eur" }),
  useSearch: () => ({ pendingTransactionAssetId: undefined }),
}));

vi.mock("./useTransactionList", () => ({
  useTransactionList: () => mockUseTransactionList(),
}));

vi.mock("../useTransactions", () => ({
  useTransactions: () => ({ cancelTransaction: mockCancelTransaction }),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { info: vi.fn(), error: vi.fn() } }));
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({ useSnackbar: () => vi.fn() }));
vi.mock("../add_transaction/AddTransactionModal", () => ({ AddTransactionModal: () => null }));
vi.mock("../edit_transaction_modal/EditTransactionModal", () => ({
  EditTransactionModal: () => <div data-testid="generic-edit-modal" />,
}));

const row = (id: string, type: string) => ({
  id,
  type,
  date: "2026-05-01",
  quantity: "1.000",
  unitPrice: "1.000",
  exchangeRate: "1.000",
  fees: "0.000",
  totalAmount: "100.000",
  realizedPnl: "—",
  realizedPnlRaw: null,
});

const raw = (id: string, transaction_type: string) => ({
  id,
  transaction_type,
  account_id: "acc-1",
  asset_id: "system-cash-eur",
});

const makeListState = (overrides: Record<string, unknown> = {}) => ({
  selectedAccountId: "acc-1",
  selectedAssetId: "system-cash-eur",
  accountOptions: [{ label: "Main", value: "acc-1" }],
  assetOptions: [{ label: "Cash EUR", value: "system-cash-eur" }],
  isLoadingAssets: false,
  assetListError: null,
  isLoadingTransactions: false,
  transactionError: null,
  sortDirection: "desc",
  sortedTransactions: [row("tx-dep", "Deposit"), row("tx-buy", "Purchase")],
  transactions: [raw("tx-dep", "Deposit"), raw("tx-buy", "Purchase")],
  transactionById: new Map([
    ["tx-dep", raw("tx-dep", "Deposit")],
    ["tx-buy", raw("tx-buy", "Purchase")],
  ]),
  handleAccountChange: vi.fn(),
  handleAssetChange: vi.fn(),
  toggleSortDirection: vi.fn(),
  handleDeleteSuccess: vi.fn(),
  handleEditSuccess: vi.fn(),
  retryAssetList: vi.fn(),
  retryTransactions: vi.fn(),
  ...overrides,
});

describe("TransactionListPage — cash-row edit dispatch (CSH-111)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseTransactionList.mockReturnValue(makeListState());
  });

  it("routes a Deposit-row edit to the cash modal via URL params (not the generic modal)", () => {
    render(<TransactionListPage />);
    const editButtons = screen.getAllByRole("button", { name: /action\.edit/i });
    // Row order is sortedTransactions order: [Deposit, Purchase].
    fireEvent.click(editButtons[0]!);

    expect(mockNavigate).toHaveBeenCalledTimes(1);
    const navArg = mockNavigate.mock.calls[0]![0] as { search: (p: object) => object };
    const patch = navArg.search({});
    expect(patch).toMatchObject({
      modal: "edit-cash-deposit",
      editTxId: "tx-dep",
      editTxAccountId: "acc-1",
      editTxAssetId: "system-cash-eur",
    });
    expect(screen.queryByTestId("generic-edit-modal")).toBeNull();
  });

  it("routes a Purchase-row edit to the generic Edit Transaction modal", () => {
    render(<TransactionListPage />);
    const editButtons = screen.getAllByRole("button", { name: /action\.edit/i });
    fireEvent.click(editButtons[1]!);

    expect(mockNavigate).not.toHaveBeenCalled();
    expect(screen.getByTestId("generic-edit-modal")).toBeInTheDocument();
  });

  it("routes a Withdrawal-row edit to the cash withdrawal modal", () => {
    mockUseTransactionList.mockReturnValue(
      makeListState({
        sortedTransactions: [row("tx-wd", "Withdrawal")],
        transactionById: new Map([["tx-wd", raw("tx-wd", "Withdrawal")]]),
      }),
    );
    render(<TransactionListPage />);
    fireEvent.click(screen.getByRole("button", { name: /action\.edit/i }));

    const navArg = mockNavigate.mock.calls[0]![0] as { search: (p: object) => object };
    expect(navArg.search({})).toMatchObject({ modal: "edit-cash-withdrawal", editTxId: "tx-wd" });
  });

  it("does nothing when the raw transaction is missing from the lookup", () => {
    mockUseTransactionList.mockReturnValue(
      makeListState({
        sortedTransactions: [row("tx-ghost", "Deposit")],
        transactionById: new Map(), // row present in the list, absent from the lookup
      }),
    );
    render(<TransactionListPage />);
    fireEvent.click(screen.getByRole("button", { name: /action\.edit/i }));

    expect(mockNavigate).not.toHaveBeenCalled();
    expect(screen.queryByTestId("generic-edit-modal")).toBeNull();
  });
});
