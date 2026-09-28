import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Transaction } from "@/bindings";
import { logger } from "@/lib/logger";
import {
  computeTotalMicro,
  decimalToMicro,
  microToDecimal,
  microToFormatted,
} from "@/lib/microUnits";
import { useAppStore } from "@/lib/store";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { transactionGateway } from "../gateway";
import type { TransactionEntryMode, TransactionFormData } from "../shared/types";
import { toTransactionDraft, useTransactionDraftCheck } from "../shared/useTransactionDraftCheck";
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

  // TRX-063 — a purchase or sale correction follows the draft check, which returns the unit
  // price and total it would record; a corrected sale is not checked for oversell (SEL-030).
  const draft = useMemo(
    () =>
      isTotalEntryEligible
        ? toTransactionDraft(
            isSell ? "Sell" : "Purchase",
            formData,
            isTotalMode ? "total" : "price",
            totalAmountInput,
            transaction.id,
          )
        : null,
    [isTotalEntryEligible, formData, isSell, isTotalMode, totalAmountInput, transaction.id],
  );
  const check = useTransactionDraftCheck(draft);
  const preview = check.preview;

  // TRX-063 — every other type (an opening balance, a dividend) is checked on save: what the
  // user typed is sent as entered and recording's rejection is shown.
  const entered = useMemo(
    () => ({
      qtyMicro: decimalToMicro(formData.quantity),
      priceMicro: decimalToMicro(formData.unitPrice),
      rateMicro: isOpeningBalance ? 1_000_000 : decimalToMicro(formData.exchangeRate),
      feesMicro: isOpeningBalance ? 0 : decimalToMicro(formData.fees),
    }),
    [formData, isOpeningBalance],
  );
  // reviewer-frontend FP: no numeric check here — recording rejects a non-positive figure on
  // save (TRX-063, F32), and an opening balance may cost zero (TRX-045) — see PR #53
  const isFormValid = isTotalEntryEligible
    ? check.isClean
    : Boolean(formData.date && formData.quantity && formData.unitPrice);
  // TD-054 — a dividend correction still previews its total in the interface.
  // TRX-051 — an opening balance's amount field holds its total cost, shown as typed.
  const totalMicro = isTotalEntryEligible
    ? (preview?.total_amount ?? 0)
    : isOpeningBalance
      ? entered.priceMicro
      : computeTotalMicro(
          entered.qtyMicro,
          entered.priceMicro,
          entered.rateMicro,
          entered.feesMicro,
        );

  // TRX-060 — a typed purchase total below the fees it includes is shown on the Total field.
  const totalBelowFeesError = useMemo<I18nMessage | null>(
    () =>
      isTotalEntryEligible && check.problem?.code === "TotalAmountBelowFees"
        ? { key: "transaction.error_validation_total_below_fees" }
        : null,
    [isTotalEntryEligible, check.problem],
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
        if (preview && preview.unit_price > 0) {
          setTotalAmountInput(microToDecimal(preview.total_amount));
        }
      } else if (preview && preview.unit_price > 0) {
        setFormData((prev) => ({ ...prev, unitPrice: microToDecimal(preview.unit_price) }));
      }
      setEntryMode(mode);
    },
    [entryMode, preview],
  );

  const doSubmit = useCallback(async () => {
    if (isTotalEntryEligible && !preview) {
      setError(check.problemMessage);
      return;
    }

    setError(null);
    setIsSubmitting(true);

    try {
      const result = await correctTransaction(transaction.id, transaction.account_id, {
        date: formData.date,
        quantity: entered.qtyMicro,
        unit_price: isOpeningBalance ? 0 : (preview?.unit_price ?? entered.priceMicro),
        exchange_rate: entered.rateMicro,
        fees: entered.feesMicro,
        // TRX-051 / TRX-061 / SEL-051 — an opening balance's total cost, or a total
        // typed in total mode, is sent as is; the backend derives the unit price.
        total_amount: isOpeningBalance
          ? entered.priceMicro
          : draft && draft.entered.mode === "Total"
            ? draft.entered.total
            : null,
        note: isOpeningBalance ? null : formData.note || null,
      });

      if (result.error) {
        setError(result.error);
        return;
      }

      // MKT-055/061 — record price separately when opt-in is on and price is non-zero (best-effort)
      const recordedPrice = preview?.unit_price ?? entered.priceMicro;
      if (recordPrice && recordedPrice > 0) {
        transactionGateway
          .recordAssetPrice(
            transaction.asset_id,
            formData.date,
            parseFloat(microToDecimal(recordedPrice)),
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
    draft,
    entered,
    preview,
    isTotalEntryEligible,
    check.problemMessage,
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
    totalAmountDisplay: microToFormatted(totalMicro),
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
    unitPriceDisplay: preview ? microToFormatted(preview.unit_price) : "—",
    handleChange,
    handleSubmit,
    handleConfirmArchived,
    handleCancelArchived,
  };
}
