import { renderHook } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BuyHoldingDTO, CorrectTransactionDTO, SellHoldingDTO, Transaction } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

const {
  mockBuyHolding,
  mockSellHolding,
  mockCorrectTransaction,
  mockCancelTransaction,
  mockGetTransactions,
} = vi.hoisted(() => ({
  mockBuyHolding: vi.fn(),
  mockSellHolding: vi.fn(),
  mockCorrectTransaction: vi.fn(),
  mockCancelTransaction: vi.fn(),
  mockGetTransactions: vi.fn(),
}));

vi.mock("./gateway", () => ({
  transactionGateway: {
    buyHolding: mockBuyHolding,
    sellHolding: mockSellHolding,
    correctTransaction: mockCorrectTransaction,
    cancelTransaction: mockCancelTransaction,
    getTransactions: mockGetTransactions,
  },
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn() },
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}::${JSON.stringify(opts)}` : key,
    i18n: { language: "en" },
  }),
}));

const { useTransactions } = await import("./useTransactions");

const makeTx = (): Transaction => ({
  id: "tx-1",
  account_id: "acc-1",
  asset_id: "asset-1",
  transaction_type: "Purchase",
  date: "2024-01-15",
  quantity: 1_000_000,
  unit_price: 100_000_000,
  exchange_rate: 1_000_000,
  fees: 0,
  total_amount: 100_000_000,
  note: null,
  realized_pnl: null,
  created_at: "2024-01-15T10:00:00Z",
});

const buyDto: BuyHoldingDTO = {
  account_id: "acc-1",
  asset_id: "asset-1",
  date: "2024-01-15",
  quantity: 1_000_000,
  unit_price: 100_000_000,
  exchange_rate: 1_000_000,
  fees: 0,
  total_amount: null,
  note: null,
};

describe("useTransactions", () => {
  beforeEach(() => {
    mockBuyHolding.mockReset();
    mockSellHolding.mockReset();
    mockCorrectTransaction.mockReset();
    mockCancelTransaction.mockReset();
    mockGetTransactions.mockReset();
  });

  // ── buyHolding ────────────────────────────────────────────────────────────────

  it("buyHolding returns data on success", async () => {
    const tx = makeTx();
    mockBuyHolding.mockResolvedValue({ status: "ok", data: tx });
    const { result } = renderHook(() => useTransactions());
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.buyHolding(buyDto);
    });
    expect(mockBuyHolding).toHaveBeenCalledWith(buyDto);
    expect(ret.data).toEqual(tx);
    expect(ret.error).toBeNull();
  });

  it("buyHolding returns error code on failure", async () => {
    mockBuyHolding.mockResolvedValue({
      status: "error",
      error: { code: "AccountNotFound" },
    });
    const { result } = renderHook(() => useTransactions());
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.buyHolding(buyDto);
    });
    expect(ret.error).toEqual({ key: "error.AccountNotFound" });
  });

  // CSH-081 — InsufficientCash is formatted with payload (balance + currency)
  it("buyHolding formats InsufficientCash with balance + currency", async () => {
    mockBuyHolding.mockResolvedValue({
      status: "error",
      error: {
        code: "InsufficientCash",
        current_balance_micros: 50_000_000,
        currency: "EUR",
      },
    });
    const { result } = renderHook(() => useTransactions());
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.buyHolding(buyDto);
    });
    expect(ret.error).toEqual({
      key: "cash.insufficient_cash_inline",
      vars: { balance: "50,00", currency: "EUR" },
    });
  });

  // ── sellHolding ───────────────────────────────────────────────────────────────

  it("sellHolding returns data on success", async () => {
    const tx = makeTx();
    mockSellHolding.mockResolvedValue({ status: "ok", data: tx });
    const { result } = renderHook(() => useTransactions());
    const sellDto: SellHoldingDTO = {
      account_id: "acc-1",
      asset_id: "asset-1",
      date: "2024-02-01",
      quantity: 500_000,
      unit_price: 110_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    };
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.sellHolding(sellDto);
    });
    expect(ret.data).toEqual(tx);
    expect(ret.error).toBeNull();
  });

  it("sellHolding returns Oversell error code on failure", async () => {
    mockSellHolding.mockResolvedValue({
      status: "error",
      error: { code: "Oversell", available: 500_000, requested: 999_000_000 },
    });
    const { result } = renderHook(() => useTransactions());
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.sellHolding({
        account_id: "acc-1",
        asset_id: "asset-1",
        date: "2024-02-01",
        quantity: 999_000_000,
        unit_price: 100_000_000,
        exchange_rate: 1_000_000,
        fees: 0,
        total_amount: null,
        note: null,
      });
    });
    expect(ret.error).toEqual({
      key: "error.Oversell",
      vars: { available: "0,500000", requested: "999,000000" },
    });
  });

  // ── correctTransaction ────────────────────────────────────────────────────────

  it("correctTransaction returns data on success", async () => {
    const tx = makeTx();
    mockCorrectTransaction.mockResolvedValue({ status: "ok", data: tx });
    const { result } = renderHook(() => useTransactions());
    const dto: Omit<CorrectTransactionDTO, "account_id" | "transaction_id"> = {
      date: "2024-01-20",
      quantity: 2_000_000,
      unit_price: 90_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    };
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.correctTransaction("tx-1", "acc-1", dto);
    });
    expect(mockCorrectTransaction).toHaveBeenCalledWith("tx-1", "acc-1", dto);
    expect(ret.data).toEqual(tx);
    expect(ret.error).toBeNull();
  });

  it("correctTransaction returns error code on failure", async () => {
    mockCorrectTransaction.mockResolvedValue({
      status: "error",
      error: { code: "TransactionNotFound" },
    });
    const { result } = renderHook(() => useTransactions());
    const dto: Omit<CorrectTransactionDTO, "account_id" | "transaction_id"> = {
      date: "2024-01-20",
      quantity: 2_000_000,
      unit_price: 90_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    };
    let ret: { data: Transaction | null; error: I18nMessage | null } = {
      data: null,
      error: null,
    };
    await act(async () => {
      ret = await result.current.correctTransaction("tx-1", "acc-1", dto);
    });
    expect(ret.error).toEqual({ key: "error.TransactionNotFound" });
  });

  // ── cancelTransaction ─────────────────────────────────────────────────────────

  it("cancelTransaction returns null error on success", async () => {
    mockCancelTransaction.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useTransactions());
    let ret: { error: I18nMessage | null } = { error: { key: "sentinel" } };
    await act(async () => {
      ret = await result.current.cancelTransaction("tx-1", "acc-1");
    });
    expect(mockCancelTransaction).toHaveBeenCalledWith("tx-1", "acc-1");
    expect(ret.error).toBeNull();
  });

  it("cancelTransaction returns error code on failure", async () => {
    mockCancelTransaction.mockResolvedValue({
      status: "error",
      error: { code: "TransactionNotFound" },
    });
    const { result } = renderHook(() => useTransactions());
    let ret: { error: I18nMessage | null } = { error: null };
    await act(async () => {
      ret = await result.current.cancelTransaction("tx-1", "acc-1");
    });
    expect(ret.error).toEqual({ key: "error.TransactionNotFound" });
  });

  // ── getTransactions ───────────────────────────────────────────────────────────

  it("getTransactions returns list on success", async () => {
    const txList = [makeTx()];
    mockGetTransactions.mockResolvedValue({ status: "ok", data: txList });
    const { result } = renderHook(() => useTransactions());
    let ret: Transaction[] = [];
    await act(async () => {
      ret = await result.current.getTransactions("acc-1", "asset-1");
    });
    expect(mockGetTransactions).toHaveBeenCalledWith("acc-1", "asset-1");
    expect(ret).toEqual(txList);
  });

  it("getTransactions returns empty array on error", async () => {
    mockGetTransactions.mockResolvedValue({
      status: "error",
      error: { code: "Unknown" },
    });
    const { result } = renderHook(() => useTransactions());
    let ret: Transaction[] = [makeTx()];
    await act(async () => {
      ret = await result.current.getTransactions("acc-1", "asset-1");
    });
    expect(ret).toEqual([]);
  });
});
