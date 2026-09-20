import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { dividendErrorToI18n } from "../shared/presenter";
import { validateAmount, validateDate } from "../shared/validateCashForm";

/** A holding the dividend can be attributed to — active, non-cash (DIV-011/020). */
export interface DividendPayingAsset {
  assetId: string;
  assetName: string;
  assetCurrency: string;
}

interface UseDividendTransactionProps {
  accountId: string;
  accountCurrency: string;
  heldAssets: DividendPayingAsset[];
  /** Called after a successful record via the primary button (caller closes + refreshes). */
  onSubmitSuccess?: () => void;
  /** Called after a successful record via "add another" (caller refreshes, modal stays open). */
  onRecorded?: () => void;
}

interface DividendFormData {
  assetId: string;
  date: string;
  amount: string;
  exchangeRate: string;
  note: string;
}

export function useDividendTransaction({
  accountId,
  accountCurrency,
  heldAssets,
  onSubmitSuccess,
  onRecorded,
}: UseDividendTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<DividendFormData>(() => ({
    assetId: "",
    date: getLastOperationDate(accountId),
    amount: "",
    exchangeRate: "1.000000",
    note: "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  // DIV-028 — when the paying asset's currency differs, the amount can be
  // typed directly in the account currency (no rate to look up).
  const [amountInAccountCurrency, setAmountInAccountCurrency] = useState(false);

  const selectedAsset = useMemo(
    () => heldAssets.find((a) => a.assetId === formData.assetId) ?? null,
    [heldAssets, formData.assetId],
  );

  // DIV-028 — the entry-mode switch only exists when the currencies differ.
  const showCurrencyModeSwitch = useMemo(
    () => selectedAsset !== null && selectedAsset.assetCurrency !== accountCurrency,
    [selectedAsset, accountCurrency],
  );

  // DIV-022/028 — exchange rate only relevant when the paying asset's currency
  // differs from the account currency AND the amount is typed in the asset
  // currency (account-currency mode needs no rate).
  const showExchangeRate = showCurrencyModeSwitch && !amountInAccountCurrency;

  // DIV-028 — the currency the typed amount is denominated in.
  const amountCurrency =
    amountInAccountCurrency || selectedAsset === null
      ? accountCurrency
      : selectedAsset.assetCurrency;

  // DIV-021 — asset selected, amount strictly positive, date valid.
  const isFormValid = useMemo(
    () =>
      formData.assetId !== "" &&
      validateAmount(formData.amount) === null &&
      validateDate(formData.date) === null,
    [formData.assetId, formData.amount, formData.date],
  );

  const handleChange = useCallback((field: keyof DividendFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  // Records the dividend; returns true on success, false on validation/backend
  // error (the error is set on state). Shared by both the primary submit and the
  // "add another" flow (DIV-010).
  const record = useCallback(async (): Promise<boolean> => {
    const amountErr = validateAmount(formData.amount);
    const dateErr = validateDate(formData.date);
    const validationError =
      formData.assetId === "" ? { key: "error.AssetNotHeld" } : (amountErr ?? dateErr);
    if (validationError) {
      setError(validationError);
      return false;
    }

    setError(null);
    setIsSubmitting(true);
    try {
      const result = await accountDetailsGateway.recordDividend({
        account_id: accountId,
        asset_id: formData.assetId,
        date: formData.date,
        amount_micros: decimalToMicro(formData.amount),
        // DIV-029 — account-currency mode credits the typed amount verbatim
        // (rate 1); asset-currency mode converts via the supplied rate.
        exchange_rate:
          showCurrencyModeSwitch && amountInAccountCurrency
            ? decimalToMicro("1")
            : decimalToMicro(formData.exchangeRate),
        note: formData.note || null,
      });
      if (result.status === "error") {
        logger.error("[useDividendTransaction] recordDividend failed", { error: result.error });
        setError(dividendErrorToI18n(result.error));
        return false;
      }
      setLastOperationDate(accountId, formData.date);
      showSnackbar(t("dividend.recorded"), "success");
      return true;
    } finally {
      setIsSubmitting(false);
    }
  }, [accountId, formData, t, showSnackbar, showCurrencyModeSwitch, amountInAccountCurrency]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (await record()) {
        onSubmitSuccess?.();
      }
    },
    [record, onSubmitSuccess],
  );

  // DIV-010 — record and keep the modal open for the next dividend: refresh the
  // background data, then clear the per-dividend fields (amount + note) while
  // keeping the asset, date and rate for quick repeat entry.
  const handleAddAnother = useCallback(async () => {
    if (await record()) {
      onRecorded?.();
      setFormData((prev) => ({ ...prev, amount: "", note: "" }));
    }
  }, [record, onRecorded]);

  return {
    formData,
    error,
    isSubmitting,
    isFormValid,
    showExchangeRate,
    showCurrencyModeSwitch,
    amountInAccountCurrency,
    setAmountInAccountCurrency,
    amountCurrency,
    handleChange,
    handleSubmit,
    handleAddAnother,
  };
}
