import { describe, expect, it } from "vitest";
import {
  toTransactionRow,
  transactionLoadErrorToI18n,
  transactionMutationErrorToI18n,
} from "./presenter";

const MICRO = 1_000_000;

const openingBalanceTx = {
  id: "tx-ob",
  account_id: "account-1",
  asset_id: "asset-1",
  transaction_type: "OpeningBalance" as const,
  date: "2024-01-10",
  quantity: 2 * MICRO,
  unit_price: 50 * MICRO, // stored computed value (TRX-047: total_cost / quantity)
  exchange_rate: 1 * MICRO,
  fees: 0,
  total_amount: 100 * MICRO,
  note: null,
  realized_pnl: null,
  created_at: "2024-01-10T00:00:00Z",
};

describe("toTransactionRow — OpeningBalance", () => {
  // TRX-052: type field carries "OpeningBalance" so the component can build the i18n key
  it("TRX-052: type is 'OpeningBalance' for i18n key construction", () => {
    const row = toTransactionRow(openingBalanceTx, "Apple", "My Account");
    expect(row.type).toBe("OpeningBalance");
  });

  // TRX-053: unit price column shows the stored computed unit_price (total_cost / quantity)
  it("TRX-053: unitPrice shows stored unit_price value", () => {
    const row = toTransactionRow(openingBalanceTx, "Apple", "My Account");
    expect(row.unitPrice).toBe("50,000");
  });

  // TRX-054: realized P&L is null — no P&L event for opening balance entries
  it("TRX-054: realizedPnl and realizedPnlRaw are null", () => {
    const row = toTransactionRow(openingBalanceTx, "Apple", "My Account");
    expect(row.realizedPnl).toBeNull();
    expect(row.realizedPnlRaw).toBeNull();
  });
});

describe("toTransactionRow — cash transactions (CSH-101)", () => {
  const depositTx = {
    id: "tx-dep",
    account_id: "account-1",
    asset_id: "system-cash-eur",
    transaction_type: "Deposit" as const,
    date: "2025-06-15",
    quantity: 250 * MICRO,
    unit_price: 1 * MICRO,
    exchange_rate: 1 * MICRO,
    fees: 0,
    total_amount: 250 * MICRO,
    note: null,
    realized_pnl: null,
    created_at: "2025-06-15T10:00:00Z",
  };
  const withdrawalTx = { ...depositTx, transaction_type: "Withdrawal" as const, id: "tx-wd" };

  // CSH-101 / TXL-023 — Deposit type round-trips for the i18n key
  it("Deposit type label is 'Deposit'", () => {
    const row = toTransactionRow(depositTx, "Cash EUR", "My Account");
    expect(row.type).toBe("Deposit");
  });

  // CSH-101 / TXL-023 — Withdrawal type round-trips for the i18n key
  it("Withdrawal type label is 'Withdrawal'", () => {
    const row = toTransactionRow(withdrawalTx, "Cash EUR", "My Account");
    expect(row.type).toBe("Withdrawal");
  });

  // TXL-022 — realized P&L is null on cash transactions; UI renders "—"
  it("Deposit realizedPnl is null", () => {
    const row = toTransactionRow(depositTx, "Cash EUR", "My Account");
    expect(row.realizedPnl).toBeNull();
  });
});

// FSD-022 / FSD-050 — FreeShares transaction type round-trips through toTransactionRow
describe("toTransactionRow — FreeShares (FSD-022/050)", () => {
  const MICRO = 1_000_000;
  const freeSharesTx = {
    id: "tx-fsd-1",
    account_id: "account-1",
    asset_id: "asset-equity-1",
    transaction_type: "FreeShares" as const,
    date: "2026-06-12",
    quantity: 5 * MICRO,
    unit_price: 0,
    exchange_rate: 1 * MICRO,
    fees: 0,
    total_amount: 0,
    note: null,
    realized_pnl: null,
    created_at: "2026-06-12T10:00:00Z",
  };

  // FSD-050 — type field carries "FreeShares" so the component can build the i18n key
  // (the key "transaction.type.FreeShares" maps to "Free shares" — FSD-050)
  it("FSD-050: type is 'FreeShares' for i18n key construction", () => {
    const row = toTransactionRow(freeSharesTx, "Apple Inc", "My Account");
    expect(row.type).toBe("FreeShares");
  });

  // FSD-022 — quantity shows the distributed share count (microToFormatted default 3 decimals)
  it("FSD-022: quantity reflects the distributed shares formatted to 3 decimals", () => {
    const row = toTransactionRow(freeSharesTx, "Apple Inc", "My Account");
    expect(row.quantity).toBe("5,000");
  });

  // FSD-023 — total_amount is 0 (no money moved); formatted to 3 decimals
  it("FSD-023: totalAmount is '0,000' (no money moved)", () => {
    const row = toTransactionRow(freeSharesTx, "Apple Inc", "My Account");
    expect(row.totalAmount).toBe("0,000");
  });

  // FSD-023 — realized P&L is null (no capital event)
  it("FSD-023: realizedPnl and realizedPnlRaw are null", () => {
    const row = toTransactionRow(freeSharesTx, "Apple Inc", "My Account");
    expect(row.realizedPnl).toBeNull();
    expect(row.realizedPnlRaw).toBeNull();
  });
});

