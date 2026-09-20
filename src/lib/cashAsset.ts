const CASH_ASSET_PREFIX = "system-cash-";

/** True when the asset_id is the deterministic system Cash Asset ID (CSH-014). */
export function isCashAsset(assetId: string): boolean {
  return assetId.startsWith(CASH_ASSET_PREFIX);
}
