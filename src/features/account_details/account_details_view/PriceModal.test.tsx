import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { logger } from "@/lib/logger";

// Gateway mock — recordAssetPrice is the price-record command (Result<null, AssetError>)
const mockRecordAssetPrice = vi.fn();
const mockSetLastOperationDate = vi.fn();

vi.mock("../gateway", () => ({
  accountDetailsGateway: {
    getAccountDetails: vi.fn(),
    subscribeToEvents: vi.fn(() => Promise.resolve(() => {})),
    recordAssetPrice: (...args: unknown[]) => mockRecordAssetPrice(...args),
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => vi.fn(),
}));

// MKT-011 — the date field seeds from the account's stored last-operation date.
const STORED_DATE = "2024-05-01";
vi.mock("@/lib/lastOperationDateStorage", () => ({
  getLastOperationDate: () => STORED_DATE,
  setLastOperationDate: (...args: unknown[]) => mockSetLastOperationDate(...args),
}));

import type { PriceableAsset } from "../shared/types";
import { usePriceModal } from "./usePriceModal";

const ASSETS: PriceableAsset[] = [
  { assetId: "asset-1", assetName: "Apple Inc", assetCurrency: "EUR" },
  { assetId: "asset-2", assetName: "Tesla Inc", assetCurrency: "USD" },
];

const BASE_PROPS = {
  assets: ASSETS,
  initialAssetId: "asset-1",
  accountId: "account-1",
  onSubmitSuccess: vi.fn(),
};

describe("usePriceModal", () => {
  const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

  beforeEach(() => {
    vi.clearAllMocks();
  });

  // MKT-011 — date seeds from the account's stored last-operation date.
  it("MKT-011 — initialises date to the stored last-operation date", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    expect(result.current.date).toBe(STORED_DATE);
  });

  // MKT-011 — asset is pre-selected to the launched holding; price starts empty.
  it("MKT-011 — pre-selects the initial asset and opens with an empty price", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    expect(result.current.assetId).toBe("asset-1");
    expect(result.current.price).toBe("");
    expect(result.current.selectedCurrency).toBe("EUR");
  });

  // MKT-011 — switching the asset updates the currency and clears the price.
  it("MKT-011 — switching the asset updates currency and clears the price", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    act(() => result.current.handleChange("price", "100"));
    act(() => result.current.handleAssetChange("asset-2"));
    expect(result.current.assetId).toBe("asset-2");
    expect(result.current.selectedCurrency).toBe("USD");
    expect(result.current.price).toBe("");
  });

  // MKT-013 — no extra IPC call when opening the modal.
  it("MKT-013 — no gateway call on mount", () => {
    renderHook(() => usePriceModal(BASE_PROPS));
    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });

  // MKT-020 — submit disabled while price is empty.
  it("MKT-020 — isFormValid is false when price is empty", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    expect(result.current.isFormValid).toBe(false);
  });

  // MKT-020 — submit disabled while date is empty.
  it("MKT-020 — isFormValid is false when date is empty", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    act(() => result.current.handleChange("date", ""));
    expect(result.current.isFormValid).toBe(false);
  });

  // MKT-021 — price ≤ 0 invalid.
  it("MKT-021 — isFormValid is false and error set when price is zero", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    act(() => result.current.handleChange("price", "0"));
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.error).toEqual({ key: "price_modal.error_price_not_positive" });
  });

  // MKT-022 — future date invalid.
  it("MKT-022 — isFormValid is false and error set for a future date", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    act(() => {
      result.current.handleChange("date", "2099-12-31");
      result.current.handleChange("price", "100");
    });
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.error).toEqual({ key: "price_modal.error_future_date" });
  });

  // MKT-022 — malformed date invalid.
  it("MKT-022 — isFormValid is false and error set for malformed date string", () => {
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    act(() => {
      result.current.handleChange("date", "not-a-date");
      result.current.handleChange("price", "100");
    });
    expect(result.current.isFormValid).toBe(false);
    expect(result.current.error).toEqual({ key: "price_modal.error_invalid_date" });
  });

  // MKT-028 — calls onSubmitSuccess on successful record, with the selected asset.
  it("MKT-028 — records the selected asset and calls onSubmitSuccess", async () => {
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => usePriceModal({ ...BASE_PROPS, onSubmitSuccess }));
    await act(async () => {
      result.current.handleChange("date", "2024-05-01");
      result.current.handleChange("price", "150.50");
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-1", "2024-05-01", 150.5);
    expect(onSubmitSuccess).toHaveBeenCalledOnce();
  });

  // MKT-011 — a recorded price stores its date as the account's last-operation date.
  it("MKT-011 — stores the entered date as the account's last-operation date on success", async () => {
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    await act(async () => {
      result.current.handleChange("date", "2024-06-15");
      result.current.handleChange("price", "150.50");
    });
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockSetLastOperationDate).toHaveBeenCalledWith("account-1", "2024-06-15");
  });

  // MKT-011 — a failed record does not persist the date.
  it("MKT-011 — does not store the date when the record fails", async () => {
    mockRecordAssetPrice.mockResolvedValue({ status: "error", error: { code: "NotPositive" } });
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    await act(async () => result.current.handleChange("price", "150.50"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(mockSetLastOperationDate).not.toHaveBeenCalled();
  });

  // MKT-014 — "add another" records, calls onRecorded (not onSubmitSuccess), clears the price.
  it("MKT-014 — handleAddAnother records, calls onRecorded, and clears the price", async () => {
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });
    const onSubmitSuccess = vi.fn();
    const onRecorded = vi.fn();
    const { result } = renderHook(() =>
      usePriceModal({ ...BASE_PROPS, onSubmitSuccess, onRecorded }),
    );
    await act(async () => result.current.handleChange("price", "150.50"));
    await act(async () => {
      await result.current.handleAddAnother();
    });
    expect(onRecorded).toHaveBeenCalledOnce();
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    expect(result.current.price).toBe(""); // cleared for the next entry
    expect(result.current.assetId).toBe("asset-1"); // asset kept
  });

  // MKT-014 — on a backend error, "add another" keeps onRecorded silent and the
  // price intact so the user can correct and retry.
  it("MKT-014 — handleAddAnother does not call onRecorded or clear the price on error", async () => {
    mockRecordAssetPrice.mockResolvedValue({ status: "error", error: { code: "NotPositive" } });
    const onRecorded = vi.fn();
    const { result } = renderHook(() => usePriceModal({ ...BASE_PROPS, onRecorded }));
    await act(async () => result.current.handleChange("price", "150.50"));
    await act(async () => {
      await result.current.handleAddAnother();
    });
    expect(onRecorded).not.toHaveBeenCalled();
    expect(result.current.price).toBe("150.50"); // preserved for correction
    expect(result.current.error).toEqual({ key: "error.NotPositive" });
  });

  // MKT-029 — inline error on backend failure; modal stays (no onSubmitSuccess).
  it("MKT-029 — sets inline error and does not call onSubmitSuccess on backend error", async () => {
    mockRecordAssetPrice.mockResolvedValue({ status: "error", error: { code: "NotPositive" } });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => usePriceModal({ ...BASE_PROPS, onSubmitSuccess }));
    await act(async () => result.current.handleChange("price", "150.50"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(onSubmitSuccess).not.toHaveBeenCalled();
    expect(result.current.error).toEqual({ key: "error.NotPositive" });
  });

  // Gateway throw path — UNKNOWN_ERROR set, isSubmitting cleared.
  it("falls back to UNKNOWN_ERROR and clears isSubmitting when gateway throws", async () => {
    mockRecordAssetPrice.mockRejectedValue(new Error("boom"));
    const { result } = renderHook(() => usePriceModal(BASE_PROPS));
    await act(async () => result.current.handleChange("price", "150.50"));
    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.error).toEqual({ key: "error.Unknown" });
    expect(result.current.isSubmitting).toBe(false);
    expect(logger.error).toHaveBeenCalledWith("[usePriceModal] recordAssetPrice threw", {
      error: expect.any(Error),
    });
  });
});
