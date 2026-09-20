import type { Asset } from "@/bindings";

/**
 * Returns true if the given reference matches an existing asset (case-insensitive).
 * Includes both active and archived assets — R9.
 * Pass excludeId to ignore the asset currently being edited.
 */
export function hasDuplicateReference(
  reference: string,
  assets: Asset[],
  excludeId?: string,
): boolean {
  const normalized = reference.trim().toUpperCase();
  if (!normalized) return false;
  return assets.some(
    (a) =>
      a.reference.toUpperCase() === normalized && (excludeId === undefined || a.id !== excludeId),
  );
}
