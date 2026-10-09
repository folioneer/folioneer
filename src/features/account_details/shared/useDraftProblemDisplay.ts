import { useMemo } from "react";
import type { TransactionDraftError } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { useProblemPlacement } from "@/ui/hooks/useProblemPlacement";

// The same file lives in `features/transactions/shared/`: a feature does not import another's
// (DEBT-008). A change here is made there too.

/** The fields of a transaction form a draft problem can concern. */
export type DraftField =
  | "account"
  | "asset"
  | "date"
  | "quantity"
  | "unitPrice"
  | "exchangeRate"
  | "fees"
  | "total"
  | "totalCost";

/** The field each problem of the draft check (TRX-062, TRX-066) is shown on. */
const FIELD_OF: Partial<Record<TransactionDraftError["code"], DraftField>> = {
  AccountMissing: "account",
  AssetMissing: "asset",
  DateMissing: "date",
  InvalidDate: "date",
  DateInFuture: "date",
  DateTooOld: "date",
  QuantityNotPositive: "quantity",
  Oversell: "quantity",
  UnitPriceNegative: "unitPrice",
  ExchangeRateNotPositive: "exchangeRate",
  FeesNegative: "fees",
  TotalAmountNotPositive: "total",
  TotalAmountBelowFees: "total",
  UnitPriceOutOfRange: "total",
  TotalCostMissing: "totalCost",
  InvalidTotalCost: "totalCost",
};

/** What to enter in a field the user has not typed in yet. */
const HINT_OF: Record<DraftField, string> = {
  account: "transaction.hint_select_account",
  asset: "transaction.hint_select_asset",
  date: "transaction.hint_enter_date",
  quantity: "transaction.hint_enter_quantity",
  unitPrice: "transaction.hint_enter_unit_price",
  exchangeRate: "transaction.hint_enter_exchange_rate",
  fees: "transaction.hint_enter_fees",
  total: "transaction.hint_enter_total",
  totalCost: "transaction.hint_enter_total_cost",
};

/**
 * The field a draft problem is shown on. While the unit price is typed the total is
 * computed, so a total that is not positive is the unit price's to fix.
 */
export function draftProblemField(
  problem: TransactionDraftError | null,
  entryMode: "price" | "total",
): DraftField | null {
  if (!problem) return null;
  if (entryMode === "price" && problem.code === "TotalAmountNotPositive") return "unitPrice";
  return FIELD_OF[problem.code] ?? null;
}

/** Where the first problem of a draft is shown (TRX-067). At most one of the three is set. */
export interface DraftProblemDisplay {
  /** The problem as an error on the field it concerns, once the user has typed in it. */
  fieldErrors: Partial<Record<DraftField, I18nMessage>>;
  /** What to enter, while the problem concerns a field the user has not typed in yet. */
  hint: I18nMessage | null;
  /** The problem as an error beside the actions: it concerns no field. */
  alert: I18nMessage | null;
  /** Records that the user typed in a field. */
  touch: (field: DraftField) => void;
}

/**
 * TRX-067 — a form never keeps saving disabled without a visible reason: the first problem
 * the core reports is an error on its field once the user has typed in it, a plain hint
 * saying what to enter until then, and an error beside the actions when it concerns no field.
 */
export function useDraftProblemDisplay(
  problem: TransactionDraftError | null,
  message: I18nMessage | null,
  entryMode: "price" | "total" = "price",
): DraftProblemDisplay {
  const placement = useProblemPlacement(draftProblemField(problem, entryMode), message);
  return useMemo(
    () => ({
      fieldErrors: placement.fieldErrors,
      hint: placement.untouchedField ? { key: HINT_OF[placement.untouchedField] } : null,
      alert: placement.alert,
      touch: placement.touch,
    }),
    [placement],
  );
}
