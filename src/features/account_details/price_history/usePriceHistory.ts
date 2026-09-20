import { useCallback, useEffect, useState } from "react";
import type { AssetPrice } from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { assetPriceMutationErrorToI18n } from "../shared/presenter";

const UNKNOWN_ERROR: I18nMessage = { key: "error.Unknown" };

interface UsePriceHistoryProps {
  assetId: string;
}

export interface UsePriceHistoryResult {
  prices: AssetPrice[];
  isLoading: boolean;
  fetchError: I18nMessage | null;
  deleteError: I18nMessage | null;
  deletingDate: string | null;
  refetch: () => void;
  /** Returns true on success, false on failure. */
  confirmDelete: (date: string) => Promise<boolean>;
}

export function usePriceHistory({ assetId }: UsePriceHistoryProps): UsePriceHistoryResult {
  const [prices, setPrices] = useState<AssetPrice[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [fetchError, setFetchError] = useState<I18nMessage | null>(null);
  const [deleteError, setDeleteError] = useState<I18nMessage | null>(null);
  const [deletingDate, setDeletingDate] = useState<string | null>(null);

  const loadPrices = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await accountDetailsGateway.getAssetPrices(assetId);
      if (result.status === "ok") {
        setPrices(result.data);
        setFetchError(null);
      } else {
        logger.error("[usePriceHistory] getAssetPrices failed", result.error);
        setFetchError(assetPriceMutationErrorToI18n(result.error));
      }
    } catch (err) {
      logger.error("[usePriceHistory] getAssetPrices threw", err);
      setFetchError(UNKNOWN_ERROR);
    } finally {
      setIsLoading(false);
    }
  }, [assetId]);

  useEffect(() => {
    loadPrices();
  }, [loadPrices]);

  const confirmDelete = useCallback(
    async (date: string): Promise<boolean> => {
      setDeletingDate(date);
      const result = await accountDetailsGateway.deleteAssetPrice(assetId, date);
      setDeletingDate(null);
      if (result.status === "ok") {
        setDeleteError(null);
        loadPrices();
        return true;
      }
      logger.error("[usePriceHistory] deleteAssetPrice failed", result.error);
      setDeleteError(assetPriceMutationErrorToI18n(result.error));
      return false;
    },
    [assetId, loadPrices],
  );

  return {
    prices,
    isLoading,
    fetchError,
    deleteError,
    deletingDate,
    refetch: loadPrices,
    confirmDelete,
  };
}
