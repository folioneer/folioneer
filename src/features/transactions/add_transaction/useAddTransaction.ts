import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getAutoRecordPrice } from "@/lib/autoRecordPriceStorage";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { microToExactDecimal, microToFormatted } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { transactionGateway } from "../gateway";
import type { TransactionFormData } from "../shared/types";
import { type DraftField, useDraftProblemDisplay } from "../shared/useDraftProblemDisplay";
import { toTransactionDraft, useTransactionDraftCheck } from "../shared/useTransactionDraftCheck";
import { useTransactions } from "../useTransactions";

interface UseAddTransactionProps {
  /** Pre-fill the asset (TRX-011). */
  prefillAssetId?: string;
  /** Pre-fill the account (TRX-011). */
  prefillAccountId?: string;
  onSubmitSuccess?: () => void;
}

const defaultForm = (accountId: string): TransactionFormData => ({
  accountId,
  assetId: "",
  date: getLastOperationDate(accountId),
  quantity: "",
  unitPrice: "",
  exchangeRate: "1.000000",
  fees: "",
  note: "",
});

export function useAddTransaction({
  prefillAssetId,
  prefillAccountId,
  onSubmitSuccess,
}: UseAddTransactionProps = {}) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const { buyHolding } = useTransactions();

  const [formData, setFormData] = useState<TransactionFormData>(() => ({
    ...defaultForm(prefillAccountId ?? ""),
    assetId: prefillAssetId ?? "",
  }));
  const [error, setError] = useState<I18nMessage | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  // MKT-052/053 — snapshot of the global auto-record toggle at hook mount
  const [recordPrice, setRecordPrice] = useState<boolean>(() => getAutoRecordPrice());

  // TRX-063 — the draft check decides whether the purchase can be saved and what it totals.
  const draft = useMemo(() => toTransactionDraft("Purchase", formData, "price", ""), [formData]);
  const check = useTransactionDraftCheck(draft);

  // TRX-067 — the first problem is shown on its field once typed in, as a hint before.
  const problemDisplay = useDraftProblemDisplay(check.problem, check.problemMessage);
  const touch = problemDisplay.touch;

  const handleChange = useCallback(
    (field: keyof TransactionFormData, value: string) => {
      setFormData((prev) => ({ ...prev, [field]: value }));
      const typed: Partial<Record<keyof TransactionFormData, DraftField>> = {
        accountId: "account",
        assetId: "asset",
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

  const doSubmit = useCallback(async () => {
    if (!check.isClean) {
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
        unit_price: check.preview?.unit_price ?? 0,
        exchange_rate: draft.entered.exchange_rate,
        fees: draft.entered.fees,
        total_amount: null,
        note: formData.note || null,
      });

      if (result.error) {
        setError(result.error);
        return;
      }

      // MKT-055/061 — record price separately when auto-record is on and price is non-zero (best-effort)
      const unitPrice = check.preview?.unit_price ?? 0;
      if (recordPrice && unitPrice > 0) {
        transactionGateway
          .recordAssetPrice(
            formData.assetId,
            formData.date,
            parseFloat(microToExactDecimal(unitPrice)),
          )
          .catch((e) => logger.warn("Failed to record asset price after buy", { error: e }));
      }

      setLastOperationDate(formData.accountId, formData.date);
      showSnackbar(t("transaction.success_created"), "success");
      setFormData({
        ...defaultForm(formData.accountId),
        assetId: prefillAssetId ?? "",
      });
      onSubmitSuccess?.();
    } finally {
      setIsSubmitting(false);
    }
  }, [
    formData,
    draft,
    check,
    recordPrice,
    buyHolding,
    t,
    prefillAssetId,
    onSubmitSuccess,
    showSnackbar,
  ]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      await doSubmit();
    },
    [doSubmit],
  );

  return {
    formData,
    /** Total amount in micro-units formatted for display (read-only, derived). */
    totalAmountDisplay: microToFormatted(check.preview?.total_amount ?? 0),
    error: error ?? problemDisplay.alert,
    /** TRX-067 — the first problem as an error on the field it concerns, once typed in. */
    fieldErrors: problemDisplay.fieldErrors,
    /** TRX-067 — what to enter, for a field not typed in yet. */
    problemHint: problemDisplay.hint,
    isSubmitting,
    isFormValid: check.isClean,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
  };
}
