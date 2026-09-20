import { commands } from "@/bindings";

/**
 * Gateway for the unupdated-prices manual-fill modal (MKT-170+). The only command
 * it issues is the shared manual-record path (MKT-175); it owns its own gateway
 * rather than importing another feature's (F26).
 */
export const unpricedPricesGateway = {
  /**
   * MKT-175 — record a manual market price for one asset. Reuses the existing
   * `record_asset_price` command (source = Manual, upsert by (asset_id, date)).
   */
  recordPrice(assetId: string, date: string, price: number) {
    return commands.recordAssetPrice(assetId, date, price);
  },
};
