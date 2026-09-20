import type { AccountError, OpenHoldingError, Transaction } from "@/bindings";
import { microToFormatted, microToFormattedFactor } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";

/**
 * F27 — Maps any transaction-BC mutation error (buy / sell / correct / cancel /
 * deposit / withdrawal / open holding) to an i18n key + interpolation vars.
 * Pure function: no React, no useTranslation. Micros formatting is performed
 * here (via the project's `microToFormatted` data helper) so components do not
 * need to know about the underlying numeric scale.
 *
 * Handles the codes reachable for these commands; `AccountError` is a BC-wide
 * union, so any unreachable variant falls through to `error.Unknown`.
 */
export function transactionMutationErrorToI18n(err: AccountError | OpenHoldingError): I18nMessage {
  switch (err.code) {
    case "InsufficientCash":
      return {
        key: "cash.insufficient_cash_inline",
        vars: {
          balance: microToFormatted(err.current_balance_micros, 2),
          currency: err.currency,
        },
      };
    case "Oversell":
      return {
        key: "error.Oversell",
        vars: {
          available: microToFormatted(err.available, 6),
          requested: microToFormatted(err.requested, 6),
        },
      };
    case "ClosedPosition":
    case "CascadingOversell":
    case "TransactionNotFound":
    case "AccountNotFound":
    case "NameAlreadyExists":
    case "DatabaseError":
    case "InvalidDate":
    case "DateInFuture":
    case "DateTooOld":
    case "QuantityNotPositive":
    case "AmountNotPositive":
    case "UnitPriceNegative":
    case "UnitPriceOutOfRange":
    case "FeesNegative":
    case "ExchangeRateNotPositive":
    case "TotalAmountNotPositive":
    case "TotalAmountBelowFees":
    case "AssetNotFound":
    case "ArchivedAsset":
    case "OpeningBalanceOnCashAsset":
    case "InvalidTotalCost":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/**
 * F27 — Maps a transaction-load failure (`getTransactions` /
 * `getAllTransactionsForAccount`) to an i18n key. Pure function: no React, no
 * useTranslation. `AccountError` is a BC-wide union; only the read-path codes
 * are mapped and any unreachable variant falls through to `error.Unknown`.
 */
export function transactionLoadErrorToI18n(err: AccountError): I18nMessage {
  switch (err.code) {
    case "DatabaseError":
    case "AccountNotFound":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/** Display-ready shape for a transaction row. */
export interface TransactionRowViewModel {
  id: string;
  accountId: string;
  assetId: string;
  assetName: string;
  accountName: string;
  /** Transaction type label (e.g. "Purchase"). */
  type: string;
  date: string;
  /** Formatted quantity string (3 decimal places). */
  quantity: string;
  /** Formatted unit price string (3 decimal places). */
  unitPrice: string;
  /** Formatted exchange rate string (3 decimal places). */
  exchangeRate: string;
  /** Formatted fees string (3 decimal places). */
  fees: string;
  /** Formatted total amount string (3 decimal places). */
  totalAmount: string;
  note: string | null;
  /** Formatted realized P&L string (3 decimal places), null for Purchase rows (SEL-041). */
  realizedPnl: string | null;
  /** Raw realized P&L in micro-units — used for sign-based color styling (SEL-043). */
  realizedPnlRaw: number | null;
  /**
   * Bank-statement cash columns (account-wide journal only; undefined elsewhere).
   * `cashOut`/`cashIn` are the formatted debit/credit for this row (empty string when
   * the type moves no cash); `balance` is the running cash balance after this row.
   */
  cashOut?: string;
  cashIn?: string;
  balance?: string;
}

/**
 * SPL-060 — a split row shows its factor as a "×N" ratio label in the quantity
 * column, with the micro-scaled factor rendered as a trimmed decimal
 * (20_000_000 → "×20", 1_500_000 → "×1.5", 100_000 → "×0.1").
 */
function formatSplitFactorLabel(factorMicros: number): string {
  return `×${microToFormattedFactor(factorMicros)}`;
}

/**
 * Maps a raw Transaction + contextual names to a display-ready ViewModel (TRX-024).
 */
export function toTransactionRow(
  tx: Transaction,
  assetName: string,
  accountName: string,
): TransactionRowViewModel {
  return {
    id: tx.id,
    accountId: tx.account_id,
    assetId: tx.asset_id,
    assetName,
    accountName,
    type: tx.transaction_type,
    date: tx.date,
    quantity:
      tx.transaction_type === "Split"
        ? formatSplitFactorLabel(tx.quantity)
        : microToFormatted(tx.quantity),
    unitPrice: microToFormatted(tx.unit_price),
    exchangeRate: microToFormatted(tx.exchange_rate),
    fees: microToFormatted(tx.fees),
    totalAmount: microToFormatted(tx.total_amount),
    note: tx.note ?? null,
    realizedPnl: tx.realized_pnl != null ? microToFormatted(tx.realized_pnl) : null,
    realizedPnlRaw: tx.realized_pnl ?? null,
  };
}

/**
 * Formats the bank-statement cash cells for a journal row (account-wide journal).
 * Takes raw micro-unit values — `null` debit/credit means the type moves no cash and
 * renders an empty cell. Keeps all micro→display formatting in the presenter (F5); the
 * running-balance arithmetic itself stays in the hook.
 */
export function toCashStatementCells(input: {
  debitMicros: number | null;
  creditMicros: number | null;
  balanceMicros: number;
}): { cashOut: string; cashIn: string; balance: string } {
  return {
    cashOut: input.debitMicros != null ? microToFormatted(input.debitMicros) : "",
    cashIn: input.creditMicros != null ? microToFormatted(input.creditMicros) : "",
    balance: microToFormatted(input.balanceMicros),
  };
}
