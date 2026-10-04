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
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { type DraftField, useDraftProblemDisplay } from "../shared/useDraftProblemDisplay";
import { useHoldingSnapshotAsOf } from "../shared/useHoldingSnapshotAsOf";
import { toTransactionDraft, useTransactionDraftCheck } from "../shared/useTransactionDraftCheck";

interface UseBuyTransactionProps {
  accountId: string;
  assetId: string;
  onSubmitSuccess?: () => void;
}

export function useBuyTransaction({ accountId, assetId, onSubmitSuccess }: UseBuyTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const { buyHolding } = useTransactions();
  const assets = useAppStore((state) => state.assets);

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
  const [showArchivedConfirm, setShowArchivedConfirm] = useState(false);
  // MKT-052/053 — snapshot of the global auto-record toggle at hook mount
  const [recordPrice, setRecordPrice] = useState<boolean>(() => getAutoRecordPrice());
  // TRX-060 — entry mode: unit price typed (default) or all-in total typed
  const [entryMode, setEntryMode] = useState<TransactionEntryMode>("price");
  const [totalAmountInput, setTotalAmountInput] = useState("");

  // TRX-063 — the draft check decides whether the purchase can be saved, and returns the
  // unit price and total it would record (TRX-026, TRX-060).
  const draft = useMemo(
    () => toTransactionDraft("Purchase", formData, entryMode, totalAmountInput),
    [formData, entryMode, totalAmountInput],
  );
  const check = useTransactionDraftCheck(draft);
  const preview = check.preview;

  // TRX-067 — the first problem is shown on its field once typed in, as a hint before.
  const problemDisplay = useDraftProblemDisplay(check.problem, check.problemMessage, entryMode);
  const touch = problemDisplay.touch;

  // TDI-020 — average cost as of the entered trade date (or today). Hidden when
  // nothing is held as of that date (TDI-021).
  const { snapshot } = useHoldingSnapshotAsOf(accountId, assetId, formData.date);
  const averageCostAsOfDate = useMemo(
    () => (snapshot && snapshot.quantity > 0 ? microToFormatted(snapshot.average_price) : null),
    [snapshot],
  );

  // TRX-029 — is the pre-determined asset archived?
  const isAssetArchived = useMemo(
    () => assets.find((a) => a.id === assetId)?.is_archived ?? false,
    [assets, assetId],
  );

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

  // TRX-060 — switching modes carries over what the user currently sees: price →
  // total seeds the total input from the computed total (when qty + price are
  // valid); total → price seeds the unit-price field from the derived price
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

  const doSubmit = useCallback(async () => {
    if (!preview) {
      setError(check.problemMessage);
      return;
    }

    setError(null);
    setIsSubmitting(true);

    try {
      const result = await buyHolding({
        account_id: formData.accountId,
        asset_id: formData.assetId,
        date: formData.date,
        quantity: draft.quantity,
        unit_price: preview.unit_price,
        exchange_rate: draft.entered.exchange_rate,
        fees: draft.entered.fees,
        // TRX-060 — total mode ships the typed total; the backend derives the unit price
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
          .catch((e) => logger.warn("Failed to record asset price after buy", { error: e }));
      }

      setLastOperationDate(formData.accountId, formData.date);
      showSnackbar(t("transaction.success_created"), "success");
      onSubmitSuccess?.();
    } finally {
      setIsSubmitting(false);
    }
  }, [
    formData,
    draft,
    preview,
    check.problemMessage,
    recordPrice,
    buyHolding,
    t,
    showSnackbar,
    onSubmitSuccess,
  ]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (isAssetArchived) {
        setShowArchivedConfirm(true);
        return;
      }
      await doSubmit();
    },
    [isAssetArchived, doSubmit],
  );

  const handleConfirmArchived = useCallback(async () => {
    setShowArchivedConfirm(false);
    await doSubmit();
  }, [doSubmit]);

  const handleCancelArchived = useCallback(() => {
    setShowArchivedConfirm(false);
  }, []);

  return {
    formData,
    totalAmountDisplay: microToFormatted(preview?.total_amount ?? 0),
    /** TRX-060 — how the money side is entered; resets with the modal (not persisted). */
    entryMode,
    setEntryMode: handleEntryModeChange,
    /** TRX-060 — the typed all-in total (decimal string), only meaningful in total mode. */
    totalAmountInput,
    handleTotalAmountChange,
    /** TRX-067 — the first problem as an error on the field it concerns, once typed in. */
    fieldErrors: problemDisplay.fieldErrors,
    /** TRX-067 — the first problem as a plain hint, for a field not typed in yet. */
    problemHint: problemDisplay.hint,
    /** TRX-060 — formatted derived unit price shown in total mode; "—" until the draft checks clean. */
    unitPriceDisplay: preview ? microToFormatted(preview.unit_price) : "—",
    /** TDI-020 — formatted account-currency average cost as of the date, or null when not held. */
    averageCostAsOfDate,
    error: error ?? problemDisplay.alert,
    isSubmitting,
    isFormValid: check.isClean,
    showArchivedConfirm,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  };
}
