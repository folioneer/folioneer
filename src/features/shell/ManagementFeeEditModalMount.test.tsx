import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { ManagementFeeEditModalMount } from "./ManagementFeeEditModalMount";

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
vi.mock("@/features/account_details/management_fee_transaction/ManagementFeeEditModal", () => ({
  ManagementFeeEditModal: ({
    editContext,
  }: {
    editContext: { transactionId: string; initialQuantity: string; lockedAssetName: string };
  }) => (
    <div data-testid="management-fee-edit-modal">
      {editContext.transactionId}:{editContext.initialQuantity}:{editContext.lockedAssetName}
    </div>
  ),
}));

const managementFeeTx = {
  id: "tx-fee-1",
  account_id: "acc-1",
  asset_id: "asset-equity-1",
  transaction_type: "ManagementFee",
  date: "2024-06-15",
  quantity: 1_000_000,
  note: null,
};

describe("ManagementFeeEditModalMount (FEE-063)", () => {
  beforeEach(() => {
    mockUseSearch.mockReset();
    mockGetTransactions.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [{ id: "asset-equity-1", name: "Apple Inc" }] as never,
    } as never);
  });

  it("renders nothing when no edit-management-fee modal param is present", () => {
    mockUseSearch.mockReturnValue({});
    const { container } = render(<ManagementFeeEditModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("fetches the transaction and renders the modal with prefilled quantity + locked asset", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-management-fee",
      editTxId: "tx-fee-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-equity-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [managementFeeTx] });

    render(<ManagementFeeEditModalMount />);

    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "asset-equity-1");
    const modal = await screen.findByTestId("management-fee-edit-modal");
    // 1_000_000 micros prefills as "1.000"; asset name resolved from the store.
    expect(modal).toHaveTextContent("tx-fee-1:1.000:Apple Inc");
  });

  it("renders nothing when the transaction is not found in the fetch result", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-management-fee",
      editTxId: "missing",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-equity-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [managementFeeTx] });

    const { container } = render(<ManagementFeeEditModalMount />);
    await waitFor(() => expect(mockGetTransactions).toHaveBeenCalled());
    expect(screen.queryByTestId("management-fee-edit-modal")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("surfaces the mapped error snackbar when the fetch fails (F27)", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-management-fee",
      editTxId: "tx-fee-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-equity-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });

    render(<ManagementFeeEditModalMount />);

    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", "error"),
    );
    expect(screen.queryByTestId("management-fee-edit-modal")).toBeNull();
  });
});
