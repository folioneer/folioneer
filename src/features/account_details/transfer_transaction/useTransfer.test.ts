import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TransferDestination, TransferTarget } from "../shared/types";
import { type TransferEditMode, useTransfer } from "./useTransfer";

const { mockRecordTransfer, mockCorrectTransfer, mockGetTransferRate, mockShowSnackbar } =
  vi.hoisted(() => ({
    mockRecordTransfer: vi.fn(),
    mockCorrectTransfer: vi.fn(),
    mockGetTransferRate: vi.fn(),
    mockShowSnackbar: vi.fn(),
  }));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordTransfer: mockRecordTransfer,
    correctTransfer: mockCorrectTransfer,
    getTransferRate: mockGetTransferRate,
  },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

// 0.5 BTC held at an average of 20,000 EUR in an EUR account.
const target: TransferTarget = {
  assetId: "asset-btc",
  assetName: "Bitcoin",
  assetReference: "BTC",
  heldQuantity: "0.5",
  averagePriceMicro: 20_000_000_000,
  accountName: "Exchange",
  accountCurrency: "EUR",
};

const destinations: TransferDestination[] = [
  { id: "acc-wallet", name: "Wallet", currency: "EUR" },
  { id: "acc-usd", name: "Broker", currency: "USD" },
];

const editMode: TransferEditMode = {
  transferId: "trf-1",
  toAccountId: "acc-usd",
  initialDate: "2024-06-15",
  initialSent: "0.2",
  initialReceived: "0.19",
  initialRate: "1.08",
  initialSentQuantity: "0.2",
  initialNote: "",
};

