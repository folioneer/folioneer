import { describe, expect, it } from "vitest";
import { SYSTEM_CATEGORY_IDS } from "@/bindings";
import { categoryMutationErrorToI18n, isSystemCategory } from "./presenter";

describe("isSystemCategory (CSH-017)", () => {
  // CSH-017 — the default category and the Cash Category, as the core lists them.
  it("returns true for every category the core lists as its own", () => {
    expect(SYSTEM_CATEGORY_IDS).toHaveLength(2);
    for (const id of SYSTEM_CATEGORY_IDS) expect(isSystemCategory(id)).toBe(true);
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
