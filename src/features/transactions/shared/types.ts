import type { CorrectTransactionDTO } from "@/bindings";

/**
 * The user-editable fields of a transaction correction. The gateway injects
 * `account_id` + `transaction_id` (from the existing-transaction edit context)
 * into the wire DTO before calling the command, so callers supply only these.
 */
export type CorrectTransactionFields = Omit<CorrectTransactionDTO, "account_id" | "transaction_id">;

/**
 * How the user enters the money side of a buy / sell (TRX-060, SEL-050):
 * `price` — unit price is typed, total is computed; `total` — the all-in
 * total is typed, unit price is derived. Not persisted — resets per modal.
 */
export type TransactionEntryMode = "price" | "total";

/**
 * Form state for a transaction (add or edit).
 * All numeric fields are decimal strings — converted to i64 micro-units at submit (TRX-024).
 */
export interface TransactionFormData {
  /** Account where the transaction occurs. */
  accountId: string;
  /** Financial asset involved. */
  assetId: string;
  /** Transaction date in YYYY-MM-DD format. */
  date: string;
  /** Quantity as a decimal string (e.g. "1.5"). */
  quantity: string;
  /** Unit price as a decimal string in the asset's native currency. */
  unitPrice: string;
  /** Exchange rate as a decimal string (default "1.000000"). */
  exchangeRate: string;
  /** Transaction fees as a decimal string in the account currency. */
  fees: string;
  /** Optional user note. */
  note: string;
}
