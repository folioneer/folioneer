import { useCallback, useState } from "react";
import type { AssetPrice } from "@/bindings";
import { logger } from "@/lib/logger";
import { microToDecimal } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { assetPriceMutationErrorToI18n } from "../shared/presenter";
import { isDateValid, isPriceValid } from "../shared/validatePriceForm";

interface UseEditPriceProps {
  assetId: string;
  target: AssetPrice;
  onSuccess: () => void;
}

export interface UseEditPriceResult {
  date: string;
  price: string;
  setDate: (v: string) => void;
  setPrice: (v: string) => void;
  isValid: boolean;
  isSubmitting: boolean;
  error: I18nMessage | null;
  handleSubmit: () => Promise<void>;
}

export function useEditPrice({
  assetId,
  target,
  onSuccess,
}: UseEditPriceProps): UseEditPriceResult {
  const [date, setDate] = useState(target.date);
  const [price, setPrice] = useState(() => microToDecimal(target.price, 6));
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<I18nMessage | null>(null);

  const isValid = isPriceValid(price) && isDateValid(date);

  const handleSubmit = useCallback(async () => {
    if (!isValid) return;
    setIsSubmitting(true);
    try {
      const result = await accountDetailsGateway.updateAssetPrice(
        assetId,
        target.date,
        date,
        parseFloat(price),
      );
      if (result.status === "ok") {
        setError(null);
        onSuccess();
      } else {
        logger.error("[useEditPrice] updateAssetPrice failed", result.error);
        setError(assetPriceMutationErrorToI18n(result.error));
      }
    } finally {
      setIsSubmitting(false);
    }
  }, [isValid, assetId, target.date, date, price, onSuccess]);

  return {
    date,
    price,
    setDate,
    setPrice,
    isValid,
    isSubmitting,
    error,
    handleSubmit,
  };
}
