import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import type {
  AccountError,
  DepositDTO,
  DividendDTO,
  Event,
  FreeSharesDTO,
  OpenHoldingDTO,
  OpenHoldingError,
  Transaction,
  WithdrawalDTO,
} from "@/bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

// Import after mock is registered so bindings.ts picks up the mock
const { accountDetailsGateway } = await import("./gateway");

describe("accountDetailsGateway — subscribeToEvents (TD-011)", () => {
  it("passes the callback the generated event discriminant, not a bare string", () => {
    expectTypeOf(accountDetailsGateway.subscribeToEvents)
      .parameter(0)
      .parameter(0)
      .toEqualTypeOf<Event["type"]>();
  });
});

describe("accountDetailsGateway — openHolding", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // TRX-042 — happy path: openHolding calls open_holding with wrapped dto and returns Transaction
  it("openHolding returns Transaction on success", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2024-01-15",
      quantity: 5_000_000,
      total_cost: 500_000_000,
    };
    const mockTransaction: Transaction = {
      id: "tx-open-1",
      account_id: "account-1",
      asset_id: "asset-1",
      transaction_type: "OpeningBalance",
      date: "2024-01-15",
      quantity: 5_000_000,
      unit_price: 100_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: 500_000_000,
      note: null,
      realized_pnl: null,
      created_at: "2024-01-15T10:00:00Z",
    };
    // bindings.ts wraps the TAURI_INVOKE result in { status: "ok", data: ... }
    mockInvoke.mockResolvedValue(mockTransaction);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "ok", data: mockTransaction });
    expect(mockInvoke).toHaveBeenCalledWith("open_holding", { dto });
  });

  // TRX-056 — AccountNotFound error is surfaced as { status: "error", error: { code: "AccountNotFound" } }
  it("openHolding returns AccountNotFound on unknown account", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "no-such-account",
      asset_id: "asset-1",
      date: "2024-01-15",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = {
      code: "AccountNotFound",
      account_id: "acc-1",
    };
    // bindings.ts catches the rejection and returns { status: "error", error: e }
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
    expect(mockInvoke).toHaveBeenCalledWith("open_holding", { dto });
  });

  // TRX-056 — AssetNotFound error is surfaced correctly
  it("openHolding returns AssetNotFound on unknown asset", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "no-such-asset",
      date: "2024-01-15",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "AssetNotFound" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
    expect(mockInvoke).toHaveBeenCalledWith("open_holding", { dto });
  });

  // TRX-050 — ArchivedAsset error is surfaced correctly
  it("openHolding returns ArchivedAsset when asset is archived", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "archived-asset",
      date: "2024-01-15",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "ArchivedAsset" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // CSH-061 — OpeningBalanceOnCashAsset error is surfaced correctly
  it("openHolding returns OpeningBalanceOnCashAsset when asset is system Cash", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "system-cash-eur",
      date: "2024-01-15",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "OpeningBalanceOnCashAsset" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  it("openHolding returns DatabaseError on unexpected backend failure", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2024-01-15",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // TRX-044 — QuantityNotPositive error is surfaced correctly
  it("openHolding returns QuantityNotPositive when quantity is zero or negative", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2024-01-15",
      quantity: 0,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "QuantityNotPositive" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // TRX-045 — InvalidTotalCost error (negative total cost) is surfaced correctly
  it("openHolding returns InvalidTotalCost when total_cost is negative", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2024-01-15",
      quantity: 1_000_000,
      total_cost: -1,
    };
    const err: OpenHoldingError = { code: "InvalidTotalCost" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // TRX-046 — DateInFuture error is surfaced correctly
  it("openHolding returns DateInFuture when date is in the future", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "2099-12-31",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "DateInFuture" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // TRX-046 — DateTooOld error is surfaced correctly
  it("openHolding returns DateTooOld when date is before 1900-01-01", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "1899-12-31",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "DateTooOld" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // InvalidDate error is surfaced correctly
  it("openHolding returns InvalidDate when date string cannot be parsed", async () => {
    const dto: OpenHoldingDTO = {
      account_id: "account-1",
      asset_id: "asset-1",
      date: "not-a-date",
      quantity: 1_000_000,
      total_cost: 100_000_000,
    };
    const err: OpenHoldingError = { code: "InvalidDate" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.openHolding(dto);

    expect(result).toEqual({ status: "error", error: err });
  });
});

describe("accountDetailsGateway — recordDeposit (CSH-022)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("recordDeposit returns Transaction on success", async () => {
    const dto: DepositDTO = {
      account_id: "account-1",
      date: "2025-06-15",
      amount_micros: 250_000_000,
      note: null,
    };
    const mockTransaction: Transaction = {
      id: "tx-deposit-1",
      account_id: "account-1",
      asset_id: "system-cash-eur",
      transaction_type: "Deposit",
      date: "2025-06-15",
      quantity: 250_000_000,
      unit_price: 1_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: 250_000_000,
      note: null,
      realized_pnl: null,
      created_at: "2025-06-15T10:00:00Z",
    };
    mockInvoke.mockResolvedValue(mockTransaction);

    const result = await accountDetailsGateway.recordDeposit(dto);

    expect(result).toEqual({ status: "ok", data: mockTransaction });
    expect(mockInvoke).toHaveBeenCalledWith("record_deposit", { dto });
  });

  it("recordDeposit surfaces AccountNotFound", async () => {
    const dto: DepositDTO = {
      account_id: "no-such",
      date: "2025-06-15",
      amount_micros: 250_000_000,
      note: null,
    };
    const err: AccountError = {
      code: "AccountNotFound",
      account_id: "no-such",
    };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDeposit(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  it("recordDeposit surfaces AmountNotPositive", async () => {
    const dto: DepositDTO = {
      account_id: "account-1",
      date: "2025-06-15",
      amount_micros: 0,
      note: null,
    };
    const err: AccountError = { code: "AmountNotPositive" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDeposit(dto);

    expect(result).toEqual({ status: "error", error: err });
  });
});

describe("accountDetailsGateway — fetchAccountAssetPrices (MKT-131, MKT-132)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // MKT-131 / MKT-132 — happy path: dispatch acknowledged, returns null
  it("fetchAccountAssetPrices returns null on successful dispatch", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await accountDetailsGateway.fetchAccountAssetPrices("account-1");
    expect(result).toEqual({ status: "ok", data: null });
    expect(mockInvoke).toHaveBeenCalledWith("fetch_account_asset_prices", {
      accountId: "account-1",
    });
  });

  // MKT-132 — unknown account rejection
  it("fetchAccountAssetPrices surfaces AccountNotFound for unknown account_id", async () => {
    const error = { code: "AccountNotFound", account_id: "no-such" };
    mockInvoke.mockRejectedValue(error);
    const result = await accountDetailsGateway.fetchAccountAssetPrices("no-such");
    expect(result).toEqual({ status: "error", error });
  });

  // MKT-113 — in-flight guard: FetchAlreadyRunning
  it("fetchAccountAssetPrices surfaces FetchAlreadyRunning when another fetch is in progress", async () => {
    const error = { code: "FetchAlreadyRunning" };
    mockInvoke.mockRejectedValue(error);
    const result = await accountDetailsGateway.fetchAccountAssetPrices("account-1");
    expect(result).toEqual({ status: "error", error });
  });

  // MKT-111 — no fetchable holdings for this account
  it("fetchAccountAssetPrices surfaces NoFetchableHoldings when account scope is empty", async () => {
    const error = { code: "NoFetchableHoldings" };
    mockInvoke.mockRejectedValue(error);
    const result = await accountDetailsGateway.fetchAccountAssetPrices("account-1");
    expect(result).toEqual({ status: "error", error });
  });

  // DatabaseError from asset or account BC
  it("fetchAccountAssetPrices surfaces DatabaseError on infrastructure failure", async () => {
    const error = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(error);
    const result = await accountDetailsGateway.fetchAccountAssetPrices("account-1");
    expect(result).toEqual({ status: "error", error });
  });

  // UnknownError catch-all
  it("fetchAccountAssetPrices surfaces UnknownError on unexpected runtime failure", async () => {
    const error = { code: "UnknownError" };
    mockInvoke.mockRejectedValue(error);
    const result = await accountDetailsGateway.fetchAccountAssetPrices("account-1");
    expect(result).toEqual({ status: "error", error });
  });
});

describe("accountDetailsGateway — recordWithdrawal (CSH-032)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("recordWithdrawal returns Transaction on success", async () => {
    const dto: WithdrawalDTO = {
      account_id: "account-1",
      date: "2025-06-15",
      amount_micros: 100_000_000,
      note: null,
    };
    const mockTransaction: Transaction = {
      id: "tx-with-1",
      account_id: "account-1",
      asset_id: "system-cash-eur",
      transaction_type: "Withdrawal",
      date: "2025-06-15",
      quantity: 100_000_000,
      unit_price: 1_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: 100_000_000,
      note: null,
      realized_pnl: null,
      created_at: "2025-06-15T10:00:00Z",
    };
    mockInvoke.mockResolvedValue(mockTransaction);

    const result = await accountDetailsGateway.recordWithdrawal(dto);

    expect(result).toEqual({ status: "ok", data: mockTransaction });
    expect(mockInvoke).toHaveBeenCalledWith("record_withdrawal", { dto });
  });

  // CSH-081 — InsufficientCash carries balance + currency payload
  it("recordWithdrawal surfaces InsufficientCash with payload", async () => {
    const dto: WithdrawalDTO = {
      account_id: "account-1",
      date: "2025-06-15",
      amount_micros: 999_000_000,
      note: null,
    };
    const err: AccountError = {
      code: "InsufficientCash",
      current_balance_micros: 50_000_000,
      currency: "EUR",
    };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordWithdrawal(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // MKT-156 — block toggle invokes the right command and returns ok payload
  it("blockAssetPriceRefresh invokes block_asset_price_refresh with the asset id", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await accountDetailsGateway.blockAssetPriceRefresh("asset-1");
    expect(mockInvoke).toHaveBeenCalledWith("block_asset_price_refresh", { id: "asset-1" });
    expect(result).toEqual({ status: "ok", data: null });
  });

  it("unblockAssetPriceRefresh invokes unblock_asset_price_refresh with the asset id", async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await accountDetailsGateway.unblockAssetPriceRefresh("asset-1");
    expect(mockInvoke).toHaveBeenCalledWith("unblock_asset_price_refresh", { id: "asset-1" });
    expect(result).toEqual({ status: "ok", data: null });
  });

  it("blockAssetPriceRefresh surfaces a typed error", async () => {
    mockInvoke.mockRejectedValue({ code: "CashAssetNotEditable" });
    const result = await accountDetailsGateway.blockAssetPriceRefresh("cash-id");
    expect(result).toEqual({ status: "error", error: { code: "CashAssetNotEditable" } });
  });
});

describe("accountDetailsGateway — backfillHoldingPriceHistory (MKT-190)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("invokes backfill_holding_price_history with the account and asset ids and returns the counts", async () => {
    mockInvoke.mockResolvedValue({ written: 12, already_priced: 3 });
    const result = await accountDetailsGateway.backfillHoldingPriceHistory("account-1", "asset-1");
    expect(mockInvoke).toHaveBeenCalledWith("backfill_holding_price_history", {
      accountId: "account-1",
      assetId: "asset-1",
    });
    expect(result).toEqual({ status: "ok", data: { written: 12, already_priced: 3 } });
  });

  it("surfaces a typed error", async () => {
    mockInvoke.mockRejectedValue({ code: "TickerNotResolved" });
    const result = await accountDetailsGateway.backfillHoldingPriceHistory("account-1", "asset-1");
    expect(result).toEqual({ status: "error", error: { code: "TickerNotResolved" } });
  });
});

