import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import { decimalToMicro, microToFieldDecimal } from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountDetailsGateway } from "../gateway";
import { type TransferRefusal, transferErrorToI18n } from "../shared/presenter";
import type { TransferDestination, TransferTarget } from "../shared/types";
import { validateAmount, validateDate } from "../shared/validateCashForm";

/** Edit-mode context (TRF-057): the transfer corrected; its asset and accounts are fixed. */
export interface TransferEditMode {
  transferId: string;
  toAccountId: string;
  initialDate: string;
  /** The quantities and the rate as field decimals. */
  initialSent: string;
  initialReceived: string;
  initialRate: string;
  /** The quantity sent in micro-units, counted back into what is held (TRF-057). */
  initialSentMicro: number;
  initialNote: string;
}

interface UseTransferProps {
  accountId: string;
  target: TransferTarget;
  destinations: TransferDestination[];
  onSubmitSuccess?: () => void;
  editMode?: TransferEditMode;
}

interface TransferFormData {
  date: string;
  toAccountId: string;
  sent: string;
  received: string;
  rate: string;
  note: string;
}

/** TRF-052 — the line under the rate: hidden once typed or filled from a correction, while the
 * rate is read, the stored rate found, or none stored. */
export type StoredRate = "typed" | "loading" | "stored" | "none";

/** A figure typed in a field, in micro-units; null when it is no positive figure (TRF-054). */
function positiveMicro(value: string): number | null {
  if (validateAmount(value) !== null) return null;
  const micro = decimalToMicro(value);
  return micro > 0 ? micro : null;
}

/** `a × b` in micro-units, rounded down (TRF-020, TRF-022). */
function times(aMicro: number, bMicro: number): number {
  return Number((BigInt(aMicro) * BigInt(bMicro)) / 1_000_000n);
}

