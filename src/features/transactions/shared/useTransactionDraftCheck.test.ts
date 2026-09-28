import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TransactionDraft } from "@/bindings";
import type { TransactionFormData } from "./types";
import { toTransactionDraft, useTransactionDraftCheck } from "./useTransactionDraftCheck";

const { mockValidateDraft } = vi.hoisted(() => ({ mockValidateDraft: vi.fn() }));

vi.mock("../gateway", () => ({
  transactionGateway: { validateTransactionDraft: mockValidateDraft },
}));

const FORM: TransactionFormData = {
  accountId: "account-1",
  assetId: "asset-1",
  date: "2026-01-02",
  quantity: "2",
  unitPrice: "100",
  exchangeRate: "1",
  fees: "5",
  note: "",
};

const draftWith = (quantity: string): TransactionDraft =>
  toTransactionDraft("Purchase", { ...FORM, quantity }, "price", "");

describe("toTransactionDraft", () => {
  // TRX-062 — what the form holds, in micro-units, the typed total only in total mode
  it("sends the unit price in price mode and the typed total in total mode", () => {
    expect(toTransactionDraft("Purchase", FORM, "price", "999")).toEqual({
      kind: "Purchase",
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2026-01-02",
      quantity: 2_000_000,
      entered: {
        mode: "UnitPrice",
        unit_price: 100_000_000,
        exchange_rate: 1_000_000,
        fees: 5_000_000,
      },
      correcting: null,
    });
    expect(toTransactionDraft("Sell", FORM, "total", "195", "tx-1")).toEqual(
      expect.objectContaining({
        kind: "Sell",
        entered: { mode: "Total", total: 195_000_000, exchange_rate: 1_000_000, fees: 5_000_000 },
        correcting: "tx-1",
      }),
    );
  });
});

describe("useTransactionDraftCheck", () => {
  beforeEach(() => {
    mockValidateDraft.mockReset();
  });

  // TRX-063 — a clean check returns the figures and allows saving
  it("returns the preview of a clean draft", async () => {
    mockValidateDraft.mockResolvedValue({
      status: "ok",
      data: { unit_price: 100_000_000, total_amount: 205_000_000 },
    });
    const draft = draftWith("2");
    const { result } = renderHook(() => useTransactionDraftCheck(draft));

    expect(result.current.isClean).toBe(false);
    await waitFor(() => expect(result.current.isClean).toBe(true));
    expect(result.current.preview).toEqual({ unit_price: 100_000_000, total_amount: 205_000_000 });
    expect(result.current.problemMessage).toBeNull();
  });

  // TRX-062 — a field not filled yet is reported with the form's message
  it("maps a missing field to the form's message", async () => {
    mockValidateDraft.mockResolvedValue({ status: "error", error: { code: "AssetMissing" } });
    const draft = draftWith("2");
    const { result } = renderHook(() => useTransactionDraftCheck(draft));

    await waitFor(() =>
      expect(result.current.problemMessage).toEqual({ key: "transaction.error_validation_asset" }),
    );
    expect(result.current.problem).toEqual({ code: "AssetMissing" });
    expect(result.current.isClean).toBe(false);
  });

  // TRX-063 — only the latest draft counts: an earlier answer arriving late is dropped
  it("ignores the answer to an earlier draft", async () => {
    let answerFirst: (value: unknown) => void = () => {};
    mockValidateDraft
      .mockImplementationOnce(() => new Promise((resolve) => (answerFirst = resolve)))
      .mockResolvedValueOnce({ status: "error", error: { code: "QuantityNotPositive" } });
    const { result, rerender } = renderHook(({ draft }) => useTransactionDraftCheck(draft), {
      initialProps: { draft: draftWith("2") },
    });

    rerender({ draft: draftWith("0") });
    await waitFor(() => expect(result.current.problem).toEqual({ code: "QuantityNotPositive" }));
    await act(async () => {
      answerFirst({ status: "ok", data: { unit_price: 1, total_amount: 1 } });
    });

    expect(result.current.isClean).toBe(false);
    expect(result.current.problem).toEqual({ code: "QuantityNotPositive" });
  });

  // TRX-063 — the check itself failing shows a generic error and keeps saving disabled
  it("shows a generic error when the check itself fails", async () => {
    mockValidateDraft.mockRejectedValue(new Error("ipc down"));
    const draft = draftWith("2");
    const { result } = renderHook(() => useTransactionDraftCheck(draft));

    await waitFor(() => expect(result.current.problemMessage).toEqual({ key: "error.Unknown" }));
    expect(result.current.isClean).toBe(false);
    expect(result.current.preview).toBeNull();
  });

  // A form with nothing to check sends nothing
  it("checks nothing for a null draft", async () => {
    const { result } = renderHook(() => useTransactionDraftCheck(null));

    await act(async () => {});
    expect(mockValidateDraft).not.toHaveBeenCalled();
    expect(result.current.isClean).toBe(false);
  });
});
