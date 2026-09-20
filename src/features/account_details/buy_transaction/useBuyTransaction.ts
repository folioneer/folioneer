import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  TransactionEntryMode,
  TransactionFormData,
} from "@/features/transactions/shared/types";
import { validateTransactionForm } from "@/features/transactions/shared/validateTransaction";
import { useTransactions } from "@/features/transactions/useTransactions";
import { getAutoRecordPrice } from "@/lib/autoRecordPriceStorage";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import {
  computeTotalMicro,
  decimalToMicro,
  deriveUnitPriceMicro,
  microToDecimal,
  microToFormatted,
} from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { useHoldingSnapshotAsOf } from "../shared/useHoldingSnapshotAsOf";

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

  const microValues = useMemo(() => {
    const qtyMicro = decimalToMicro(formData.quantity);
    const rateMicro = decimalToMicro(formData.exchangeRate);
    const feesMicro = decimalToMicro(formData.fees);
    if (entryMode === "total") {
      // TRX-060 — the typed total is ground truth; the unit price is derived
      const totalMicro = decimalToMicro(totalAmountInput);
      const priceMicro = deriveUnitPriceMicro(totalMicro, feesMicro, qtyMicro, rateMicro, false);
      return { qtyMicro, priceMicro, rateMicro, feesMicro, totalMicro };
    }
    const priceMicro = decimalToMicro(formData.unitPrice);
    const totalMicro = computeTotalMicro(qtyMicro, priceMicro, rateMicro, feesMicro);
    return { qtyMicro, priceMicro, rateMicro, feesMicro, totalMicro };
  }, [
    formData.quantity,
    formData.unitPrice,
    formData.exchangeRate,
    formData.fees,
    entryMode,
    totalAmountInput,
  ]);

  const totalEntryFeesMicro = entryMode === "total" ? microValues.feesMicro : null;

  const isFormValid = useMemo(
    () =>
      validateTransactionForm(
        formData,
        microValues.qtyMicro,
        microValues.totalMicro,
        totalEntryFeesMicro,
      ) === null,
    [formData, microValues.qtyMicro, microValues.totalMicro, totalEntryFeesMicro],
  );

  // TRX-060 — inline rejection on the total field: a typed all-in total cannot
  // be lower than the fees it includes. Submit stays disabled via isFormValid.
  const totalBelowFeesError = useMemo<I18nMessage | null>(
    () =>
      entryMode === "total" &&
      microValues.totalMicro > 0 &&
      microValues.feesMicro > 0 &&
      microValues.totalMicro < microValues.feesMicro
        ? { key: "transaction.error_validation_total_below_fees" }
        : null,
    [entryMode, microValues.totalMicro, microValues.feesMicro],
  );

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

  const handleChange = useCallback((field: keyof TransactionFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  // TRX-060 — switching modes carries over what the user currently sees: price →
  // total seeds the total input from the computed total (when qty + price are
  // valid); total → price seeds the unit-price field from the derived price
  // (when derivable). Otherwise the target field keeps its previous content.
  const handleEntryModeChange = useCallback(
    (mode: TransactionEntryMode) => {
      if (mode === entryMode) return;
      if (mode === "total") {
        if (microValues.qtyMicro > 0 && microValues.priceMicro > 0) {
          setTotalAmountInput(microToDecimal(microValues.totalMicro));
        }
      } else if (microValues.priceMicro > 0) {
        setFormData((prev) => ({ ...prev, unitPrice: microToDecimal(microValues.priceMicro) }));
      }
      setEntryMode(mode);
    },
    [entryMode, microValues],
  );

  const doSubmit = useCallback(async () => {
    const validationError = validateTransactionForm(
      formData,
      microValues.qtyMicro,
      microValues.totalMicro,
      totalEntryFeesMicro,
    );
    if (validationError) {
      setError(validationError);
      return;
    }

    setError(null);
    setIsSubmitting(true);

    try {
      const result = await buyHolding({
        account_id: formData.accountId,
        asset_id: formData.assetId,
        date: formData.date,
        quantity: microValues.qtyMicro,
        unit_price: microValues.priceMicro,
        exchange_rate: microValues.rateMicro,
        fees: microValues.feesMicro,
        // TRX-060 — total mode ships the typed total; the backend re-derives
        // the authoritative unit price from it (priceMicro mirrors that formula)
        total_amount: entryMode === "total" ? microValues.totalMicro : null,
        note: formData.note || null,
      });

      if (result.error) {
        setError(result.error);
        return;
      }

      // MKT-055/061 — record price separately when auto-record is on and price is non-zero (best-effort)
      if (recordPrice && microValues.priceMicro > 0) {
        accountDetailsGateway
          .recordAssetPrice(
            formData.assetId,
            formData.date,
            parseFloat(microToDecimal(microValues.priceMicro)),
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
    microValues,
    entryMode,
    totalEntryFeesMicro,
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
    totalAmountDisplay: microToFormatted(microValues.totalMicro),
    /** TRX-060 — how the money side is entered; resets with the modal (not persisted). */
    entryMode,
    setEntryMode: handleEntryModeChange,
    /** TRX-060 — the typed all-in total (decimal string), only meaningful in total mode. */
    totalAmountInput,
    handleTotalAmountChange: setTotalAmountInput,
    /** TRX-060 — inline error for the Total field when the typed total is below the fees. */
    totalBelowFeesError,
    /** TRX-060 — formatted derived unit price shown in total mode; "—" when quantity is 0. */
    unitPriceDisplay: microValues.qtyMicro > 0 ? microToFormatted(microValues.priceMicro) : "—",
    /** TDI-020 — formatted account-currency average cost as of the date, or null when not held. */
    averageCostAsOfDate,
    error,
    isSubmitting,
    isFormValid,
    showArchivedConfirm,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  };
}
