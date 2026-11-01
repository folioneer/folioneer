import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Transaction } from "@/bindings";
import { WriteOffEditModal } from "./WriteOffEditModal";

const { mockGetAccountDetails, mockShowSnackbar } = vi.hoisted(() => ({
  mockGetAccountDetails: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: { getAccountDetails: mockGetAccountDetails },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn() } }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

// Stub the form; surface the target and the edit context it is given.
vi.mock("./WriteOffModal", () => ({
  WriteOffModal: ({
    target,
    editMode,
  }: {
    target: { heldQuantity: string; averagePriceMicro: number };
    editMode: { initialReason: string; initialReadQuantity: number };
  }) => (
    <div data-testid="write-off-form">
      {target.heldQuantity}:{target.averagePriceMicro}:{editMode.initialReason}:
      {editMode.initialReadQuantity}
    </div>
  ),
}));

// 0.1 BTC written off at a cost of 2,000: its average price is 20,000.
const transaction = {
  id: "tx-wof-1",
  account_id: "acc-1",
  asset_id: "asset-btc",
  transaction_type: "WriteOff",
  date: "2024-06-15",
  quantity: "0.1",
  realized_pnl: -2_000_000_000,
  write_off_reason: "Lost",
  note: null,
} as unknown as Transaction;

const props = {
  accountId: "acc-1",
  transaction,
  assetName: "Bitcoin",
  assetReference: "BTC",
  onClose: vi.fn(),
  onSubmitSuccess: vi.fn(),
};

describe("WriteOffEditModal (WOF-058)", () => {
  beforeEach(() => {
    mockGetAccountDetails.mockReset();
    mockShowSnackbar.mockReset();
    props.onClose.mockClear();
  });

  it("fills the form with what is held today and the write-off's own average price", async () => {
    mockGetAccountDetails.mockResolvedValue({
      status: "ok",
      data: { holdings: [{ asset_id: "asset-btc", quantity: "0.4" }] },
    });
    render(<WriteOffEditModal {...props} />);
    expect(await screen.findByTestId("write-off-form")).toHaveTextContent(
      "0.4:20000000000:Lost:0.1",
    );
  });

  it("holds nothing when the write-off closed the position", async () => {
    mockGetAccountDetails.mockResolvedValue({ status: "ok", data: { holdings: [] } });
    render(<WriteOffEditModal {...props} />);
    expect(await screen.findByTestId("write-off-form")).toHaveTextContent(/^0:/);
  });

  it("closes with the mapped error when the holding cannot be read (F27)", async () => {
    mockGetAccountDetails.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    render(<WriteOffEditModal {...props} />);
    await waitFor(() => expect(props.onClose).toHaveBeenCalled());
    expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", "error");
    expect(screen.queryByTestId("write-off-form")).toBeNull();
  });
});
