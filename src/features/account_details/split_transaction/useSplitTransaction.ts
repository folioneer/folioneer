import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import {
  decimalToMicro,
  microToDecimal,
  microToFormattedPrice,
  microToFormattedQuantity,
} from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { splitErrorToI18n } from "../shared/presenter";
import type { SplitTarget } from "../shared/types";
import { validateDate } from "../shared/validateCashForm";

const MICRO = 1_000_000;

/** Edit-mode context (SPL-030): the split being corrected; the asset is immutable. */
export interface SplitEditMode {
  transactionId: string;
  lockedAssetId: string;
  lockedAssetName: string;
  /** Current values to prefill the form when correcting an existing split. */
  initialDate?: string;
  /** Factor as a decimal multiplier string ("2.000" for a 2-for-1 split). */
  initialFactor?: string;
  initialNote?: string;
}

interface UseSplitTransactionProps {
  accountId: string;
  target: SplitTarget;
  onSubmitSuccess?: () => void;
  editMode?: SplitEditMode;
}

interface SplitFormData {
  date: string;
  /** Create mode — the "new" side of the new : old ratio (positive integer, SPL-061). */
  ratioNew: string;
  /** Create mode — the "old" side of the new : old ratio (positive integer, SPL-061). */
  ratioOld: string;
  /** Edit mode — the factor as a decimal multiplier (SPL-030). */
  factor: string;
  note: string;
}

/** Read-only preview of the rescaled position (SPL-061 / SPL-020 formulas). */
export interface SplitPreview {
  oldQuantity: string;
  oldAveragePrice: string;
  newQuantity: string;
  newAveragePrice: string;
  /** Raw rescaled quantity — 0 means the split collapses the position (SPL-021). */
  newQuantityMicro: number;
}

function parsePositiveInteger(value: string): number | null {
  if (!/^\d+$/.test(value)) return null;
  const parsed = Number(value);
  return parsed > 0 ? parsed : null;
}

