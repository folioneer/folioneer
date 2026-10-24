import type {
  AccountError,
  OpenHoldingError,
  Transaction,
  TransactionDraftError,
  WriteOffReason,
} from "@/bindings";
import {
  microToFormatted,
  microToFormattedFactor,
  microToFormattedQuantity,
} from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
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
    case "TradeOnCashAsset":
    case "ArchivedAssetSell":
    case "InvalidTotalCost":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/**
 * F27 — Maps the first problem of a transaction draft (TRX-062) to an i18n key: a field
 * not filled yet, or the recording rejection the draft would meet.
 */
export function transactionDraftErrorToI18n(err: TransactionDraftError): I18nMessage {
  switch (err.code) {
    case "AccountMissing":
      return { key: "transaction.error_validation_account" };
    case "AssetMissing":
      return { key: "transaction.error_validation_asset" };
    case "DateMissing":
      return { key: "transaction.error_validation_date" };
    case "TotalCostMissing":
      return { key: "transaction.error_validation_total_cost" };
    default:
      return transactionMutationErrorToI18n(err);
  }
}

/**
 * F27 — Maps a transaction-load failure (`getTransactions` /
 * `getAccountJournal`) to an i18n key. Pure function: no React, no
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
  /** WOF-057 — a write-off's reason, read with its type; null or undefined for other types. */
  writeOffReason?: WriteOffReason | null;
  /**
   * TRF-056 — for a part of a transfer, the transfer's other account, read with the journal;
   * null for any other row and for a part left alone (TRF-041).
   */
  transferAccount: { id: string; name: string } | null;
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
   * AGT-045 — the agent session that recorded the row (account-wide journal only); null or
   * undefined for a transaction the owner typed.
   */
  recordedBy?: { agent: string; sessionStartedAt: string } | null;
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
  transferAccount: { id: string; name: string } | null = null,
): TransactionRowViewModel {
  return {
    id: tx.id,
    accountId: tx.account_id,
    assetId: tx.asset_id,
    assetName,
    accountName,
    type: tx.transaction_type,
    writeOffReason: tx.write_off_reason ?? null,
    transferAccount: tx.transfer_id ? transferAccount : null,
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

/**
 * SEL-046 / WOF-055 — the refusal of a sale or a write-off for lack of units, recorded or
 * corrected, as it shows under the quantity: what was held of `reference` on `isoDate`, or
 * that a later transaction would be left short; null for any other refusal. Each feature that takes
 * units out has its own copy (DEBT-008); a change here is made there too.
 */
export function takeOutRefusal(
  error: { code: string; available?: number },
  reference: string,
  isoDate: string,
  locale: string,
): I18nMessage | null {
  const date = formatIsoDateNumeric(isoDate, locale);
  switch (error.code) {
    case "Oversell":
      if (error.available === undefined) return null;
      return {
        key: "transaction.error_over_held_on",
        vars: { held: microToFormattedQuantity(error.available), reference, date },
      };
    case "ClosedPosition":
      return { key: "transaction.error_none_held_on", vars: { reference, date } };
    case "CascadingOversell":
      return { key: "transaction.error_later_left_short" };
    default:
      return null;
  }
}

/**
 * TRF-056 — the line under a part of a transfer's type, naming the other account; null for a
 * row that is no part of a transfer or a part left alone.
 */
export function transferLineKey(row: TransactionRowViewModel): string | null {
  if (!row.transferAccount) return null;
  switch (row.type) {
    case "TransferOut":
      return "transfer.to_account";
    case "TransferIn":
      return "transfer.from_account";
    case "WriteOff":
      return "transfer.part_of_transfer_to";
    default:
      return null;
  }
}

/** TRF-057 / TXL-040 — the confirmation of a deletion: a part of a transfer removes it whole. */
export function deleteConfirmKey(row: TransactionRowViewModel | undefined): string {
  return row?.transferAccount
    ? "transfer.delete_confirm_message"
    : "transaction.delete_confirm_message";
}