describe("accountDetailsGateway — previewManagementFee (FEE-029)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("invokes preview_management_fee with positional args and returns the removal", async () => {
    const removal = {
      held_quantity: 25_000_000,
      removed_quantity: 500_000,
      percent_micros: 2_000_000,
    };
    mockInvoke.mockResolvedValue(removal);
    const result = await accountDetailsGateway.previewManagementFee(
      "account-1",
      "asset-1",
      "2026-09-19",
      24_500_000,
    );
    expect(mockInvoke).toHaveBeenCalledWith("preview_management_fee", {
      accountId: "account-1",
      assetId: "asset-1",
      date: "2026-09-19",
      resultingQuantityMicros: 24_500_000,
    });
    expect(result).toEqual({ status: "ok", data: removal });
  });

  it("surfaces the typed rejection with the held quantity", async () => {
    mockInvoke.mockRejectedValue({
      code: "ResultingQuantityNotBelowHeld",
      held_quantity: 25_000_000,
    });
    const result = await accountDetailsGateway.previewManagementFee(
      "account-1",
      "asset-1",
      "2026-09-19",
      26_000_000,
    );
    expect(result).toEqual({
      status: "error",
      error: { code: "ResultingQuantityNotBelowHeld", held_quantity: 25_000_000 },
    });
  });
});

