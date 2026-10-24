import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { TransferEditModal } from "./TransferEditModal";

const { mockGetTransfer, mockGetAccountDetails, mockShowSnackbar } = vi.hoisted(() => ({
  mockGetTransfer: vi.fn(),
  mockGetAccountDetails: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    getTransfer: mockGetTransfer,
    getAccountDetails: mockGetAccountDetails,
  },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn() } }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

// Stub the form; surface the target, the accounts and the edit context it is given.
vi.mock("./TransferModal", () => ({
  TransferModal: ({
    accountId,
    target,
    destinations,
    editMode,
  }: {
    accountId: string;
    target: { quantityMicro: number; averagePriceMicro: number; accountCurrency: string };
    destinations: { id: string }[];
    editMode: { toAccountId: string; initialRate: string; initialSentMicro: number };
  }) => (
    <div data-testid="transfer-form">
      {accountId}:{target.quantityMicro}:{target.averagePriceMicro}:{target.accountCurrency}:
      {destinations.map((d) => d.id).join(",")}:{editMode.toAccountId}:{editMode.initialRate}:
      {editMode.initialSentMicro}
    </div>
  ),
}));

// 0.2 BTC sent at a cost of 4,000: its average price is 20,000.
const transfer = {
  transfer_id: "trf-1",
  from_account_id: "acc-exchange",
  to_account_id: "acc-usd",
  asset_id: "asset-btc",
  date: "2024-06-15",
  quantity_sent: 200_000,
  quantity_received: 190_000,
  cost: 4_000_000_000,
  exchange_rate: 1_080_000,
  note: null,
};

const props = { transferId: "trf-1", onClose: vi.fn(), onSubmitSuccess: vi.fn() };

describe("TransferEditModal (TRF-057)", () => {
  beforeEach(() => {
    mockGetTransfer.mockReset();
    mockGetAccountDetails.mockReset();
    mockShowSnackbar.mockReset();
    props.onClose.mockReset();
    useAppStore.setState({
      assets: [{ id: "asset-btc", name: "Bitcoin", reference: "BTC" }] as never,
      accounts: [
        { id: "acc-exchange", name: "Exchange", currency: "EUR" },
        { id: "acc-usd", name: "Broker", currency: "USD" },
      ] as never,
    } as never);
  });

  // TRF-057 — the form for the whole transfer: sent from its own account, at its own average
  // price, with what is held today and its other accounts.
  it("opens the form for the whole transfer from its sending account", async () => {
    mockGetTransfer.mockResolvedValue({ status: "ok", data: transfer });
    mockGetAccountDetails.mockResolvedValue({
      status: "ok",
      data: { holdings: [{ asset_id: "asset-btc", quantity: 300_000 }] },
    });
    render(<TransferEditModal {...props} />);
    await waitFor(() => expect(screen.getByTestId("transfer-form")).toBeInTheDocument());
    expect(mockGetAccountDetails).toHaveBeenCalledWith("acc-exchange");
    expect(screen.getByTestId("transfer-form").textContent).toBe(
      "acc-exchange:300000:21052631578:EUR:acc-usd:acc-usd:1.080:200000",
    );
  });

  // TRF-057 — nothing held once the transfer took it all.
  it("reads nothing held once the transfer moved it all", async () => {
    mockGetTransfer.mockResolvedValue({ status: "ok", data: transfer });
    mockGetAccountDetails.mockResolvedValue({ status: "ok", data: { holdings: [] } });
    render(<TransferEditModal {...props} />);
    await waitFor(() =>
      expect(screen.getByTestId("transfer-form").textContent).toMatch(/^acc-exchange:0:/),
    );
  });

  // TRF-060 — a transfer no longer whole: its message, and the form closes.
  it("says so and closes when the transfer is not read", async () => {
    mockGetTransfer.mockResolvedValue({ status: "error", error: { code: "TransferNotFound" } });
    render(<TransferEditModal {...props} />);
    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("transfer.error_TransferNotFound", "error"),
    );
    expect(props.onClose).toHaveBeenCalled();
    expect(screen.queryByTestId("transfer-form")).toBeNull();
  });

  // A failed read of the holding: its message, and the form closes.
  it("says so and closes when the holding is not read", async () => {
    mockGetTransfer.mockResolvedValue({ status: "ok", data: transfer });
    mockGetAccountDetails.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    render(<TransferEditModal {...props} />);
    await waitFor(() =>
      expect(mockShowSnackbar).toHaveBeenCalledWith("error.DatabaseError", "error"),
    );
    expect(props.onClose).toHaveBeenCalled();
  });
});
