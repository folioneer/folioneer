import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { WriteOffEditModalMount } from "./WriteOffEditModalMount";

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

// Stub the form so the mount renders in isolation; surface what it is given.
vi.mock("@/features/account_details/write_off_transaction/WriteOffEditModal", () => ({
  WriteOffEditModal: ({
    transaction,
    assetName,
    assetReference,
  }: {
    transaction: { id: string };
    assetName: string;
    assetReference: string;
  }) => (
    <div data-testid="write-off-modal">
      {transaction.id}:{assetName}:{assetReference}
    </div>
  ),
}));

const writeOffTx = {
  id: "tx-wof-1",
  account_id: "acc-1",
  asset_id: "asset-btc",
  transaction_type: "WriteOff",
  date: "2024-06-15",
  quantity: 100_000,
  note: null,
};

const editParams = {
  modal: "edit-write-off",
  editTxId: "tx-wof-1",
  editTxAccountId: "acc-1",
  editTxAssetId: "asset-btc",
};

describe("WriteOffEditModalMount (WOF-058)", () => {
  beforeEach(() => {
    mockUseSearch.mockReset();
    mockGetTransactions.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [{ id: "asset-btc", name: "Bitcoin", reference: "BTC" }] as never,
    } as never);
  });

  it("renders nothing without the edit-write-off params", () => {
    mockUseSearch.mockReturnValue({});
    const { container } = render(<WriteOffEditModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("opens the form on the write-off with the asset's name and reference", async () => {
    mockUseSearch.mockReturnValue(editParams);
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [writeOffTx] });
    render(<WriteOffEditModalMount />);
    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "asset-btc");
    expect(await screen.findByTestId("write-off-modal")).toHaveTextContent("tx-wof-1:Bitcoin:BTC");
  });

  it("surfaces the mapped error when the fetch fails (F27)", async () => {
    mockUseSearch.mockReturnValue(editParams);
    mockGetTransactions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    render(<WriteOffEditModalMount />);
    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", "error"),
    );
    expect(screen.queryByTestId("write-off-modal")).toBeNull();
  });
});
