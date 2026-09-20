import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { useAddTransaction } from "./useAddTransaction";

const { mockBuyHolding, mockRecordAssetPrice } = vi.hoisted(() => ({
  mockBuyHolding: vi.fn(),
  mockRecordAssetPrice: vi.fn(),
}));

vi.mock("../useTransactions", () => ({
  useTransactions: () => ({
    buyHolding: mockBuyHolding,
    sellHolding: vi.fn(),
    correctTransaction: vi.fn(),
    cancelTransaction: vi.fn(),
    getTransactions: vi.fn(),
  }),
}));

vi.mock("../gateway", () => ({
  transactionGateway: {
    recordAssetPrice: mockRecordAssetPrice,
  },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "fr" },
  }),
}));

const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useAddTransaction", () => {
  beforeEach(() => {
    localStorage.clear();
    mockBuyHolding.mockReset();
    mockRecordAssetPrice.mockReset();
    useAppStore.setState({
      assets: [
        { id: "asset-1", name: "Apple", is_archived: false, currency: "USD" },
        { id: "asset-archived", name: "OldCo", is_archived: true, currency: "USD" },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "My Account" }] as Account[],
    });
  });

  // TRX-011 — pre-fill assetId from props
  it("pre-fills assetId from prefillAssetId prop", () => {
    const { result } = renderHook(() => useAddTransaction({ prefillAssetId: "asset-1" }));
    expect(result.current.formData.assetId).toBe("asset-1");
  });

  // TRX-011 — pre-fill accountId from props
  it("pre-fills accountId from prefillAccountId prop", () => {
    const { result } = renderHook(() => useAddTransaction({ prefillAccountId: "account-1" }));
    expect(result.current.formData.accountId).toBe("account-1");
  });

  // TRX-026 — totalAmountDisplay is derived from micro values when quantity changes
  it("recalculates totalAmountDisplay when quantity changes", async () => {
    const { result } = renderHook(() => useAddTransaction());

    await act(async () => {
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
      result.current.handleChange("quantity", "2");
    });

    // 2 * 100 * 1 + 0 = 200.000
    expect(result.current.totalAmountDisplay).toBe("200,000");
  });

  // TRX-029 — archived asset triggers confirmation dialog on submit
  it("sets showArchivedConfirm when submitting with an archived asset", async () => {
    const { result } = renderHook(() => useAddTransaction({ prefillAccountId: "account-1" }));

    await act(async () => {
      result.current.handleChange("assetId", "asset-archived");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.showArchivedConfirm).toBe(true);
  });

  // TRX-029 — cancelling archived confirmation does not submit
  it("handleCancelArchived resets showArchivedConfirm without submitting", async () => {
    const { result } = renderHook(() => useAddTransaction({ prefillAccountId: "account-1" }));

    await act(async () => {
      result.current.handleChange("assetId", "asset-archived");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.showArchivedConfirm).toBe(true);

    await act(async () => {
      result.current.handleCancelArchived();
    });

    expect(result.current.showArchivedConfirm).toBe(false);
    expect(mockBuyHolding).not.toHaveBeenCalled();
  });

  // TRX-020 — validation: empty accountId blocks submit
  it("sets error and does not submit when accountId is empty", async () => {
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() => useAddTransaction({ onSubmitSuccess }));

    await act(async () => {
      result.current.handleChange("assetId", "asset-1");
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "10");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).not.toBeNull();
    expect(mockBuyHolding).not.toHaveBeenCalled();
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // Backend error keeps modal open and exposes error
  it("sets error and does not call onSubmitSuccess on backend error", async () => {
    mockBuyHolding.mockResolvedValue({
      data: null,
      error: "Invariant mismatch",
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useAddTransaction({
        prefillAccountId: "account-1",
        prefillAssetId: "asset-1",
        onSubmitSuccess,
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "10");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBe("Invariant mismatch");
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // Success calls onSubmitSuccess
  it("calls onSubmitSuccess on success", async () => {
    mockBuyHolding.mockResolvedValue({
      data: { id: "tx-1" },
      error: null,
    });
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useAddTransaction({
        prefillAccountId: "account-1",
        prefillAssetId: "asset-1",
        onSubmitSuccess,
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-01-15");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "10");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });

  // After a successful submit the form resets but carries the just-entered date forward
  // (last-operation prefill), keyed by the manually-selected account — not today.
  it("resets with the submitted date after success and persists it per account", async () => {
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-reset" }, error: null });
    const { result } = renderHook(() => useAddTransaction());

    await act(async () => {
      result.current.handleChange("accountId", "account-1");
      result.current.handleChange("assetId", "asset-1");
      result.current.handleChange("date", "2018-03-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "10");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.formData.date).toBe("2018-03-01");
    expect(result.current.formData.accountId).toBe("account-1");
    expect(result.current.formData.quantity).toBe("");
    expect(localStorage.getItem("last_operation_date_account-1")).toBe("2018-03-01");
  });

  // handleSubmit with archived asset → does not call buyHolding (waits for confirmation)
  it("handleSubmit with archived asset does not submit immediately", async () => {
    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useAddTransaction({ prefillAccountId: "account-1", onSubmitSuccess }),
    );

    await act(async () => {
      result.current.handleChange("assetId", "asset-archived");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockBuyHolding).not.toHaveBeenCalled();
    expect(onSubmitSuccess).not.toHaveBeenCalled();
  });

  // MKT-052 — recordPrice defaults to false when localStorage key is absent
  it("recordPrice defaults to false when localStorage auto_record_price is absent", () => {
    const { result } = renderHook(() => useAddTransaction());
    expect(result.current.recordPrice).toBe(false);
  });

  // MKT-052 — recordPrice is true at mount when localStorage key is "true"
  it("recordPrice defaults to true when localStorage auto_record_price is true", () => {
    localStorage.setItem("auto_record_price", "true");
    const { result } = renderHook(() => useAddTransaction());
    expect(result.current.recordPrice).toBe(true);
  });

  // MKT-053 — snapshot at mount: mutating localStorage after mount does not change recordPrice
  it("does not change recordPrice when localStorage is mutated after mount (snapshot semantics)", () => {
    const { result } = renderHook(() => useAddTransaction());

    expect(result.current.recordPrice).toBe(false);

    localStorage.setItem("auto_record_price", "true");

    expect(result.current.recordPrice).toBe(false);
  });

  // MKT-054 — calls recordAssetPrice when recordPrice is true and price is non-zero
  it("calls recordAssetPrice when recordPrice is true and price is non-zero", async () => {
    localStorage.setItem("auto_record_price", "true");
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-add-1" }, error: null });
    mockRecordAssetPrice.mockResolvedValue({ status: "ok", data: null });

    const { result } = renderHook(() =>
      useAddTransaction({
        prefillAccountId: "account-1",
        prefillAssetId: "asset-1",
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).toHaveBeenCalledWith("asset-1", "2024-06-01", 100);
  });

  // MKT-054 — does not call recordAssetPrice when recordPrice is false
  it("does not call recordAssetPrice when recordPrice is false", async () => {
    mockBuyHolding.mockResolvedValue({ data: { id: "tx-add-2" }, error: null });

    const { result } = renderHook(() =>
      useAddTransaction({
        prefillAccountId: "account-1",
        prefillAssetId: "asset-1",
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });

  // MKT-061 — skip recordAssetPrice when recordPrice is true but unit_price is 0
  it("does not call recordAssetPrice when recordPrice is true but unit_price is 0", async () => {
    localStorage.setItem("auto_record_price", "true");
    mockBuyHolding.mockResolvedValue({
      data: { id: "tx-zero-price" },
      error: null,
    });
    // mockRecordAssetPrice intentionally not set — it must not be called

    const { result } = renderHook(() =>
      useAddTransaction({
        prefillAccountId: "account-1",
        prefillAssetId: "asset-1",
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "0");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "5"); // fees > 0 so totalMicro > 0, passes validation
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockBuyHolding).toHaveBeenCalled();
    expect(mockRecordAssetPrice).not.toHaveBeenCalled();
  });

  // MKT-062 — recordAssetPrice failure is silent; transaction commits and onSubmitSuccess fires
  it("swallows recordAssetPrice rejection and still calls onSubmitSuccess", async () => {
    localStorage.setItem("auto_record_price", "true");
    mockBuyHolding.mockResolvedValue({
      data: { id: "tx-price-fail" },
      error: null,
    });
    mockRecordAssetPrice.mockRejectedValue(new Error("network error"));

    const onSubmitSuccess = vi.fn();
    const { result } = renderHook(() =>
      useAddTransaction({
        prefillAccountId: "account-1",
        prefillAssetId: "asset-1",
        onSubmitSuccess,
      }),
    );

    await act(async () => {
      result.current.handleChange("date", "2024-06-01");
      result.current.handleChange("quantity", "1");
      result.current.handleChange("unitPrice", "100");
      result.current.handleChange("exchangeRate", "1");
      result.current.handleChange("fees", "0");
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toBeNull();
    expect(onSubmitSuccess).toHaveBeenCalledTimes(1);
  });
});
