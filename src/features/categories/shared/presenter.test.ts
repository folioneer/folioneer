import { describe, expect, it } from "vitest";
import {
  categoryMutationErrorToI18n,
  isSystemCategory,
  SYSTEM_CASH_CATEGORY_ID,
  SYSTEM_CATEGORY_ID,
} from "./presenter";

describe("isSystemCategory (CSH-017)", () => {
  it("returns true for the default uncategorized id", () => {
    expect(isSystemCategory(SYSTEM_CATEGORY_ID)).toBe(true);
  });

  // CSH-017 — system Cash Category is also flagged as system, hidden from category lists.
  it("returns true for the system Cash Category id", () => {
    expect(isSystemCategory(SYSTEM_CASH_CATEGORY_ID)).toBe(true);
  });

  it("returns false for a regular category id", () => {
    expect(isSystemCategory("user-category-1")).toBe(false);
  });
});

// F27 layer-3 presenter — exhaustive variant coverage. Category errors map to
// category-scoped i18n keys (category.error_*) rather than the generic error.* namespace,
// matching the project's per-domain wording for system-category protection.
describe("categoryMutationErrorToI18n", () => {
  it("DuplicateName maps to category-scoped duplicate key", () => {
    expect(categoryMutationErrorToI18n({ code: "DuplicateName" })).toEqual({
      key: "category.error_duplicate",
    });
  });

  it("SystemReadonly maps to system_readonly key (rename guard)", () => {
    expect(categoryMutationErrorToI18n({ code: "SystemReadonly" })).toEqual({
      key: "category.error_system_readonly",
    });
  });

  it("SystemProtected maps to system_protected key (delete guard)", () => {
    expect(categoryMutationErrorToI18n({ code: "SystemProtected" })).toEqual({
      key: "category.error_system_protected",
    });
  });

  it.each([
    [{ code: "CategoryNotFound" as const, id: "cat-missing" }],
    [{ code: "DatabaseError" as const }],
    [{ code: "LabelEmpty" as const }],
  ])("%j falls through to generic error key", (err) => {
    expect(categoryMutationErrorToI18n(err)).toEqual({ key: "category.error_generic" });
  });
});