describe("accountDetailsGateway — recordFreeShares (FSD-022)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FSD-022 — happy path: recordFreeShares passes DTO through and returns Transaction
  it("recordFreeShares returns Transaction on success", async () => {
    const dto: FreeSharesDTO = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-06-12",
      quantity: 5_000_000,
      note: null,
    };
    const mockTransaction: Transaction = {
      id: "tx-fsd-1",
      account_id: "account-1",
      asset_id: "asset-equity-1",
      transaction_type: "FreeShares",
      date: "2026-06-12",
      quantity: 5_000_000,
      unit_price: 0,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: 0,
      note: null,
      realized_pnl: null,
      created_at: "2026-06-12T10:00:00Z",
    };
    mockInvoke.mockResolvedValue(mockTransaction);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "ok", data: mockTransaction });
    expect(mockInvoke).toHaveBeenCalledWith("record_free_shares", { dto });
  });

  // FSD-011 — AccountNotFound
  it("recordFreeShares surfaces AccountNotFound", async () => {
    const dto = {
      account_id: "no-such",
      asset_id: "asset-equity-1",
      date: "2026-06-12",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "AccountNotFound", account_id: "no-such" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
    expect(mockInvoke).toHaveBeenCalledWith("record_free_shares", { dto });
  });

  // FSD-011 — AssetNotFound
  it("recordFreeShares surfaces AssetNotFound", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "no-such-asset",
      date: "2026-06-12",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "AssetNotFound" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // FSD-011 — AssetNotHeld: asset exists but no active holding
  it("recordFreeShares surfaces AssetNotHeld", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-not-held",
      date: "2026-06-12",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "AssetNotHeld" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // FSD-011 — FreeSharesOnCashAsset: distributing asset is a Cash Asset
  it("recordFreeShares surfaces FreeSharesOnCashAsset", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "system-cash-eur",
      date: "2026-06-12",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "FreeSharesOnCashAsset" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // FSD-021 — QuantityNotPositive
  it("recordFreeShares surfaces QuantityNotPositive", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-06-12",
      quantity: 0,
      note: null,
    };
    const err = { code: "QuantityNotPositive" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // FSD-021 — InvalidDate
  it("recordFreeShares surfaces InvalidDate", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "not-a-date",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "InvalidDate" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // FSD-021 — DateInFuture
  it("recordFreeShares surfaces DateInFuture", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2099-12-31",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "DateInFuture" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // FSD-021 — DateTooOld
  it("recordFreeShares surfaces DateTooOld", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "1899-12-31",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "DateTooOld" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DatabaseError — infrastructure failure
  it("recordFreeShares surfaces DatabaseError on infrastructure failure", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-06-12",
      quantity: 5_000_000,
      note: null,
    };
    const err = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordFreeShares(dto);

    expect(result).toEqual({ status: "error", error: err });
  });
});

describe("accountDetailsGateway — recordDividend (DIV-023)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // DIV-023 — happy path: recordDividend passes DTO through and returns Transaction
  it("recordDividend returns Transaction on success", async () => {
    const dto: DividendDTO = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const mockTransaction: Transaction = {
      id: "tx-div-1",
      account_id: "account-1",
      asset_id: "asset-equity-1",
      transaction_type: "Dividend",
      date: "2026-05-31",
      quantity: 1_000_000,
      unit_price: 1_000_000,
      exchange_rate: 1_000_000,
      fees: 0,
      total_amount: 100_000_000,
      note: null,
      realized_pnl: null,
      created_at: "2026-05-31T10:00:00Z",
    };
    mockInvoke.mockResolvedValue(mockTransaction);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "ok", data: mockTransaction });
    expect(mockInvoke).toHaveBeenCalledWith("record_dividend", { dto });
  });

  // DIV-011 — AccountNotFound: unknown account
  it("recordDividend surfaces AccountNotFound", async () => {
    const dto = {
      account_id: "no-such",
      asset_id: "asset-equity-1",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "AccountNotFound", account_id: "no-such" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
    expect(mockInvoke).toHaveBeenCalledWith("record_dividend", { dto });
  });

  // DIV-011 — AssetNotFound: unknown paying asset
  it("recordDividend surfaces AssetNotFound", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "no-such-asset",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "AssetNotFound" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DIV-011 — AssetNotHeld: asset exists but no active holding
  it("recordDividend surfaces AssetNotHeld", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-not-held",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "AssetNotHeld" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DIV-011 — DividendOnCashAsset: paying asset is a Cash Asset
  it("recordDividend surfaces DividendOnCashAsset", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "system-cash-eur",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "DividendOnCashAsset" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DIV-021 — AmountNotPositive: amount is zero
  it("recordDividend surfaces AmountNotPositive", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-05-31",
      amount_micros: 0,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "AmountNotPositive" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DIV-022 — ExchangeRateNotPositive: rate is zero
  it("recordDividend surfaces ExchangeRateNotPositive", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 0,
      note: null,
    };
    const err = { code: "ExchangeRateNotPositive" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DIV-021 — DateInFuture
  it("recordDividend surfaces DateInFuture", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2099-12-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "DateInFuture" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });

  // DatabaseError — infrastructure failure
  it("recordDividend surfaces DatabaseError on infrastructure failure", async () => {
    const dto = {
      account_id: "account-1",
      asset_id: "asset-equity-1",
      date: "2026-05-31",
      amount_micros: 100_000_000,
      exchange_rate: 1_000_000,
      note: null,
    };
    const err = { code: "DatabaseError" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.recordDividend(dto);

    expect(result).toEqual({ status: "error", error: err });
  });
});

describe("accountDetailsGateway — getAccountDetails", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("passes null as_of_date for the live view when no date is given", async () => {
    mockInvoke.mockResolvedValue({ account_name: "Acme" });

    await accountDetailsGateway.getAccountDetails("account-1");

    expect(mockInvoke).toHaveBeenCalledWith("get_account_details", {
      accountId: "account-1",
      asOfDate: null,
    });
  });

  it("passes a non-empty as-of date through to the command", async () => {
    mockInvoke.mockResolvedValue({ account_name: "Acme" });

    await accountDetailsGateway.getAccountDetails("account-1", "2024-06-01");

    expect(mockInvoke).toHaveBeenCalledWith("get_account_details", {
      accountId: "account-1",
      asOfDate: "2024-06-01",
    });
  });

  it("surfaces AccountNotFound for an unknown account", async () => {
    const err: AccountError = { code: "AccountNotFound", account_id: "no-such" };
    mockInvoke.mockRejectedValue(err);

    const result = await accountDetailsGateway.getAccountDetails("no-such", "2024-06-01");

    expect(result).toEqual({ status: "error", error: err });
  });
});
