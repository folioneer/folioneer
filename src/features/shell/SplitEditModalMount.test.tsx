import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { SplitEditModalMount } from "./SplitEditModalMount";

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
vi.mock("@/features/account_details/split_transaction/SplitModal", () => ({
  SplitModal: ({ editMode }: { editMode: { transactionId: string; initialFactor: string } }) => (
    <div data-testid="split-modal">
      {editMode.transactionId}:{editMode.initialFactor}
    </div>
  ),
}));

// The micro-scaled factor rides in `quantity` (SPL-010) — ×2 split.
const splitTx = {
  id: "tx-spl-1",
  account_id: "acc-1",
  asset_id: "asset-equity-1",
  transaction_type: "Split",
  date: "2024-06-15",
  quantity: 2_000_000,
  note: null,
};

describe("SplitEditModalMount (SPL-030)", () => {
  beforeEach(() => {
    mockUseSearch.mockReset();
    mockGetTransactions.mockReset();
    mockShowSnackbar.mockReset();
    useAppStore.setState({
      assets: [{ id: "asset-equity-1", name: "Alphabet Inc" }] as never,
    } as never);
  });

  it("renders nothing when no edit-split modal param is present", () => {
    mockUseSearch.mockReturnValue({});
    const { container } = render(<SplitEditModalMount />);
    expect(container).toBeEmptyDOMElement();
  });

  it("fetches the transaction and renders the modal in edit mode with the prefilled factor", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-split",
      editTxId: "tx-spl-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-equity-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [splitTx] });

    render(<SplitEditModalMount />);

    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "asset-equity-1");
    const modal = await screen.findByTestId("split-modal");
    // 2_000_000 micros prefill as the "2.000" decimal multiplier.
    expect(modal).toHaveTextContent("tx-spl-1:2.000");
  });

  it("renders nothing when the transaction is not found in the fetch result", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-split",
      editTxId: "missing",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-equity-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "ok", data: [splitTx] });

    const { container } = render(<SplitEditModalMount />);
    await waitFor(() => expect(mockGetTransactions).toHaveBeenCalled());
    expect(screen.queryByTestId("split-modal")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("surfaces the mapped error snackbar when the fetch fails (F27)", async () => {
    mockUseSearch.mockReturnValue({
      modal: "edit-split",
      editTxId: "tx-spl-1",
      editTxAccountId: "acc-1",
      editTxAssetId: "asset-equity-1",
    });
    mockGetTransactions.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });

    render(<SplitEditModalMount />);

    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", "error"),
    );
    expect(screen.queryByTestId("split-modal")).toBeNull();
  });
});