export function useTransfer({
  accountId,
  target,
  destinations,
  onSubmitSuccess,
  editMode,
}: UseTransferProps) {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();

  const [formData, setFormData] = useState<TransferFormData>(() => ({
    date: editMode?.initialDate ?? getLastOperationDate(accountId),
    toAccountId: editMode?.toAccountId ?? "",
    sent: editMode?.initialSent ?? "",
    received: editMode?.initialReceived ?? "",
    rate: editMode?.initialRate ?? "",
    note: editMode?.initialNote ?? "",
  }));
  // TRF-051 / TRF-057 — the quantity received follows the quantity sent until it is typed;
  // a correction's filled figures count as typed.
  const [receivedTyped, setReceivedTyped] = useState(!!editMode);
  const [rateTyped, setRateTyped] = useState(!!editMode);
  const [storedRate, setStoredRate] = useState<StoredRate>(editMode ? "typed" : "loading");
  const [refusal, setRefusal] = useState<TransferRefusal | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const destination = destinations.find((d) => d.id === formData.toAccountId) ?? null;
  const betweenCurrencies = destination !== null && destination.currency !== target.accountCurrency;

  // TRF-052 — the rate the core would take for the date, read again when the date or the
  // receiving account changes, until the owner types one.
  useEffect(() => {
    if (!betweenCurrencies || rateTyped || validateDate(formData.date) !== null) {
      return;
    }
    let cancelled = false;
    setStoredRate("loading");
    void (async () => {
      const result = await accountDetailsGateway.getTransferRate(
        accountId,
        formData.toAccountId,
        formData.date,
      );
      if (cancelled) return;
      if (result.status === "error") {
        logger.error("[useTransfer] the stored rate was not read", { error: result.error });
        setStoredRate("none");
        setFormData((prev) => ({ ...prev, rate: "" }));
        return;
      }
      const rate = result.data;
      setStoredRate(rate === null ? "none" : "stored");
      setFormData((prev) => ({ ...prev, rate: rate === null ? "" : microToFieldDecimal(rate) }));
    })();
    return () => {
      cancelled = true;
    };
  }, [accountId, betweenCurrencies, formData.toAccountId, formData.date, rateTyped]);

  // TRF-057 — when correcting, the transfer's quantity sent counts back into what is held.
  const heldMicro = target.quantityMicro + (editMode?.initialSentMicro ?? 0);

  const sentMicro = useMemo(() => positiveMicro(formData.sent), [formData.sent]);
  const receivedMicro = useMemo(() => positiveMicro(formData.received), [formData.received]);
  const rateMicro = useMemo(
    () => (betweenCurrencies ? positiveMicro(formData.rate) : 1_000_000),
    [betweenCurrencies, formData.rate],
  );

  // TRF-054 — refused under their fields as they are typed.
  const overHolding = sentMicro !== null && sentMicro > heldMicro;
  const receivedAboveSent =
    sentMicro !== null && receivedMicro !== null && receivedMicro > sentMicro;

  const isFormValid =
    destination !== null &&
    sentMicro !== null &&
    receivedMicro !== null &&
    rateMicro !== null &&
    !overHolding &&
    !receivedAboveSent &&
    validateDate(formData.date) === null;

  // TRF-053 — the form's estimate at the average price shown.
  const summary = useMemo(() => {
    if (
      sentMicro === null ||
      receivedMicro === null ||
      overHolding ||
      receivedAboveSent ||
      destination === null
    ) {
      return null;
    }
    const costMicro = times(receivedMicro, target.averagePriceMicro);
    const feeMicro = sentMicro - receivedMicro;
    return {
      costMicro,
      convertedMicro: betweenCurrencies && rateMicro !== null ? times(costMicro, rateMicro) : null,
      rateMicro,
      feeMicro,
      feeCostMicro: times(feeMicro, target.averagePriceMicro),
    };
  }, [
    sentMicro,
    receivedMicro,
    overHolding,
    receivedAboveSent,
    destination,
    betweenCurrencies,
    rateMicro,
    target.averagePriceMicro,
  ]);

  const handleChange = useCallback(
    (field: keyof TransferFormData, value: string) => {
      // TRF-055 — a refusal holds until the date, the account, a quantity or the rate changes.
      if (field !== "note") setRefusal(null);
      if (field === "received") setReceivedTyped(true);
      if (field === "rate") {
        setRateTyped(true);
        setStoredRate("typed");
      }
      setFormData((prev) => {
        const next = { ...prev, [field]: value };
        if (field === "sent" && !receivedTyped) next.received = value;
        return next;
      });
    },
    [receivedTyped],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!isFormValid || sentMicro === null || receivedMicro === null || rateMicro === null) {
        return;
      }
      setRefusal(null);
      setIsSubmitting(true);
      try {
        const figures = {
          date: formData.date,
          quantity_sent: sentMicro,
          quantity_received: receivedMicro,
          exchange_rate: betweenCurrencies ? rateMicro : null,
          note: formData.note || null,
        };
        const result = editMode
          ? await accountDetailsGateway.correctTransfer({
              transfer_id: editMode.transferId,
              ...figures,
            })
          : await accountDetailsGateway.recordTransfer({
              from_account_id: accountId,
              to_account_id: formData.toAccountId,
              asset_id: target.assetId,
              ...figures,
            });
        if (result.status === "error") {
          logger.error("[useTransfer] transfer refused", { error: result.error });
          setRefusal(
            transferErrorToI18n(result.error, target.assetReference, formData.date, i18n.language),
          );
          return;
        }
        if (!editMode) setLastOperationDate(accountId, formData.date);
        showSnackbar(t("transfer.recorded"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      accountId,
      target,
      formData,
      isFormValid,
      sentMicro,
      receivedMicro,
      rateMicro,
      betweenCurrencies,
      editMode,
      t,
      i18n.language,
      showSnackbar,
      onSubmitSuccess,
    ],
  );

  return {
    formData,
    destination,
    betweenCurrencies,
    storedRate,
    refusal,
    isSubmitting,
    isFormValid,
    heldMicro,
    overHolding,
    receivedAboveSent,
    summary,
    handleChange,
    handleSubmit,
  };
}
