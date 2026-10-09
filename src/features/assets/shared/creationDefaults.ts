import { ASSET_CREATION_DEFAULTS, type AssetClass, type AssetKind } from "@/bindings";

/** What the form of one kind asks for, as the core describes it (AST-037). */
export type KindForm = (typeof ASSET_CREATION_DEFAULTS.kinds)[number];

/**
 * What a new asset starts from, as the core defines it (CSH-015, R3, AST-037): the kinds a
 * user may pick with what each one's form asks for, the class and the category preselected,
 * and each class's default risk level. The values are generated from the core into the
 * bindings; nothing is decided here.
 */
export const KIND_FORMS: readonly KindForm[] = ASSET_CREATION_DEFAULTS.kinds;

export const DEFAULT_ASSET_KIND: AssetKind = ASSET_CREATION_DEFAULTS.kind;

/** The class the core preselects when no kind decides it. */
export const DEFAULT_ASSET_CLASS: AssetClass = ASSET_CREATION_DEFAULTS.class;

/** What the form of a kind asks for; `null` for the kind a user never picks (cash). */
export function kindFormOf(kind: AssetKind): KindForm | null {
  return KIND_FORMS.find((form) => form.kind === kind) ?? null;
}

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

/**
 * The class a form of `kind` shows for `current`: `current` when the kind offers it, else
 * the class the core preselects for that kind (AST-037).
 */
export function classForKind(kind: AssetKind, current: AssetClass | null | undefined): AssetClass {
  const form = kindFormOf(kind);
  if (!form) return current ?? DEFAULT_ASSET_CLASS;
  return current && form.classes.some((entry) => entry.class === current) ? current : form.class;
}

/**
 * The kind whose form offers `assetClass`: `current` when it does or when no class is
 * given, else the first kind of the core's that does (AST-037).
 */
export function kindOffering(
  current: AssetKind,
  assetClass: AssetClass | null | undefined,
): AssetKind {
  const offers = (form: (typeof KIND_FORMS)[number]) =>
    form.classes.some((entry) => entry.class === assetClass);
  if (!assetClass) return current;
  const currentForm = kindFormOf(current);
  if (currentForm && offers(currentForm)) return current;
  return KIND_FORMS.find(offers)?.kind ?? current;
}