describe("useTransfer (TRF-051 to TRF-057)", () => {
  const onSubmitSuccess = vi.fn();

  beforeEach(() => {
    mockRecordTransfer.mockReset();
    mockCorrectTransfer.mockReset();
    mockGetTransferRate.mockReset();
    mockShowSnackbar.mockReset();
    onSubmitSuccess.mockClear();
    localStorage.clear();
  });

  const setup = (edit?: TransferEditMode) =>
    renderHook(() =>
      useTransfer({
        accountId: "acc-exchange",
        target,
        destinations,
        onSubmitSuccess,
        editMode: edit,
      }),
    );

  // TRF-051 — no account chosen; the quantity received follows the quantity sent until typed.
  it("starts with no account and keeps the quantity received on the quantity sent", () => {
    const { result } = setup();
    expect(result.current.formData.toAccountId).toBe("");
    expect(result.current.isFormValid).toBe(false);
    act(() => result.current.handleChange("sent", "0.2"));
    expect(result.current.formData.received).toBe("0.2");
    act(() => result.current.handleChange("received", "0.19"));
    act(() => result.current.handleChange("sent", "0.3"));
    expect(result.current.formData.received).toBe("0.19");
  });

  // TRF-052 — within one currency, no rate is read and none is asked.
  it("reads no rate between accounts of one currency", () => {
    const { result } = setup();
    act(() => result.current.handleChange("toAccountId", "acc-wallet"));
    act(() => result.current.handleChange("sent", "0.2"));
    expect(result.current.betweenCurrencies).toBe(false);
    expect(mockGetTransferRate).not.toHaveBeenCalled();
    expect(result.current.isFormValid).toBe(true);
  });

  // TRF-052 — between currencies, the stored rate fills the field and is read again on a
  // new date, until the owner types one.
  it("fills the stored rate and reads it again on a new date until it is typed", async () => {
    mockGetTransferRate.mockResolvedValue({ status: "ok", data: 1_080_000 });
    const { result } = setup();
    act(() => result.current.handleChange("date", "2024-06-15"));
    act(() => result.current.handleChange("toAccountId", "acc-usd"));
    await waitFor(() => expect(result.current.storedRate).toBe("stored"));
    expect(result.current.formData.rate).toBe("1.080");
    expect(mockGetTransferRate).toHaveBeenLastCalledWith("acc-exchange", "acc-usd", "2024-06-15");

    mockGetTransferRate.mockResolvedValue({ status: "ok", data: 1_100_000 });
    act(() => result.current.handleChange("date", "2024-06-10"));
    await waitFor(() => expect(result.current.formData.rate).toBe("1.100"));

    act(() => result.current.handleChange("rate", "1.2"));
    const calls = mockGetTransferRate.mock.calls.length;
    act(() => result.current.handleChange("date", "2024-06-01"));
    expect(mockGetTransferRate.mock.calls.length).toBe(calls);
    expect(result.current.formData.rate).toBe("1.2");
    expect(result.current.storedRate).toBe("typed");
  });

  // TRF-052 — with no rate stored, the field stays empty and the form waits for one.
  it("leaves the rate empty when none is stored", async () => {
    mockGetTransferRate.mockResolvedValue({ status: "ok", data: null });
    const { result } = setup();
    act(() => result.current.handleChange("toAccountId", "acc-usd"));
    act(() => result.current.handleChange("sent", "0.2"));
    await waitFor(() => expect(result.current.storedRate).toBe("none"));
    expect(result.current.formData.rate).toBe("");
    expect(result.current.isFormValid).toBe(false);
  });

  // TRF-052 — a failed read is shown as no stored rate.
  it("shows a failed rate read as no stored rate", async () => {
    mockGetTransferRate.mockResolvedValue({ status: "error", error: { code: "DatabaseError" } });
    const { result } = setup();
    act(() => result.current.handleChange("toAccountId", "acc-usd"));
    await waitFor(() => expect(result.current.storedRate).toBe("none"));
  });

  // TRF-053 — the cost of what is received at the average price, converted, and the fee.
  it("sums up the cost moved, converted, and the fee in kind", async () => {
    mockGetTransferRate.mockResolvedValue({ status: "ok", data: 1_080_000 });
    const { result } = setup();
    act(() => result.current.handleChange("toAccountId", "acc-usd"));
    await waitFor(() => expect(result.current.storedRate).toBe("stored"));
    act(() => result.current.handleChange("sent", "0.2"));
    act(() => result.current.handleChange("received", "0.19"));
    expect(result.current.summary).toEqual({
      costMicro: 3_800_000_000,
      convertedMicro: 4_104_000_000,
      rateMicro: 1_080_000,
      fee: "0.01",
      feeCostMicro: 200_000_000,
    });
  });

  // TRF-054 — above the quantity held, or received above sent: refused, no summary.
  it("refuses a quantity above the holding and a quantity received above the one sent", () => {
    const { result } = setup();
    act(() => result.current.handleChange("toAccountId", "acc-wallet"));
    act(() => result.current.handleChange("sent", "0.6"));
    expect(result.current.overHolding).toBe(true);
    expect(result.current.summary).toBeNull();
    expect(result.current.isFormValid).toBe(false);
    act(() => result.current.handleChange("sent", "0.2"));
    act(() => result.current.handleChange("received", "0.3"));
    expect(result.current.receivedAboveSent).toBe(true);
    expect(result.current.isFormValid).toBe(false);
  });

  // TRF-055 — saving records the transfer, says so and keeps the date for the next form.
  it("records the transfer and keeps its date", async () => {
    mockRecordTransfer.mockResolvedValue({ status: "ok", data: "trf-1" });
    const { result } = setup();
    act(() => result.current.handleChange("date", "2024-06-15"));
    act(() => result.current.handleChange("toAccountId", "acc-wallet"));
    act(() => result.current.handleChange("sent", "0.2"));
    act(() => result.current.handleChange("note", "cold storage"));
    await act(() => result.current.handleSubmit(fakeSubmit));
    expect(mockRecordTransfer).toHaveBeenCalledWith({
      from_account_id: "acc-exchange",
      to_account_id: "acc-wallet",
      asset_id: "asset-btc",
      date: "2024-06-15",
      quantity_sent: "0.2",
      quantity_received: "0.2",
      exchange_rate: null,
      note: "cold storage",
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("transfer.recorded", "success");
    expect(onSubmitSuccess).toHaveBeenCalled();
    expect(localStorage.length).toBeGreaterThan(0);
  });

  // TRF-055 — a lack of units shows under the quantity sent; it holds until a figure changes.
  it("shows a lack of units under the quantity sent until a figure changes", async () => {
    mockRecordTransfer.mockResolvedValue({
      status: "error",
      error: { code: "Oversell", available: "0.1" },
    });
    const { result } = setup();
    act(() => result.current.handleChange("toAccountId", "acc-wallet"));
    act(() => result.current.handleChange("sent", "0.2"));
    await act(() => result.current.handleSubmit(fakeSubmit));
    expect(result.current.refusal?.onQuantity).toBe(true);
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    act(() => result.current.handleChange("note", "x"));
    expect(result.current.refusal).not.toBeNull();
    act(() => result.current.handleChange("sent", "0.1"));
    expect(result.current.refusal).toBeNull();
  });

  // TRF-057 — the correction: figures filled count as typed, the quantity sent counts back.
  it("corrects the whole transfer with its filled figures", async () => {
    mockCorrectTransfer.mockResolvedValue({ status: "ok", data: null });
    const { result } = setup(editMode);
    expect(result.current.storedRate).toBe("typed");
    expect(mockGetTransferRate).not.toHaveBeenCalled();
    expect(result.current.heldQuantity).toBe("0.7");
    act(() => result.current.handleChange("sent", "0.6"));
    expect(result.current.overHolding).toBe(false);
    expect(result.current.formData.received).toBe("0.19");
    await act(() => result.current.handleSubmit(fakeSubmit));
    expect(mockCorrectTransfer).toHaveBeenCalledWith({
      transfer_id: "trf-1",
      date: "2024-06-15",
      quantity_sent: "0.6",
      quantity_received: "0.19",
      exchange_rate: 1_080_000,
      note: null,
    });
    expect(mockRecordTransfer).not.toHaveBeenCalled();
  });
});