export function useSplitTransaction({
  accountId,
  target,
  onSubmitSuccess,
  editMode,
}: UseSplitTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const isEditMode = editMode != null;
  const hasCurrentPrice = target.currentPriceMicro !== null;

  const [formData, setFormData] = useState<SplitFormData>(() => ({
    date: editMode?.initialDate ?? getLastOperationDate(accountId),
    ratioNew: "2",
    ratioOld: "1",
    factor: editMode?.initialFactor ?? "",
    note: editMode?.initialNote ?? "",
  }));
  // SPL-040 — checked by default when a prior price exists; unchecked (and the
  // derived field empty) when none does. Absent in edit mode.
  const [recordPrice, setRecordPrice] = useState(!isEditMode && hasCurrentPrice);
  // The price field is a derived prefill until the user types an explicit value.
  const [priceOverride, setPriceOverride] = useState<string | null>(null);
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // SPL-061 — micro-scaled factor: round(new × MICRO / old) from the ratio pair,
  // or the decimal factor input converted at the input boundary in edit mode
  // (SPL-030; decimalToMicro normalises the fr comma like every CalcField).
  const factorMicro = useMemo<number | null>(() => {
    if (isEditMode) {
      const micro = decimalToMicro(formData.factor);
      return micro > 0 ? micro : null;
    }
    const newPart = parsePositiveInteger(formData.ratioNew);
    const oldPart = parsePositiveInteger(formData.ratioOld);
    if (newPart === null || oldPart === null) return null;
    const factor = Math.round((newPart * MICRO) / oldPart);
    return Number.isSafeInteger(factor) ? factor : null;
  }, [isEditMode, formData.factor, formData.ratioNew, formData.ratioOld]);

  // SPL-011 — the factor must be strictly positive and different from ×1.
  const ratioError = useMemo<I18nMessage | null>(
    () =>
      factorMicro === null || factorMicro <= 0 || factorMicro === MICRO
        ? { key: "transaction.error_validation_split_ratio" }
        : null,
    [factorMicro],
  );

  // SPL-061 — read-only preview of the rescaled position, mirroring the backend
  // SPL-020 formulas: quantity ← floor(quantity × factor / MICRO), then the new
  // average derives from the preserved cost basis. Factoring `factorMicro / MICRO`
  // out keeps the intermediates below MAX_SAFE_INTEGER.
  const preview = useMemo<SplitPreview | null>(() => {
    if (isEditMode || factorMicro === null || factorMicro <= 0) return null;
    const newQuantityMicro = Math.floor(target.holdingQuantityMicro * (factorMicro / MICRO));
    const newAveragePriceMicro =
      newQuantityMicro > 0
        ? Math.round(target.averagePriceMicro * (target.holdingQuantityMicro / newQuantityMicro))
        : 0;
    return {
      oldQuantity: microToFormattedQuantity(target.holdingQuantityMicro),
      oldAveragePrice: microToFormattedPrice(target.averagePriceMicro),
      newQuantity: microToFormattedQuantity(newQuantityMicro),
      newAveragePrice: microToFormattedPrice(newAveragePriceMicro),
      newQuantityMicro,
    };
  }, [isEditMode, factorMicro, target.holdingQuantityMicro, target.averagePriceMicro]);

  // SPL-021 — a rescale that floors the quantity to zero is rejected upfront.
  const collapsesPosition = preview !== null && preview.newQuantityMicro === 0;

  // SPL-040 — derived post-split price prefill: round(latest price × MICRO / factor).
  const derivedPrice = useMemo(() => {
    if (target.currentPriceMicro === null || factorMicro === null || factorMicro <= 0) return "";
    return microToDecimal(Math.round(target.currentPriceMicro * (MICRO / factorMicro)));
  }, [target.currentPriceMicro, factorMicro]);
  const priceInput = priceOverride ?? derivedPrice;

  const isFormValid = useMemo(
    () => ratioError === null && !collapsesPosition && validateDate(formData.date) === null,
    [ratioError, collapsesPosition, formData.date],
  );

  const handleChange = useCallback((field: keyof SplitFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handlePriceChange = useCallback((value: string) => {
    setPriceOverride(value);
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const validationError = ratioError ?? validateDate(formData.date);
      if (validationError || factorMicro === null || collapsesPosition) {
        setError(validationError ?? { key: "transaction.error_validation_split_ratio" });
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        const note = formData.note || null;

        const result = editMode
          ? // SPL-030 — edit reuses correct_transaction; the factor rides in the
            // `quantity` field and the money fields carry the inert zero-cost
            // convention (unit_price 0, exchange_rate 1.0, fees 0, no total).
            await accountDetailsGateway.correctTransaction(editMode.transactionId, accountId, {
              date: formData.date,
              quantity: factorMicro,
              unit_price: 0,
              exchange_rate: 1_000_000,
              fees: 0,
              total_amount: null,
              note,
            })
          : await accountDetailsGateway.recordSplit({
              account_id: accountId,
              asset_id: target.assetId,
              date: formData.date,
              factor: factorMicro,
              note,
            });

        if (result.status === "error") {
          logger.error("[useSplitTransaction] recordSplit failed", { error: result.error });
          setError(splitErrorToI18n(result.error));
          return;
        }

        // SPL-040 — record the post-split price separately when the checkbox is
        // on and the price is positive (best-effort, like MKT-055).
        const price = parseFloat(priceInput);
        if (!editMode && recordPrice && Number.isFinite(price) && price > 0) {
          accountDetailsGateway
            .recordAssetPrice(target.assetId, formData.date, price)
            .catch((err) => logger.warn("Failed to record post-split asset price", { error: err }));
        }

        if (!editMode) setLastOperationDate(accountId, formData.date);
        showSnackbar(t(editMode ? "split.updated" : "split.recorded"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      accountId,
      target.assetId,
      formData,
      factorMicro,
      ratioError,
      collapsesPosition,
      recordPrice,
      priceInput,
      editMode,
      t,
      showSnackbar,
      onSubmitSuccess,
    ],
  );

  return {
    formData,
    preview,
    collapsesPosition,
    ratioError,
    error,
    isSubmitting,
    isFormValid,
    isEditMode,
    hasCurrentPrice,
    recordPrice,
    setRecordPrice,
    priceInput,
    handlePriceChange,
    handleChange,
    handleSubmit,
  };
}
