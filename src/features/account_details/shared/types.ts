import type { ThresholdDirection } from "@/bindings";

export type ModalTarget = {
  accountName: string;
  assetId: string;
  assetName: string;
  assetCurrency: string;
  showExchangeRate: boolean;
};

export type SellTarget = ModalTarget & {
  holdingQuantityMicro: number;
};

/** The holding a stock split rescales — quantity, average and latest price feed the modal's preview and price prefill (SPL-061/040). */
export type SplitTarget = {
  assetId: string;
  assetName: string;
  /** Holding quantity in micro-units. */
  holdingQuantityMicro: number;
  /** Average price in micro-units. */
  averagePriceMicro: number;
  /** Latest recorded price in micro-units, or null when none exists (SPL-040). */
  currentPriceMicro: number | null;
};

/** The holding a note is pinned to; `existing` prefills the modal when a note is already stored (HNO-020/042). */
export type HoldingNoteTarget = {
  assetId: string;
  assetName: string;
  /** ISO 4217 currency of the asset — labels the threshold-price field (HNO-031). */
  assetCurrency: string;
  /** The stored note, or null when creating a new one (HNO-042). */
  existing: {
    text: string;
    /** Alarm threshold in asset-currency micros, or null when no alarm (HNO-011). */
    thresholdPrice: number | null;
    thresholdDirection: ThresholdDirection | null;
  } | null;
};

/** A holding a price can be recorded against — active, non-cash (MKT-010/011). */
export interface PriceableAsset {
  assetId: string;
  assetName: string;
  assetCurrency: string;
}
