import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { CashTransactionEditMount } from "./CashTransactionEditMount";

const { mockUseSearch, mockGetTransactions, mockShowSnackbar } = vi.hoisted(() => ({
  mockUseSearch: vi.fn(),
  mockGetTransactions: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("@tanstack/react-router", () => ({
  useSearch: () => mockUseSearch(),
  useNavigate: () => vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("@/features/transactions/gateway", () => ({
  transactionGateway: { getTransactions: (...a: unknown[]) => mockGetTransactions(...a) },
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn() } }));

// Stub the cash modals so the mount renders in isolation.
vi.mock("@/features/account_details/deposit_transaction/DepositTransactionModal", () => ({
  DepositTransactionModal: ({ editTransaction }: { editTransaction: { id: string } }) => (
    <div data-testid="deposit-modal">{editTransaction.id}</div>
  ),
}));
vi.mock("@/features/account_details/withdrawal_transaction/WithdrawalTransactionModal", () => ({
  WithdrawalTransactionModal: ({ editTransaction }: { editTransaction: { id: string } }) => (
    <div data-testid="withdrawal-modal">{editTransaction.id}</div>
  ),
}));

const depositTx = {
  id: "tx-dep-1",
  account_id: "acc-1",
  asset_id: "system-cash-eur",
  transaction_type: "Deposit",
};

describe("CashTransactionEditMount (CSH-111)", () => {
  beforeEach(() => {
    mockUseSearch.mockReset();
    mockGetTransactions.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      accounts: [{ id: "acc-1", name: "Main", currency: "EUR" }] as never,
    } as never);
  });

  it("renders nothing when no cash-edit modal param is present", () => {
    mockUseSearch.mockReturnValue({});
    const { container } = render(<CashTransactionEditMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("fetches the transaction and renders the Deposit modal in edit mode", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-cash-deposit",
      editTxId: "tx-dep-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "system-cash-eur",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [depositTx] });

    render(<CashTransactionEditMount />);

    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "system-cash-eur");
    const modal = await screen.findByTestId("deposit-modal");
    expect(modal).toHaveTextContent("tx-dep-1");
  });

  it("renders the Withdrawal modal for an edit-cash-withdrawal param", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-cash-withdrawal",
      editTxId: "tx-wd-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "system-cash-eur",
    });
    mockGetTransactions.mockResolvedValue({
      status: "ok",
      data: [{ ...depositTx, id: "tx-wd-1", transaction_type: "Withdrawal" }],
    });

    render(<CashTransactionEditMount />);

    expect(await screen.findByTestId("withdrawal-modal")).toHaveTextContent("tx-wd-1");
  });

  it("renders nothing when the transaction is not found in the fetch result", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-cash-deposit",
      editTxId: "missing",
      editTxAccountId: "acc-1",
      editTxAssetId: "system-cash-eur",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [depositTx] });

    const { container } = render(<CashTransactionEditMount />);
    await waitFor(() => expect(mockGetTransactions).toHaveBeenCalled());
    expect(screen.queryByTestId("deposit-modal")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("surfaces an error snackbar when the fetch fails (F27)", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-cash-deposit",
      editTxId: "tx-dep-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "system-cash-eur",
    });
    mockGetTransactions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });

    render(<CashTransactionEditMount />);

    await waitFor(() => expect(mockShowSnackbar).toHaveBeenCalledWith("error.Unknown", "error"));
    expect(screen.queryByTestId("deposit-modal")).toBeNull();
  });
});
