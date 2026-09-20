import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  TransactionEntryMode,
  TransactionFormData,
} from "@/features/transactions/shared/types";
import { validateSellForm } from "@/features/transactions/shared/validateTransaction";
import { useTransactions } from "@/features/transactions/useTransactions";
import { getAutoRecordPrice } from "@/lib/autoRecordPriceStorage";
import { getLastOperationDate, setLastOperationDate } from "@/lib/lastOperationDateStorage";
import { logger } from "@/lib/logger";
import {
  computeCostBasisMicro,
  computeSellTotalMicro,
  decimalToMicro,
  deriveUnitPriceMicro,
  microToDecimal,
  microToFormatted,
} from "@/lib/microUnits";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { useHoldingSnapshotAsOf } from "../shared/useHoldingSnapshotAsOf";

interface UseSellTransactionProps {
  accountId: string;
  assetId: string;
  /** Holding quantity in micro-units — used for oversell guard (SEL-022). */
  holdingQuantityMicro: number;
  onSubmitSuccess?: () => void;
}

export function useSellTransaction({
  accountId,
  assetId,
  holdingQuantityMicro,
  onSubmitSuccess,
}: UseSellTransactionProps) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const { sellHolding } = useTransactions();

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
  // MKT-052/053 — snapshot of the global auto-record toggle at hook mount
  const [recordPrice, setRecordPrice] = useState<boolean>(() => getAutoRecordPrice());
  // SEL-050 — entry mode: unit price typed (default) or all-in net proceeds typed
  const [entryMode, setEntryMode] = useState<TransactionEntryMode>("price");
  const [totalAmountInput, setTotalAmountInput] = useState("");

  const microValues = useMemo(() => {
    const qtyMicro = decimalToMicro(formData.quantity);
    const rateMicro = decimalToMicro(formData.exchangeRate);
    const feesMicro = decimalToMicro(formData.fees);
    if (entryMode === "total") {
      // SEL-050 — the typed net proceeds are ground truth; the unit price is derived
      const totalMicro = decimalToMicro(totalAmountInput);
      const priceMicro = deriveUnitPriceMicro(totalMicro, feesMicro, qtyMicro, rateMicro, true);
      return { qtyMicro, priceMicro, rateMicro, feesMicro, totalMicro };
    }
    const priceMicro = decimalToMicro(formData.unitPrice);
    const totalMicro = computeSellTotalMicro(qtyMicro, priceMicro, rateMicro, feesMicro);
    return { qtyMicro, priceMicro, rateMicro, feesMicro, totalMicro };
  }, [
    formData.quantity,
    formData.unitPrice,
    formData.exchangeRate,
    formData.fees,
    entryMode,
    totalAmountInput,
  ]);

  const isFormValid = useMemo(
    () =>
      validateSellForm(
        formData,
        microValues.qtyMicro,
        microValues.totalMicro,
        holdingQuantityMicro,
      ) === null,
    [formData, microValues.qtyMicro, microValues.totalMicro, holdingQuantityMicro],
  );

  // TDI-020 — average cost as of the entered sell date (or today). Hidden when
  // nothing is held as of that date (TDI-021).
  const { snapshot } = useHoldingSnapshotAsOf(accountId, assetId, formData.date);
  const averageCostAsOfDate = useMemo(
    () => (snapshot && snapshot.quantity > 0 ? microToFormatted(snapshot.average_price) : null),
    [snapshot],
  );

  // TDI-030/031 — potential realized P&L of the typed sell: proceeds minus the
  // VWAP cost basis of the sold quantity. Shown only when a quantity and price
  // are entered and the holding is held as of the date.
  const potentialPnl = useMemo(() => {
    if (!snapshot || snapshot.quantity <= 0) return null;
    if (microValues.qtyMicro <= 0 || microValues.priceMicro <= 0) return null;
    const costBasis = computeCostBasisMicro(snapshot.average_price, microValues.qtyMicro);
    const pnlMicro = microValues.totalMicro - costBasis;
    return { formatted: microToFormatted(pnlMicro), raw: pnlMicro };
  }, [snapshot, microValues.qtyMicro, microValues.priceMicro, microValues.totalMicro]);

  const handleChange = useCallback((field: keyof TransactionFormData, value: string) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  }, []);

  // SEL-050 — switching modes carries over what the user currently sees: price →
  // total seeds the total input from the computed net proceeds (when qty + price
  // are valid); total → price seeds the unit-price field from the derived price
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

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();

      const validationError = validateSellForm(
        formData,
        microValues.qtyMicro,
        microValues.totalMicro,
        holdingQuantityMicro,
      );
      if (validationError) {
        setError(validationError);
        return;
      }

      setError(null);
      setIsSubmitting(true);

      try {
        const result = await sellHolding({
          account_id: formData.accountId,
          asset_id: formData.assetId,
          date: formData.date,
          quantity: microValues.qtyMicro,
          unit_price: microValues.priceMicro,
          exchange_rate: microValues.rateMicro,
          fees: microValues.feesMicro,
          // SEL-050 — total mode ships the typed net proceeds; the backend
          // re-derives the authoritative unit price from them
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
            .catch((e) =>
              logger.warn("Failed to record asset price after sell", {
                error: e,
              }),
            );
        }

        setLastOperationDate(formData.accountId, formData.date);
        showSnackbar(t("transaction.success_sell_created"), "success");
        onSubmitSuccess?.();
      } finally {
        setIsSubmitting(false);
      }
    },
    [
      formData,
      microValues,
      entryMode,
      holdingQuantityMicro,
      recordPrice,
      sellHolding,
      t,
      showSnackbar,
      onSubmitSuccess,
    ],
  );

  return {
    formData,
    /** Sell total proceeds in micro-units formatted for display (SEL-023, read-only). */
    totalAmountDisplay: microToFormatted(microValues.totalMicro),
    /** Maximum sellable quantity formatted for display (SEL-022). */
    maxQuantityDisplay: microToFormatted(holdingQuantityMicro, 6),
    /** SEL-050 — how the money side is entered; resets with the modal (not persisted). */
    entryMode,
    setEntryMode: handleEntryModeChange,
    /** SEL-050 — the typed all-in net proceeds (decimal string), only meaningful in total mode. */
    totalAmountInput,
    handleTotalAmountChange: setTotalAmountInput,
    /** SEL-050 — formatted derived unit price shown in total mode; "—" when quantity is 0. */
    unitPriceDisplay: microValues.qtyMicro > 0 ? microToFormatted(microValues.priceMicro) : "—",
    /** TDI-020 — formatted account-currency average cost as of the date, or null when not held. */
    averageCostAsOfDate,
    /** TDI-030 — potential realized P&L of the typed sell (`{ formatted, raw }`), or null. */
    potentialPnl,
    error,
    isSubmitting,
    isFormValid,
    recordPrice,
    setRecordPrice,
    handleChange,
    handleSubmit,
  };
}
