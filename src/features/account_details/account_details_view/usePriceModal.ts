import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { assetPriceMutationErrorToI18n } from "../shared/presenter";
import type { PriceableAsset } from "../shared/types";

export interface UsePriceModalProps {
  /** Active non-cash holdings selectable in the asset combobox (MKT-011). */
  assets: PriceableAsset[];
  /** Asset pre-selected when the modal opens (the holding it was launched from). */
  initialAssetId: string;
  /** Account whose stored last-operation date seeds the date field (MKT-011). */
  accountId: string;
  onSubmitSuccess?: () => void;
  /**
   * Refresh-only callback for "record & add another" — keeps the modal open
   * (MKT-014). Receives the asset the price was recorded for, so the price
   * history list refreshes only when it matches the displayed asset.
   */
  onRecorded?: (assetId: string) => void;
}

export interface UsePriceModalResult {
  assetId: string;
  date: string;
  price: string;
  selectedCurrency: string;
  error: I18nMessage | null;
  isSubmitting: boolean;
  isFormValid: boolean;
  handleAssetChange: (assetId: string) => void;
  handleChange: (field: "date" | "price", value: string) => void;
  handleSubmit: (e: React.FormEvent) => Promise<void>;
  handleAddAnother: () => Promise<void>;
}

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

const today = () => new Date().toISOString().slice(0, 10);

function validatePrice(price: string): I18nMessage | null {
  const n = parseFloat(price);
  if (Number.isNaN(n) || n <= 0) return { key: "price_modal.error_price_not_positive" };
  return null;
}

function validateDate(date: string): I18nMessage | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return { key: "price_modal.error_invalid_date" };
  if (date > today()) return { key: "price_modal.error_future_date" };
  return null;
}

export function usePriceModal({
  assets,
  initialAssetId,
  accountId,
  onSubmitSuccess,
  onRecorded,
}: UsePriceModalProps): UsePriceModalResult {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const [assetId, setAssetId] = useState(initialAssetId);
  const [date, setDate] = useState(() => getLastOperationDate(accountId));
  const [price, setPrice] = useState("");
  const [submitError, setSubmitError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const selectedCurrency = useMemo(
    () => assets.find((a) => a.assetId === assetId)?.assetCurrency ?? "",
    [assets, assetId],
  );

  // Inline validation — only applied to non-empty values (MKT-021, MKT-022)
  const priceValidationError = price.length > 0 ? validatePrice(price) : null;
  const dateValidationError = date.length > 0 ? validateDate(date) : null;

  const isFormValid =
    assetId.length > 0 &&
    date.length > 0 &&
    price.length > 0 &&
    priceValidationError === null &&
    dateValidationError === null;

  // Display order: submit error then inline validation errors
  const error = submitError ?? priceValidationError ?? dateValidationError;

  // Switching the asset clears the price — a price belongs to one asset (MKT-011).
  const handleAssetChange = useCallback((nextAssetId: string) => {
    setSubmitError(null);
    setAssetId(nextAssetId);
    setPrice("");
  }, []);

  const handleChange = useCallback((field: "date" | "price", value: string) => {
    setSubmitError(null);
    if (field === "date") setDate(value);
    else setPrice(value);
  }, []);

  // Records the price; returns true on success. Shared by the primary submit and
  // the "add another" flow (MKT-014).
  const record = useCallback(async (): Promise<boolean> => {
    if (!isFormValid) return false;
    setIsSubmitting(true);
    try {
      const result = await accountDetailsGateway.recordAssetPrice(assetId, date, parseFloat(price));
      if (result.status === "ok") {
        // Remember the date so the next operation on this account pre-fills it,
        // consistent with every other operation hook (MKT-011).
        setLastOperationDate(accountId, date);
        showSnackbar(t("price_modal.success"));
        return true;
      }
      logger.error("[usePriceModal] recordAssetPrice failed", result.error);
      setSubmitError(assetPriceMutationErrorToI18n(result.error));
      return false;
    } catch (err) {
      logger.error("[usePriceModal] recordAssetPrice threw", { error: err });
      setSubmitError(UNKNOWN_ERROR);
      return false;
    } finally {
      setIsSubmitting(false);
    }
  }, [isFormValid, assetId, accountId, date, price, showSnackbar, t]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (await record()) {
        onSubmitSuccess?.();
      }
    },
    [record, onSubmitSuccess],
  );

  // MKT-014 — record and keep the modal open: refresh, then clear the price while
  // keeping the asset and date for the next quick entry.
  const handleAddAnother = useCallback(async () => {
    if (await record()) {
      onRecorded?.(assetId);
      setPrice("");
    }
  }, [record, onRecorded, assetId]);

  return {
    assetId,
    date,
    price,
    selectedCurrency,
    error,
    isSubmitting,
    isFormValid,
    handleAssetChange,
    handleChange,
    handleSubmit,
    handleAddAnother,
  };
}
