import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { StockSplitDraft } from "@/bindings";
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
import { useLatestCheck } from "@/ui/hooks/useLatestCheck";
import { accountDetailsGateway } from "../gateway";
import { splitErrorToI18n, transactionDraftErrorToI18n } from "../shared/presenter";
import type { SplitTarget } from "../shared/types";

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

/** Read-only preview of the rescaled position, formatted from the core's answer (SPL-062). */
export interface SplitPreview {
  oldQuantity: string;
  oldAveragePrice: string;
  newQuantity: string;
  newAveragePrice: string;
}

/** A typed whole number, or null while the field holds anything else. */
function typedInteger(value: string): number | null {
  return /^\d+$/.test(value) ? Number(value) : null;
}

const SPLIT_SIZE_PROBLEMS = new Set(["SplitFactorNotPositive", "SplitFactorIsOne"]);
const CHECK_FAILED: I18nMessage = { key: "error.Unknown" };

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

  // SPL-062 — the core checks the split as it is typed and returns the factor, what it
  // makes of the position and the price to carry across it. The form computes none of
  // them: it sends the ratio as typed (create) or the factor (correction, SPL-030).
  const draft = useMemo<StockSplitDraft>(
    () => ({
      account_id: accountId,
      asset_id: target.assetId,
      date: formData.date,
      size: isEditMode
        ? { mode: "Factor", factor: decimalToMicro(formData.factor) }
        : {
            mode: "Ratio",
            new: typedInteger(formData.ratioNew),
            old: typedInteger(formData.ratioOld),
          },
      correcting: editMode?.transactionId ?? null,
    }),
    [
      accountId,
      target.assetId,
      formData.date,
      formData.ratioNew,
      formData.ratioOld,
      formData.factor,
      isEditMode,
      editMode?.transactionId,
    ],
  );
  const logFailure = useCallback(
    (cause: unknown) => logger.error("Failed to check the split draft", { error: cause }),
    [],
  );
  const check = useLatestCheck(draft, accountDetailsGateway.validateStockSplitDraft, logFailure);
  const problemCode = check.error?.code ?? null;

  // SPL-011 — the factor must be strictly positive and different from ×1: the core says.
  const ratioError = useMemo<I18nMessage | null>(
    () =>
      problemCode !== null && SPLIT_SIZE_PROBLEMS.has(problemCode)
        ? { key: "transaction.error_validation_split_ratio" }
        : null,
    [problemCode],
  );

  // Any other problem — a date, a position not held then, a split that leaves nothing
  // (SPL-021), a check that could not run — is stated.
  const problemMessage = useMemo<I18nMessage | null>(() => {
    if (check.failed) return CHECK_FAILED;
    if (check.error === null || ratioError !== null) return null;
    return transactionDraftErrorToI18n(check.error);
  }, [check.failed, check.error, ratioError]);

  // SPL-061 — read-only preview of the rescaled position, as the core returns it.
  const position = check.data?.position ?? null;
  const preview = useMemo<SplitPreview | null>(
    () =>
      position === null
        ? null
        : {
            oldQuantity: microToFormattedQuantity(position.old_quantity),
            oldAveragePrice: microToFormattedPrice(position.old_average_price),
            newQuantity: microToFormattedQuantity(position.new_quantity),
            newAveragePrice: microToFormattedPrice(position.new_average_price),
          },
    [position],
  );

  // SPL-040 — the post-split price prefill is the core's: the latest price carried
  // across the split.
  const priceAfterSplit = check.data?.price_after_split ?? null;
  const priceInput =
    priceOverride ?? (priceAfterSplit === null ? "" : microToDecimal(priceAfterSplit));

  const isFormValid = check.data !== null;
  const factorMicro = check.data?.factor ?? null;

  const handleChange = useCallback((field: keyof SplitFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  const handlePriceChange = useCallback((value: string) => {
    setPriceOverride(value);
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      // Saving follows the check: without a clean answer there is nothing to record.
      if (factorMicro === null) return;

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
    ratioError,
    error: error ?? problemMessage,
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
