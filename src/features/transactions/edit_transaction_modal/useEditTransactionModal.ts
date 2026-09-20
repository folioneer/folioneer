import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import {
  computeSellTotalMicro,
  computeTotalMicro,
  decimalToMicro,
  deriveUnitPriceMicro,
  microToDecimal,
  microToFormatted,
} from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { transactionGateway } from "../gateway";
import type { TransactionEntryMode, TransactionFormData } from "../shared/types";
import { validateTransactionForm } from "../shared/validateTransaction";
import { useTransactions } from "../useTransactions";

interface UseEditTransactionModalProps {
  transaction: Transaction;
  onSubmitSuccess?: () => void;
}

/**
 * Populates the form from an existing Transaction (micro-units → decimal strings)
 * and submits via correctTransaction (TRX-031, TRX-033).
 */
export function useEditTransactionModal({
  transaction,
  onSubmitSuccess,
}: UseEditTransactionModalProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const { correctTransaction } = useTransactions();
  const assets = useAppStore((state) => state.assets);

  const isOpeningBalance = transaction.transaction_type === "OpeningBalance";
  const isSell = transaction.transaction_type === "Sell";
  // TRX-061 / SEL-051 — total-entry correction is offered only for the two
  // securities trades whose total decomposes into a derived unit price.
  const isTotalEntryEligible = transaction.transaction_type === "Purchase" || isSell;

  const [formData, setFormData] = useState<TransactionFormData>(() => ({
    accountId: transaction.account_id,
    assetId: transaction.asset_id,
    date: transaction.date,
    // TRX-051: for OpeningBalance, unitPrice field is repurposed to hold the total cost
    quantity: microToDecimal(transaction.quantity),
    unitPrice: isOpeningBalance
      ? microToDecimal(transaction.total_amount)
      : microToDecimal(transaction.unit_price),
    exchangeRate: microToDecimal(transaction.exchange_rate),
    fees: microToDecimal(transaction.fees),
    note: transaction.note ?? "",
  }));

  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [showArchivedConfirm, setShowArchivedConfirm] = useState(false);
  // MKT-052 — edit mode always starts OFF, regardless of the global toggle.
  // The user can manually opt in per-edit; the prior price record is independent (MKT-059).
  const [recordPrice, setRecordPrice] = useState<boolean>(false);
  // TRX-061 / SEL-051 — entry mode: unit price typed (default) or all-in total typed.
  const [entryMode, setEntryMode] = useState<TransactionEntryMode>("price");
  const [totalAmountInput, setTotalAmountInput] = useState("");

  const isTotalMode = isTotalEntryEligible && entryMode === "total";

  // Derive micro-unit values from form strings — single conversion at the input boundary (ADR-001).
  // TRX-051: for OpeningBalance, priceMicro holds total cost; totalMicro = priceMicro directly.
  // Use sell formula when editing a Sell transaction (SEL-023).
  const microValues = useMemo(() => {
    const qtyMicro = decimalToMicro(formData.quantity);
    const priceMicro = decimalToMicro(formData.unitPrice);
    if (isOpeningBalance) {
      return {
        qtyMicro,
        priceMicro,
        rateMicro: 1_000_000,
        feesMicro: 0,
        totalMicro: priceMicro,
      };
    }
    const rateMicro = decimalToMicro(formData.exchangeRate);
    const feesMicro = decimalToMicro(formData.fees);
    if (isTotalMode) {
      // TRX-061 / SEL-051 — the typed total is ground truth; the unit price is
      // derived from it (priceMicro mirrors the backend re-derivation).
      const totalMicro = decimalToMicro(totalAmountInput);
      const derivedPriceMicro = deriveUnitPriceMicro(
        totalMicro,
        feesMicro,
        qtyMicro,
        rateMicro,
        isSell,
      );
      return { qtyMicro, priceMicro: derivedPriceMicro, rateMicro, feesMicro, totalMicro };
    }
    const totalMicro = isSell
      ? computeSellTotalMicro(qtyMicro, priceMicro, rateMicro, feesMicro)
      : computeTotalMicro(qtyMicro, priceMicro, rateMicro, feesMicro);
    return { qtyMicro, priceMicro, rateMicro, feesMicro, totalMicro };
  }, [
    formData.quantity,
    formData.unitPrice,
    formData.exchangeRate,
    formData.fees,
    isOpeningBalance,
    isSell,
    isTotalMode,
    totalAmountInput,
  ]);

  // TRX-060/061 — a typed purchase total must cover the fees it includes; a sell
  // total is net proceeds, so the fees-floor check does not apply to sells.
  const totalEntryFeesMicro = isTotalMode && !isSell ? microValues.feesMicro : null;

  // Derived form validity
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

  // TRX-060 — inline rejection on the total field for a purchase: a typed all-in
  // total cannot be lower than the fees it includes. Submit stays disabled via
  // isFormValid. Sells have no fees floor (the total is already net).
  const totalBelowFeesError = useMemo<I18nMessage | null>(
    () =>
      isTotalMode &&
      !isSell &&
      microValues.totalMicro > 0 &&
      microValues.feesMicro > 0 &&
      microValues.totalMicro < microValues.feesMicro
        ? { key: "transaction.error_validation_total_below_fees" }
        : null,
    [isTotalMode, isSell, microValues.totalMicro, microValues.feesMicro],
  );

  // TRX-029 — derived flag: is the currently selected asset archived?
  const isSelectedAssetArchived = formData.assetId
    ? (assets.find((a) => a.id === formData.assetId)?.is_archived ?? false)
    : false;

  const handleChange = useCallback((field: keyof TransactionFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handleTotalAmountChange = useCallback((value: string) => {
    setTotalAmountInput(value);
  }, []);

  // TRX-061 — switching modes carries over what the user currently sees: price →
  // total seeds the total input from the computed total; total → price seeds the
  // unit-price field from the derived price. Otherwise the target keeps its content.
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
      // TRX-051: for OpeningBalance, compute unit_price = total_cost / quantity (TRX-047 formula)
      const unitPriceMicro =
        isOpeningBalance && microValues.qtyMicro > 0
          ? Math.floor((microValues.priceMicro * 1_000_000) / microValues.qtyMicro)
          : microValues.priceMicro;

      const result = await correctTransaction(transaction.id, transaction.account_id, {
        date: formData.date,
        quantity: microValues.qtyMicro,
        unit_price: unitPriceMicro,
        exchange_rate: microValues.rateMicro,
        fees: microValues.feesMicro,
        // TRX-061 / SEL-051 — total mode ships the typed total; the backend
        // re-derives the authoritative unit price from it.
        total_amount: isTotalMode ? microValues.totalMicro : null,
        note: isOpeningBalance ? null : formData.note || null,
      });

      if (result.error) {
        setError(result.error);
        return;
      }

      // MKT-055/061 — record price separately when opt-in is on and price is non-zero (best-effort)
      if (recordPrice && microValues.priceMicro > 0) {
        transactionGateway
          .recordAssetPrice(
            transaction.asset_id,
            formData.date,
            parseFloat(microToDecimal(microValues.priceMicro)),
          )
          .catch((e) =>
            logger.warn("Failed to record asset price after correction", {
              error: e,
            }),
          );
      }

      showSnackbar(t("transaction.success_updated"), "success");
      onSubmitSuccess?.();
    } finally {
      setIsSubmitting(false);
    }
  }, [
    formData,
    microValues,
    totalEntryFeesMicro,
    isTotalMode,
    recordPrice,
    isOpeningBalance,
    correctTransaction,
    transaction.id,
    transaction.account_id,
    transaction.asset_id,
    t,
    onSubmitSuccess,
    showSnackbar,
  ]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (isSelectedAssetArchived) {
        // TRX-029 — show confirmation before submitting with an archived asset
        setShowArchivedConfirm(true);
        return;
      }
      await doSubmit();
    },
    [isSelectedAssetArchived, doSubmit],
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
    /** Total amount formatted for display. For OpeningBalance, equals total cost (TRX-051). */
    totalAmountDisplay: microToFormatted(microValues.totalMicro),
    error,
    isSubmitting,
    isFormValid,
    showArchivedConfirm,
    recordPrice,
    setRecordPrice,
    // TRX-061 / SEL-051 — total-entry correction (Purchase / Sell only).
    isTotalEntryEligible,
    isTotalMode,
    entryMode,
    handleEntryModeChange,
    totalAmountInput,
    handleTotalAmountChange,
    totalBelowFeesError,
    /** Derived unit price shown read-only while in total-entry mode. */
    unitPriceDisplay: microValues.qtyMicro > 0 ? microToFormatted(microValues.priceMicro) : "—",
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  };
}
