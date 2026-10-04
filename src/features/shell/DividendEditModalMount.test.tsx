import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { DividendEditModalMount } from "./DividendEditModalMount";

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

// Stub the dialog so the mount renders in isolation; surface what it is handed.
vi.mock("@/features/account_details/dividend_transaction/EditDividendModal", () => ({
  EditDividendModal: (props: {
    dividend: {
      transactionId: string;
      amount: string;
      exchangeRate: string;
      unitPriceMicro: number;
    };
    accountName: string;
    accountCurrency: string;
    assetName: string;
    assetCurrency: string;
    amountInAccountCurrency: boolean;
  }) => (
    <div data-testid="edit-dividend-modal">
      {[
        props.dividend.transactionId,
        props.dividend.amount,
        props.dividend.exchangeRate,
        props.dividend.unitPriceMicro,
        props.accountName,
        props.accountCurrency,
        props.assetName,
        props.assetCurrency,
        props.amountInAccountCurrency ? "account-currency" : "asset-currency",
      ].join("|")}
    </div>
  ),
}));

const dividendTx = {
  id: "tx-div-1",
  account_id: "acc-1",
  asset_id: "asset-1",
  transaction_type: "Dividend",
  date: "2026-09-15",
  quantity: 18_600_000,
  unit_price: 1_000_000,
  exchange_rate: 921_400,
  note: null,
};

describe("DividendEditModalMount (DIV-040)", () => {
  beforeEach(() => {
    mockUseSearch.mockReset();
    mockGetTransactions.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [{ id: "asset-1", name: "Microsoft", currency: "USD" }] as never,
      accounts: [{ id: "acc-1", name: "PEA", currency: "EUR" }] as never,
    } as never);
  });

  const params = (editTxId: string) => ({
    modal: "edit-dividend",
    editTxId,
    editTxAccountId: "acc-1",
    editTxAssetId: "asset-1",
  });

  it("renders nothing when no edit-dividend param is present", () => {
    mockUseSearch.mockReturnValue({ modal: "edit-interest" });
    const { container } = render(<DividendEditModalMount />);
    expect(container).toBeEmptyDOMElement();
    expect(mockGetTransactions).not.toHaveBeenCalled();
  });

  // The dialog is handed the dividend as recorded, and the names and currencies it states.
  it("fetches the dividend and hands it to the dialog with its account and asset", async () => {
    mockUseSearch.mockReturnValue(params("tx-div-1"));
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [dividendTx] });

    render(<DividendEditModalMount />);

    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "asset-1");
    const modal = await screen.findByTestId("edit-dividend-modal");
    // DIV-044 — the amount and the rate as recorded, to their last decimal: saving without typing
    // must change neither.
    expect(modal).toHaveTextContent(
      "tx-div-1|18.6|0.9214|1000000|PEA|EUR|Microsoft|USD|asset-currency",
    );
  });

  // DIV-043 — a foreign dividend stored at a rate of exactly 1 was typed in the account's
  // currency; one in the account's own currency is not concerned.
  it("recognises a dividend recorded in the account's currency", async () => {
    mockUseSearch.mockReturnValue(params("tx-div-1"));
    mockGetTransactions.mockResolvedValue({
      status: "ok",
      data: [{ ...dividendTx, exchange_rate: 1_000_000 }],
    });
    const foreign = render(<DividendEditModalMount />);
    expect(await screen.findByTestId("edit-dividend-modal")).toHaveTextContent("|account-currency");
    foreign.unmount();

    useAppStore.setState({
      assets: [{ id: "asset-1", name: "ASML", currency: "EUR" }] as never,
    } as never);
    render(<DividendEditModalMount />);
    expect(await screen.findByTestId("edit-dividend-modal")).toHaveTextContent("|asset-currency");
  });

  // Without the account or the asset the dialog could not state them nor know whether a
  // rate applies: nothing is shown rather than a dialog with empty labels.
  it("renders nothing while the store holds no such asset or account", async () => {
    useAppStore.setState({ assets: [] as never } as never);
    mockUseSearch.mockReturnValue(params("tx-div-1"));
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [dividendTx] });

    const { container } = render(<DividendEditModalMount />);
    await waitFor(() => expect(mockGetTransactions).toHaveBeenCalled());
    expect(screen.queryByTestId("edit-dividend-modal")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing when the dividend is no longer there", async () => {
    mockUseSearch.mockReturnValue(params("missing"));
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [dividendTx] });

    const { container } = render(<DividendEditModalMount />);
    await waitFor(() => expect(mockGetTransactions).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("says so when the dividend cannot be loaded (F27)", async () => {
    mockUseSearch.mockReturnValue(params("tx-div-1"));
    mockGetTransactions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });

    const { container } = render(<DividendEditModalMount />);
    await waitFor(() => expect(mockShowSnackbar).toHaveBeenCalledWith(expect.any(String), "error"));
    expect(container).toBeEmptyDOMElement();
  });
});
