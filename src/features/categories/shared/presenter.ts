import { type AssetError, SYSTEM_CATEGORY_IDS } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/** Whether the application owns this category (CSH-017): the core lists them. */
export function isSystemCategory(id: string): boolean {
  return (SYSTEM_CATEGORY_IDS as readonly string[]).includes(id);
}

/**
 * F27 — Maps any category-BC mutation error (add / update / delete) to a
 * category-scoped i18n key. Pure function, no React, no useTranslation.
 *
 * Encodes the project's domain mapping:
 * - `DuplicateName` → name-collision wording
 * - `SystemReadonly` / `SystemProtected` → system-category protection wording
 * - Everything else (LabelEmpty / CategoryNotFound / DatabaseError) → generic fallback
 *
 * No payload-bearing variants worth interpolating today — `CategoryNotFound { id }`
 * exposes internal IDs that don't help the user. `err` is the BC-wide `AssetError`
 * union, so unreachable category codes fall through to the generic key.
 */
export function categoryMutationErrorToI18n(err: AssetError): I18nMessage {
  switch (err.code) {
    case "DuplicateName":
      return { key: "category.error_duplicate" };
    case "SystemReadonly":
      return { key: "category.error_system_readonly" };
    case "SystemProtected":
      return { key: "category.error_system_protected" };
    default:
      return { key: "category.error_generic" };
  }
}
