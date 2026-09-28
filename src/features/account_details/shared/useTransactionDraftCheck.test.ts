import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  type TradeFormFields,
  toTransactionDraft,
  useTransactionDraftCheck,
} from "./useTransactionDraftCheck";

const { mockValidateDraft } = vi.hoisted(() => ({ mockValidateDraft: vi.fn() }));

vi.mock("../gateway", () => ({
  accountDetailsGateway: { validateTransactionDraft: mockValidateDraft },
}));

const FORM: TradeFormFields = {
  accountId: "account-1",
  assetId: "asset-1",
  date: "2026-01-02",
  quantity: "2",
  unitPrice: "100",
  exchangeRate: "1",
  fees: "5",
};

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
    expect(toTransactionDraft("Sell", FORM, "total", "195").entered).toEqual({
      mode: "Total",
      total: 195_000_000,
      exchange_rate: 1_000_000,
      fees: 5_000_000,
    });
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
    const draft = toTransactionDraft("Purchase", FORM, "price", "");
    const { result } = renderHook(() => useTransactionDraftCheck(draft));

    expect(result.current.isClean).toBe(false);
    await waitFor(() => expect(result.current.isClean).toBe(true));
    expect(result.current.preview?.total_amount).toBe(205_000_000);
    expect(result.current.problemMessage).toBeNull();
  });

  // TRX-063 — a problem is mapped to its message
  it("maps a problem to its message", async () => {
    mockValidateDraft.mockResolvedValue({ status: "error", error: { code: "DateMissing" } });
    const draft = toTransactionDraft("Sell", FORM, "price", "");
    const { result } = renderHook(() => useTransactionDraftCheck(draft));

    await waitFor(() =>
      expect(result.current.problemMessage).toEqual({ key: "transaction.error_validation_date" }),
    );
  });

  // TRX-063 — the check itself failing shows a generic error and keeps saving disabled
  it("shows a generic error when the check itself fails", async () => {
    mockValidateDraft.mockRejectedValue(new Error("ipc down"));
    const draft = toTransactionDraft("Sell", FORM, "price", "");
    const { result } = renderHook(() => useTransactionDraftCheck(draft));

    await waitFor(() => expect(result.current.problemMessage).toEqual({ key: "error.Unknown" }));
    expect(result.current.isClean).toBe(false);
  });
});