// SPL-060 — a split row shows the micro-scaled factor riding in `quantity`
// as a trimmed "×N" ratio label.
describe("toTransactionRow — Split (SPL-060)", () => {
  const splitTx = (factorMicros: number) => ({
    id: "tx-spl-1",
    account_id: "account-1",
    asset_id: "asset-equity-1",
    transaction_type: "Split" as const,
    date: "2026-07-01",
    quantity: factorMicros,
    unit_price: 0,
    exchange_rate: 1_000_000,
    fees: 0,
    total_amount: 0,
    note: null,
    realized_pnl: null,
    created_at: "2026-07-01T10:00:00Z",
  });

  it("SPL-060: a whole factor renders without decimals (20_000_000 → '×20')", () => {
    const row = toTransactionRow(splitTx(20_000_000), "Alphabet Inc", "My Account");
    expect(row.quantity).toBe("×20");
  });

  it("SPL-060: a fractional factor renders trimmed (1_500_000 → '×1,5' fr)", () => {
    const row = toTransactionRow(splitTx(1_500_000), "Alphabet Inc", "My Account");
    expect(row.quantity).toBe("×1,5");
  });

  it("SPL-060: a reverse-split factor renders trimmed (100_000 → '×0,1' fr)", () => {
    const row = toTransactionRow(splitTx(100_000), "Alphabet Inc", "My Account");
    expect(row.quantity).toBe("×0,1");
  });

  it("SPL-060: type is 'Split' for i18n key construction", () => {
    const row = toTransactionRow(splitTx(2_000_000), "Alphabet Inc", "My Account");
    expect(row.type).toBe("Split");
  });
});

// F27 layer-3 presenter — exhaustive variant coverage across AccountError
// and OpenHoldingError. Payload-bearing variants get pre-formatted micros (presenter
// owns the data formatting; component owns t()).
describe("transactionMutationErrorToI18n", () => {
  it("InsufficientCash interpolates balance (formatted to 2 decimals) and currency", () => {
    expect(
      transactionMutationErrorToI18n({
        code: "InsufficientCash",
        current_balance_micros: 50_000_000,
        currency: "EUR",
      }),
    ).toEqual({
      key: "cash.insufficient_cash_inline",
      vars: { balance: "50,00", currency: "EUR" },
    });
  });

  it("Oversell interpolates available + requested (formatted to 6 decimals)", () => {
    expect(
      transactionMutationErrorToI18n({
        code: "Oversell",
        available: 1_500_000,
        requested: 2_000_000,
      }),
    ).toEqual({
      key: "error.Oversell",
      vars: { available: "1,500000", requested: "2,000000" },
    });
  });

  it.each([
    "ClosedPosition",
    "CascadingOversell",
    "TransactionNotFound",
    "AccountNotFound",
    "NameAlreadyExists",
    "DatabaseError",
    "InvalidDate",
    "DateInFuture",
    "DateTooOld",
    "QuantityNotPositive",
    "AmountNotPositive",
    "UnitPriceNegative",
    "UnitPriceOutOfRange",
    "FeesNegative",
    "ExchangeRateNotPositive",
    "TotalAmountNotPositive",
    "TotalAmountBelowFees",
    "AssetNotFound",
    "ArchivedAsset",
    "OpeningBalanceOnCashAsset",
    "InvalidTotalCost",
  ] as const)("%s unit variant maps to its flat error key", (code) => {
    // `AccountNotFound` carries `account_id` payload; we strip it because the
    // presenter falls through to the flat key regardless. Other codes are unit.
    const err =
      code === "AccountNotFound"
        ? { code, account_id: "acc-1" }
        : ({ code } as Parameters<typeof transactionMutationErrorToI18n>[0]);
    expect(transactionMutationErrorToI18n(err)).toEqual({ key: `error.${code}` });
  });

  it.each([
    "NameEmpty",
    "InvalidCurrency",
    "NegativeQuantity",
    "NegativeAveragePrice",
  ] as const)("unreachable account-only code %s falls back to error.Unknown", (code) => {
    const err = { code } as Parameters<typeof transactionMutationErrorToI18n>[0];
    expect(transactionMutationErrorToI18n(err)).toEqual({ key: "error.Unknown" });
  });
});

describe("transactionLoadErrorToI18n", () => {
  it("DatabaseError maps to its flat error key", () => {
    expect(transactionLoadErrorToI18n({ code: "DatabaseError" })).toEqual({
      key: "error.DatabaseError",
    });
  });

  it("AccountNotFound maps to its flat error key", () => {
    expect(transactionLoadErrorToI18n({ code: "AccountNotFound", account_id: "acc-1" })).toEqual({
      key: "error.AccountNotFound",
    });
  });

  it("unreachable code falls back to error.Unknown", () => {
    const err = { code: "NameEmpty" } as Parameters<typeof transactionLoadErrorToI18n>[0];
    expect(transactionLoadErrorToI18n(err)).toEqual({ key: "error.Unknown" });
  });
});
