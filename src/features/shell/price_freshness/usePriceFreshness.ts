import { useCallback, useEffect, useState } from "react";
import type { PriceFreshness } from "@/bindings";
import { logger } from "@/lib/logger";
import { useAppStore } from "@/lib/store";
import { getPriceFreshness, subscribeToEvents } from "../gateway";

export interface UsePriceFreshnessResult {
  /** False until a read has succeeded: there is nothing truthful to show before that. */
  visible: boolean;
  /** Date of the newest price among the held assets (MKT-200); null when there is none. */
  newestPriceDate: string | null;
  /** When this computer last fetched prices (MKT-201); null when it never did. */
  lastFetchAt: string | null;
}

/**
 * MKT-202/203 — the header's price item: reads the two figures on mount and again after
 * whatever changes them — a completed fetch task, a price recorded outside one, a
 * transaction changing what is held, a sync that applied changes.
 */
export function usePriceFreshness(): UsePriceFreshnessResult {
  const [freshness, setFreshness] = useState<PriceFreshness | null>(null);

  const refresh = useCallback(async () => {
    const result = await getPriceFreshness();
    if (result.status === "ok") {
      setFreshness(result.data);
    } else {
      logger.error("[usePriceFreshness] get_price_freshness failed", { error: result.error });
    }
  }, []);

  useEffect(() => {
    void refresh();
    const unlistenPromise = subscribeToEvents((type) => {
      // MKT-181 — per-asset events are coalesced while a fetch task runs; the item
      // reads once on AssetPriceFetchCompleted.
      if (type === "AssetPriceUpdated" && useAppStore.getState().priceFetch.active) {
        return;
      }
      if (
        type === "AssetPriceFetchCompleted" ||
        type === "AssetPriceUpdated" ||
        type === "TransactionUpdated" ||
        type === "SyncCompleted"
      ) {
        void refresh();
      }
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh]);

  return {
    visible: freshness !== null,
    newestPriceDate: freshness?.newest_price_date ?? null,
    lastFetchAt: freshness?.last_fetch_at ?? null,
  };
}
