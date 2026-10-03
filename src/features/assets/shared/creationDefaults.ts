import { ASSET_CREATION_DEFAULTS, type AssetClass } from "@/bindings";

/**
 * What a new asset starts from, as the core defines it (CSH-015, R3): the classes a user
 * may pick, the class and the category preselected, and each class's default risk level.
 * The values are generated from the core into the bindings; nothing is decided here.
 */
export const ADDABLE_ASSET_CLASSES: AssetClass[] = ASSET_CREATION_DEFAULTS.classes.map(
  (entry) => entry.class,
);

export const DEFAULT_ASSET_CLASS: AssetClass = ASSET_CREATION_DEFAULTS.class;
/** The risk levels offered, lowest risk first. */
export const RISK_LEVELS: readonly number[] = ASSET_CREATION_DEFAULTS.risk_levels;

export const DEFAULT_CATEGORY_ID: string = ASSET_CREATION_DEFAULTS.category_id;

/** The risk level the core preselects for a class; its preselected level for a class it does not offer. */
export function defaultRiskOf(assetClass: AssetClass): number {
  return (
    ASSET_CREATION_DEFAULTS.classes.find((entry) => entry.class === assetClass)?.default_risk ??
    ASSET_CREATION_DEFAULTS.risk_level
  );
}
