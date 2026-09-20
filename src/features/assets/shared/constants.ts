import type { AssetClass } from "@/bindings";

export const SYSTEM_CATEGORY_ID = "default-uncategorized";

// CSH-015 — `Cash` is a system-only asset class, seeded automatically via
// `ensure_cash_asset` for each account currency. Excluded from the user-facing
// Add Asset / Edit Asset dropdown so users cannot create their own cash assets.
export const ASSET_CLASSES: AssetClass[] = [
  "RealEstate",
  "Stocks",
  "Bonds",
  "ETF",
  "ETP",
  "MutualFunds",
  "DigitalAsset",
  "Derivatives",
];

export const RISK_LEVELS = [1, 2, 3, 4, 5];

/** Default risk level per asset class — R3. */
export const DEFAULT_RISK_BY_CLASS: Record<AssetClass, number> = {
  Cash: 1,
  Bonds: 2,
  RealEstate: 2,
  MutualFunds: 3,
  ETF: 3,
  ETP: 3,
  Stocks: 4,
  DigitalAsset: 5,
  Derivatives: 5,
};
