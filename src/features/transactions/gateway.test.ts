import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import type {
  AccountError,
  AssetError,
  BuyHoldingDTO,
  CorrectTransactionDTO,
  Event,
  SellHoldingDTO,
  Transaction,
} from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const { transactionGateway } = await import("./gateway");

describe("transactionGateway — subscribeToEvents (TD-011)", () => {
  it("passes the callback the generated event discriminant, not a bare string", () => {
    expectTypeOf(transactionGateway.subscribeToEvents)
      .parameter(0)
      .parameter(0)
      .toEqualTypeOf<Event["type"]>();
  });
});

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

describe("transactionGateway", () => {
  beforeEach(() => vi.clearAllMocks());

  // ── buyHolding ──────────────────────────────────────────────────────────────

  it("buyHolding returns Transaction on success", async () => {
    const tx = makeTx();
    mockInvoke.mockResolvedValue(tx);
    const result = await transactionGateway.buyHolding(buyDto);
    expect(result).toEqual({ status: "ok", data: tx });
    expect(mockInvoke).toHaveBeenCalledWith("buy_holding", { dto: buyDto });
  });

  it("buyHolding returns error on failure", async () => {
    const err: AccountError = {
      code: "AccountNotFound",
      account_id: "acc-1",
    };
    mockInvoke.mockRejectedValue(err);
    const result = await transactionGateway.buyHolding(buyDto);
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── sellHolding ─────────────────────────────────────────────────────────────

  it("sellHolding returns Transaction on success", async () => {
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
    const tx = makeTx();
    mockInvoke.mockResolvedValue(tx);
    const result = await transactionGateway.sellHolding(sellDto);
    expect(result).toEqual({ status: "ok", data: tx });
    expect(mockInvoke).toHaveBeenCalledWith("sell_holding", { dto: sellDto });
  });

  it("sellHolding returns Oversell error", async () => {
    const sellDto: SellHoldingDTO = {
      account_id: "acc-1",
      asset_id: "asset-1",
      date: "2024-02-01",
      quantity: 999_000_000,
      unit_price: 100_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    };
    const err: AccountError = {
      code: "Oversell",
      available: 500_000,
      requested: 999_000_000,
    };
    mockInvoke.mockRejectedValue(err);
    const result = await transactionGateway.sellHolding(sellDto);
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── correctTransaction ───────────────────────────────────────────────────────

  it("correctTransaction returns updated Transaction on success", async () => {
    const dto: Omit<CorrectTransactionDTO, "account_id" | "transaction_id"> = {
      date: "2024-01-20",
      quantity: 2_000_000,
      unit_price: 90_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    };
    const tx = makeTx();
    mockInvoke.mockResolvedValue(tx);
    const result = await transactionGateway.correctTransaction("tx-1", "acc-1", dto);
    expect(result).toEqual({ status: "ok", data: tx });
    expect(mockInvoke).toHaveBeenCalledWith("correct_transaction", {
      dto: { ...dto, account_id: "acc-1", transaction_id: "tx-1" },
    });
  });

  it("correctTransaction returns error on failure", async () => {
    const err: AccountError = { code: "TransactionNotFound" };
    mockInvoke.mockRejectedValue(err);
    const dto: Omit<CorrectTransactionDTO, "account_id" | "transaction_id"> = {
      date: "2024-01-20",
      quantity: 2_000_000,
      unit_price: 90_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: null,
      note: null,
    };
    const result = await transactionGateway.correctTransaction("tx-1", "acc-1", dto);
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── cancelTransaction ────────────────────────────────────────────────────────

  it("cancelTransaction returns null on success", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await transactionGateway.cancelTransaction("tx-1", "acc-1");
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("cancel_transaction", {
      dto: { account_id: "acc-1", transaction_id: "tx-1" },
    });
  });

  it("cancelTransaction returns error on failure", async () => {
    const err: AccountError = { code: "TransactionNotFound" };
    mockInvoke.mockRejectedValue(err);
    const result = await transactionGateway.cancelTransaction("tx-1", "acc-1");
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── getTransactions ──────────────────────────────────────────────────────────

  it("getTransactions returns list on success", async () => {
    const txList = [makeTx()];
    mockInvoke.mockResolvedValue(txList);
    const result = await transactionGateway.getTransactions("acc-1", "asset-1");
    expect(result).toEqual({ status: "ok", data: txList });
    expect(mockInvoke).toHaveBeenCalledWith("get_transactions", {
      accountId: "acc-1",
      assetId: "asset-1",
    });
  });

  // ── getAssetIdsForAccount ────────────────────────────────────────────────────

  it("getAssetIdsForAccount returns string list on success", async () => {
    mockInvoke.mockResolvedValue(["asset-1", "asset-2"]);
    const result = await transactionGateway.getAssetIdsForAccount("acc-1");
    expect(result).toEqual({ status: "ok", data: ["asset-1", "asset-2"] });
    expect(mockInvoke).toHaveBeenCalledWith("get_asset_ids_for_account", {
      accountId: "acc-1",
    });
  });

  it("getAssetIdsForAccount surfaces DatabaseError on repo failure", async () => {
    const err: AccountError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await transactionGateway.getAssetIdsForAccount("acc-1");
    expect(result).toEqual({ status: "error", error: err });
  });

  // ── recordAssetPrice ─────────────────────────────────────────────────────────

  it("recordAssetPrice returns null on success", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await transactionGateway.recordAssetPrice("asset-1", "2024-01-15", 150.5);
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("record_asset_price", {
      assetId: "asset-1",
      date: "2024-01-15",
      price: 150.5,
    });
  });

  it("recordAssetPrice surfaces price-side DatabaseError on repo failure", async () => {
    const err: AssetError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);
    const result = await transactionGateway.recordAssetPrice("asset-1", "2024-01-15", 150.5);
    expect(result).toEqual({ status: "error", error: err });
  });
});
