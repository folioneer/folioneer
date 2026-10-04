import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  TransactionEntryMode,
  TransactionFormData,
} from "@/features/transactions/shared/types";
import { useTransactions } from "@/features/transactions/useTransactions";
import { getAutoRecordPrice } from "@/lib/autoRecordPriceStorage";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { microToExactDecimal, microToFieldDecimal, microToFormatted } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { type DraftField, useDraftProblemDisplay } from "../shared/useDraftProblemDisplay";
import { useHoldingSnapshotAsOf } from "../shared/useHoldingSnapshotAsOf";
import { toTransactionDraft, useTransactionDraftCheck } from "../shared/useTransactionDraftCheck";

interface UseSellTransactionProps {
  accountId: string;
  assetId: string;
  /** Holding quantity in micro-units — shown as the maximum sellable quantity (SEL-022). */
  holdingQuantityMicro: number;
  onSubmitSuccess?: () => void;
}

export function useSellTransaction({
  accountId,
  assetId,
  holdingQuantityMicro,
  onSubmitSuccess,
}: UseSellTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const { sellHolding } = useTransactions();

  const [formData, setFormData] = useState<TransactionFormData>(() => ({
    accountId,
    assetId,
    date: getLastOperationDate(accountId),
    quantity: "",
    unitPrice: "",
    exchangeRate: "1.000000",
    fees: "0",
    note: "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  // MKT-052/053 — snapshot of the global auto-record toggle at hook mount
  const [recordPrice, setRecordPrice] = useState<boolean>(() => getAutoRecordPrice());
  // SEL-050 — entry mode: unit price typed (default) or all-in net proceeds typed
  const [entryMode, setEntryMode] = useState<TransactionEntryMode>("price");
  const [totalAmountInput, setTotalAmountInput] = useState("");

  // TRX-063 — the draft check decides whether the sale can be saved (oversell included,
  // SEL-021), and returns the unit price and net proceeds it would record (SEL-023, SEL-050).
  const draft = useMemo(
    () => toTransactionDraft("Sell", formData, entryMode, totalAmountInput),
    [formData, entryMode, totalAmountInput],
  );
  const check = useTransactionDraftCheck(draft);
  const preview = check.preview;

  // TRX-067 — the first problem is shown on its field once typed in, as a hint before.
  const problemDisplay = useDraftProblemDisplay(check.problem, check.problemMessage, entryMode);
  const touch = problemDisplay.touch;

  // TDI-020 — average cost as of the entered sell date (or today). Hidden when
  // nothing is held as of that date (TDI-021).
  const { snapshot } = useHoldingSnapshotAsOf(accountId, assetId, formData.date);
  const averageCostAsOfDate = useMemo(
    () => (snapshot && snapshot.quantity > 0 ? microToFormatted(snapshot.average_price) : null),
    [snapshot],
  );

  // TDI-030/031 — the gain the typed sale would realize is the core's: the draft check
  // returns it, or none when it cannot be computed.
  const potentialPnl = useMemo(() => {
    const pnlMicro = preview?.realized_pnl ?? null;
    return pnlMicro === null ? null : { formatted: microToFormatted(pnlMicro), raw: pnlMicro };
  }, [preview]);

  const handleChange = useCallback(
    (field: keyof TransactionFormData, value: string) => {
      setFormData((prev) => ({ ...prev, [field]: value }));
      const typed: Partial<Record<keyof TransactionFormData, DraftField>> = {
        date: "date",
        quantity: "quantity",
        unitPrice: "unitPrice",
        exchangeRate: "exchangeRate",
        fees: "fees",
      };
      const draftField = typed[field];
      if (draftField) touch(draftField);
    },
    [touch],
  );

  const handleTotalAmountChange = useCallback(
    (value: string) => {
      setTotalAmountInput(value);
      touch("total");
    },
    [touch],
  );

  // SEL-050 — switching modes carries over what the user currently sees: price →
  // total seeds the total input from the computed net proceeds (when qty + price
  // are valid); total → price seeds the unit-price field from the derived price
  // (when derivable). Otherwise the target field keeps its previous content.
  const handleEntryModeChange = useCallback(
    (mode: TransactionEntryMode) => {
      if (mode === entryMode) return;
      if (mode === "total") {
        if (preview && preview.unit_price > 0) {
          setTotalAmountInput(microToFieldDecimal(preview.total_amount));
        }
      } else if (preview && preview.unit_price > 0) {
        setFormData((prev) => ({ ...prev, unitPrice: microToFieldDecimal(preview.unit_price) }));
      }
      setEntryMode(mode);
    },
    [entryMode, preview],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();

      if (!preview) {
        setError(check.problemMessage);
        return;
      }

      setError(null);
      setIsSubmitting(true);

      try {
        const result = await sellHolding({
          account_id: formData.accountId,
          asset_id: formData.assetId,
          date: formData.date,
          quantity: draft.quantity,
          unit_price: preview.unit_price,
          exchange_rate: draft.entered.exchange_rate,
          fees: draft.entered.fees,
          // SEL-050 — total mode ships the typed net proceeds; the backend derives the unit price
          total_amount: draft.entered.mode === "Total" ? draft.entered.total : null,
          note: formData.note || null,
        });

        if (result.error) {
          setError(result.error);
          return;
        }

        // MKT-055/061 — record price separately when auto-record is on and price is non-zero (best-effort)
        if (recordPrice && preview.unit_price > 0) {
          accountDetailsGateway
            .recordAssetPrice(
              formData.assetId,
              formData.date,
              parseFloat(microToExactDecimal(preview.unit_price)),
            )
            .catch((e) =>
              logger.warn("Failed to record asset price after sell", {
                error: e,
              }),
            );
        }

        setLastOperationDate(formData.accountId, formData.date);
        showSnackbar(t("transaction.success_sell_created"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      formData,
      draft,
      preview,
      check.problemMessage,
      recordPrice,
      sellHolding,
      t,
      showSnackbar,
      onSubmitSuccess,
    ],
  );

  return {
    formData,
    /** Sell total proceeds in micro-units formatted for display (SEL-023, read-only). */
    totalAmountDisplay: microToFormatted(preview?.total_amount ?? 0),
    /** Maximum sellable quantity formatted for display (SEL-022). */
    maxQuantityDisplay: microToFormatted(holdingQuantityMicro, 6),
    /** SEL-050 — how the money side is entered; resets with the modal (not persisted). */
    entryMode,
    setEntryMode: handleEntryModeChange,
    /** SEL-050 — the typed all-in net proceeds (decimal string), only meaningful in total mode. */
    totalAmountInput,
    handleTotalAmountChange,
    /** TRX-067 — the first problem as an error on the field it concerns, once typed in. */
    fieldErrors: problemDisplay.fieldErrors,
    /** TRX-067 — what to enter, for a field not typed in yet. */
    problemHint: problemDisplay.hint,
    /** SEL-050 — formatted derived unit price shown in total mode; "—" until the draft checks clean. */
    unitPriceDisplay: preview ? microToFormatted(preview.unit_price) : "—",
    /** TDI-020 — formatted account-currency average cost as of the date, or null when not held. */
    averageCostAsOfDate,
    /** TDI-030 — potential realized P&L of the typed sell (`{ formatted, raw }`), or null. */
    potentialPnl,
    error: error ?? problemDisplay.alert,
    isSubmitting,
    isFormValid: check.isClean,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
  };
}
