import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { InterestEditModalMount } from "./InterestEditModalMount";

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

// Stub the modal so the mount renders in isolation; surface the edit context.
vi.mock("@/features/account_details/interest_transaction/InterestModal", () => ({
  InterestModal: ({
    editMode,
  }: {
    editMode: { transactionId: string; initialQuantity: string };
  }) => (
    <div data-testid="interest-modal">
      {editMode.transactionId}:{editMode.initialQuantity}
    </div>
  ),
}));

const interestTx = {
  id: "tx-int-1",
  account_id: "acc-1",
  asset_id: "asset-fund-1",
  transaction_type: "Interest",
  date: "2024-06-15",
  quantity: 5_000_000,
  note: null,
};

describe("InterestEditModalMount (INT-040)", () => {
  beforeEach(() => {
    mockUseSearch.mockReset();
    mockGetTransactions.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [{ id: "asset-fund-1", name: "Euro Fund" }] as never,
    } as never);
  });

  it("renders nothing when no edit-interest modal param is present", () => {
    mockUseSearch.mockReturnValue({});
    const { container } = render(<InterestEditModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("fetches the transaction and renders the modal in edit mode with prefilled quantity", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-interest",
      editTxId: "tx-int-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-fund-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [interestTx] });

    render(<InterestEditModalMount />);

    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "asset-fund-1");
    const modal = await screen.findByTestId("interest-modal");
    // 5_000_000 micros prefills as "5.000".
    expect(modal).toHaveTextContent("tx-int-1:5.000");
  });

  it("renders nothing when the transaction is not found in the fetch result", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-interest",
      editTxId: "missing",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-fund-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [interestTx] });

    const { container } = render(<InterestEditModalMount />);
    await waitFor(() => expect(mockGetTransactions).toHaveBeenCalled());
    expect(screen.queryByTestId("interest-modal")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("surfaces the mapped error snackbar when the fetch fails (F27)", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-interest",
      editTxId: "tx-int-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-fund-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });

    render(<InterestEditModalMount />);

    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", "error"),
    );
    expect(screen.queryByTestId("interest-modal")).toBeNull();
  });
});
