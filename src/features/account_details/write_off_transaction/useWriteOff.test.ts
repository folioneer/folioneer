import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { microToFormattedQuantity } from "@/lib/microUnits";
import { formatIsoDateNumeric, todayIso } from "@/ui/format/date";
import type { WriteOffTarget } from "../shared/types";
import { useWriteOff } from "./useWriteOff";

const { mockRecordWriteOff, mockCorrectWriteOff, mockShowSnackbar } = vi.hoisted(() => ({
  mockRecordWriteOff: vi.fn(),
  mockCorrectWriteOff: vi.fn(),
  mockShowSnackbar: vi.fn(),
}));

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    recordWriteOff: mockRecordWriteOff,
    correctWriteOff: mockCorrectWriteOff,
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

// 0.5 BTC held at an average of 20,000.
const target: WriteOffTarget = {
  assetId: "asset-btc",
  assetName: "Bitcoin",
  assetReference: "BTC",
  quantityMicro: 500_000,
  averagePriceMicro: 20_000_000_000,
};

const transaction = {
  id: "tx-1",
  account_id: "account-1",
  asset_id: "asset-btc",
  transaction_type: "WriteOff",
  date: "2024-06-15",
  quantity: 100_000,
} as const;

describe("useWriteOff (WOF-051 to WOF-058)", () => {
  const onSubmitSuccess = vi.fn();

  beforeEach(() => {
    mockRecordWriteOff.mockReset();
    mockCorrectWriteOff.mockReset();
    mockShowSnackbar.mockReset();
    onSubmitSuccess.mockClear();
    localStorage.clear();
  });

  const setup = (editMode?: Parameters<typeof useWriteOff>[0]["editMode"]) =>
    renderHook(() => useWriteOff({ accountId: "account-1", target, onSubmitSuccess, editMode }));

  // WOF-051 — today's date (no last operation date yet) and NetworkFee by default.
  it("starts with today's date, the network-fee reason and no quantity", () => {
    const { result } = setup();
    expect(result.current.formData.date).toBe(todayIso());
    expect(result.current.formData.reason).toBe("NetworkFee");
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.estimatedCostMicro).toBeNull();
  });

  // WOF-052 — the estimate is the quantity at the average price, rounded down.
  it("estimates the cost the quantity takes out at the average price", () => {
    const { result } = setup();
    act(() => result.current.handleChange("quantity", "0.000421"));
    expect(result.current.estimatedCostMicro).toBe(8_420_000);
    expect(result.current.isFormValid).toBe(true);
  });

  // WOF-053 — above what is held: refused under the field, nothing sent.
  it("refuses a quantity above the quantity held without recording", async () => {
    const { result } = setup();
    act(() => result.current.handleChange("quantity", "0.6"));
    expect(result.current.overHolding).toBe(true);
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.estimatedCostMicro).toBeNull();
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(mockRecordWriteOff).not.toHaveBeenCalled();
  });

  // WOF-053 — zero, or a figure below one micro-unit, is no quantity: nothing is sent.
  it.each(["0", "0.0000001"])("keeps the action inactive for %s", async (quantity) => {
    const { result } = setup();
    act(() => result.current.handleChange("quantity", quantity));
    expect(result.current.isFormValid).toBe(false);
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(mockRecordWriteOff).not.toHaveBeenCalled();
  });

  // WOF-056 — the window records through record_write_off with the reason.
  it("records the write-off with its reason and closes on success", async () => {
    mockRecordWriteOff.mockResolvedValue({ status: "ok", data: transaction });
    const { result } = setup();
    act(() => {
      result.current.handleChange("date", "2024-06-15");
      result.current.handleChange("reason", "Lost");
      result.current.handleChange("quantity", "0.1");
      result.current.handleChange("note", "seed lost");
    });
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(mockRecordWriteOff).toHaveBeenCalledWith({
      account_id: "account-1",
      asset_id: "asset-btc",
      date: "2024-06-15",
      quantity: 100_000,
      reason: "Lost",
      note: "seed lost",
    });
    expect(mockShowSnackbar).toHaveBeenCalledWith("write_off.recorded", "success");
    expect(onSubmitSuccess).toHaveBeenCalledOnce();
  });

  // WOF-055 — Oversell shows under the quantity, naming the date, and holds until the
  // quantity changes.
  it("shows an oversell under the quantity until the quantity changes", async () => {
    mockRecordWriteOff.mockResolvedValue({
      status: "error",
      error: { code: "Oversell", available: 200_000, requested: 300_000 },
    });
    const { result } = setup();
    act(() => result.current.handleChange("date", "2024-06-15"));
    act(() => result.current.handleChange("quantity", "0.3"));
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(result.current.refusal).toEqual({
      message: {
        key: "transaction.error_over_held_on",
        vars: {
          held: microToFormattedQuantity(200_000),
          reference: "BTC",
          date: formatIsoDateNumeric("2024-06-15", "en"),
        },
      },
      onQuantity: true,
    });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    act(() => result.current.handleChange("note", "still refused"));
    expect(result.current.refusal).not.toBeNull();
    act(() => result.current.handleChange("quantity", "0.2"));
    expect(result.current.refusal).toBeNull();
  });

  // WOF-055 — a change of the date or of the reason clears the refusal too.
  it.each(["date", "reason"] as const)("clears a refusal when the %s changes", async (field) => {
    mockRecordWriteOff.mockResolvedValue({
      status: "error",
      error: { code: "Oversell", available: 200_000, requested: 300_000 },
    });
    const { result } = setup();
    act(() => result.current.handleChange("quantity", "0.3"));
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(result.current.refusal).not.toBeNull();
    act(() => result.current.handleChange(field, field === "date" ? "2024-06-01" : "Lost"));
    expect(result.current.refusal).toBeNull();
  });

  // WOF-055 — any other refusal below the form.
  it("shows an archived asset below the form", async () => {
    mockRecordWriteOff.mockResolvedValue({ status: "error", error: { code: "ArchivedAsset" } });
    const { result } = setup();
    act(() => result.current.handleChange("quantity", "0.1"));
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(result.current.refusal).toEqual({
      message: { key: "write_off.error_archived" },
      onQuantity: false,
    });
  });

  // WOF-058 — the edit fills the form, counts the write-off back in and corrects it.
  it("corrects a write-off, its quantity counted back into what is held", async () => {
    mockCorrectWriteOff.mockResolvedValue({ status: "ok", data: transaction });
    const { result } = setup({
      transactionId: "tx-1",
      initialDate: "2024-06-15",
      initialQuantity: "0.1",
      initialQuantityMicro: 100_000,
      initialReason: "Dust",
      initialNote: "",
    });
    expect(result.current.heldMicro).toBe(600_000);
    act(() => {
      result.current.handleChange("quantity", "0.6");
      result.current.handleChange("reason", "Lost");
    });
    expect(result.current.overHolding).toBe(false);
    await act(async () => result.current.handleSubmit(fakeSubmit));
    expect(mockCorrectWriteOff).toHaveBeenCalledWith({
      account_id: "account-1",
      transaction_id: "tx-1",
      date: "2024-06-15",
      quantity: 600_000,
      reason: "Lost",
      note: null,
    });
    expect(mockRecordWriteOff).not.toHaveBeenCalled();
  });
});
