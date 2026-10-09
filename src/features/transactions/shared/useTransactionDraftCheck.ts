import { useCallback, useMemo } from "react";
import type {
  DraftKind,
  TransactionDraft,
  TransactionDraftError,
  TransactionPreview,
} from "@/bindings";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";
import { useLatestCheck } from "@/ui/hooks/useLatestCheck";
import { transactionGateway } from "../gateway";
import { transactionDraftErrorToI18n } from "./presenter";
import type { TransactionEntryMode, TransactionFormData } from "./types";

/** What the draft check (TRX-062) answered for the latest draft. */
export interface TransactionDraftCheck {
  /** The unit price and total recording would store; null until a draft checks clean. */
  preview: TransactionPreview | null;
  /** The draft's first problem, as the check reported it. */
  problem: TransactionDraftError | null;
  /** The first problem as a message, or a generic error when the check itself failed. */
  problemMessage: I18nMessage | null;
  /** TRX-063 — the latest draft checked without a problem: saving may be enabled. */
  isClean: boolean;
}

const CHECK_FAILED: I18nMessage = { key: "error.Unknown" };

/**
 * The draft of what the form holds, in micro-units (TRX-024): the typed total in total
 * mode, the unit price otherwise. `correcting` names the transaction a correction replaces.
 */
export function toTransactionDraft(
  kind: DraftKind,
  form: TransactionFormData,
  entryMode: TransactionEntryMode,
  totalAmountInput: string,
  correcting: string | null = null,
): TransactionDraft {
  const exchange_rate = decimalToMicro(form.exchangeRate);
  const fees = decimalToMicro(form.fees);
  return {
    kind,
    account_id: form.accountId,
    asset_id: form.assetId,
    date: form.date,
    quantity: decimalToMicro(form.quantity),
    entered:
      entryMode === "total"
        ? { mode: "Total", total: decimalToMicro(totalAmountInput), exchange_rate, fees }
        : { mode: "UnitPrice", unit_price: decimalToMicro(form.unitPrice), exchange_rate, fees },
    correcting,
  };
}

/**
 * TRX-063 — sends the draft to the check each time it changes and keeps only the answer
 * to the latest one. Each feature that records a trade has its own copy (DEBT-008). While a check runs, the draft is not clean. `null` checks nothing.
 */
export function useTransactionDraftCheck(draft: TransactionDraft | null): TransactionDraftCheck {
  const logFailure = useCallback(
    (cause: unknown) => logger.error("Failed to check the transaction draft", { error: cause }),
    [],
  );
  const answer = useLatestCheck(draft, transactionGateway.validateTransactionDraft, logFailure);
  return useMemo(
    () => ({
      preview: answer.data,
      problem: answer.error,
      problemMessage: answer.failed
        ? CHECK_FAILED
        : answer.error
          ? transactionDraftErrorToI18n(answer.error)
          : null,
      isClean: answer.data !== null,
    }),
    [answer],
  );
}
