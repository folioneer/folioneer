import { renderHook } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetCategory } from "@/bindings";
import { useAppStore } from "@/lib/store";
import type { I18nMessage } from "@/ui/format/i18n";

const { mockAddCategory, mockUpdateCategory, mockDeleteCategory } = vi.hoisted(() => ({
  mockAddCategory: vi.fn(),
  mockUpdateCategory: vi.fn(),
  mockDeleteCategory: vi.fn(),
}));

vi.mock("./gateway", () => ({
  categoryGateway: {
    addCategory: mockAddCategory,
    updateCategory: mockUpdateCategory,
    deleteCategory: mockDeleteCategory,
    getCategories: vi.fn(),
  },
}));

const { useCategories } = await import("./useCategories");

describe("useCategories", () => {
  beforeEach(() => {
    mockAddCategory.mockReset();
    mockUpdateCategory.mockReset();
    mockDeleteCategory.mockReset();
    // Override store fetchCategories so mutations don't hit the gateway.
    useAppStore.setState({
      categories: [] as AssetCategory[],
      isLoadingCategories: false,
      categoriesError: null,
      fetchCategories: vi.fn(),
    });
  });

  // ── addCategory ───────────────────────────────────────────────────────────────

  it("addCategory returns empty object on success", async () => {
    mockAddCategory.mockResolvedValue({
      status: "ok",
      data: { id: "cat-1", name: "Bonds" },
    });
    const { result } = renderHook(() => useCategories());
    let ret: { error?: I18nMessage } = {};
    await act(async () => {
      ret = await result.current.addCategory("Bonds");
    });
    expect(mockAddCategory).toHaveBeenCalledWith("Bonds");
    expect(ret).toEqual({});
  });

  it("addCategory returns error key on failure", async () => {
    mockAddCategory.mockResolvedValue({
      status: "error",
      error: { code: "DuplicateName" },
    });
    const { result } = renderHook(() => useCategories());
    let ret: { error?: I18nMessage } = {};
    await act(async () => {
      ret = await result.current.addCategory("Bonds");
    });
    expect(ret).toEqual({ error: { key: "category.error_duplicate" } });
  });

  // ── updateCategory ────────────────────────────────────────────────────────────

  it("updateCategory returns empty object on success", async () => {
    mockUpdateCategory.mockResolvedValue({
      status: "ok",
      data: { id: "cat-1", name: "Equities" },
    });
    const { result } = renderHook(() => useCategories());
    let ret: { error?: I18nMessage } = {};
    await act(async () => {
      ret = await result.current.updateCategory("cat-1", "Equities");
    });
    expect(mockUpdateCategory).toHaveBeenCalledWith("cat-1", "Equities");
    expect(ret).toEqual({});
  });

  it("updateCategory returns error key on failure", async () => {
    mockUpdateCategory.mockResolvedValue({
      status: "error",
      error: { code: "CategoryNotFound", id: "missing" },
    });
    const { result } = renderHook(() => useCategories());
    let ret: { error?: I18nMessage } = {};
    await act(async () => {
      ret = await result.current.updateCategory("missing", "X");
    });
    expect(ret).toEqual({ error: { key: "category.error_generic" } });
  });

  // ── deleteCategory ────────────────────────────────────────────────────────────

  it("deleteCategory returns empty object on success", async () => {
    mockDeleteCategory.mockResolvedValue({ status: "ok", data: null });
    const { result } = renderHook(() => useCategories());
    let ret: { error?: I18nMessage } = {};
    await act(async () => {
      ret = await result.current.deleteCategory("cat-1");
    });
    expect(mockDeleteCategory).toHaveBeenCalledWith("cat-1");
    expect(ret).toEqual({});
  });

  it("deleteCategory returns system_protected key on system-category attempt", async () => {
    mockDeleteCategory.mockResolvedValue({
      status: "error",
      error: { code: "SystemProtected" },
    });
    const { result } = renderHook(() => useCategories());
    let ret: { error?: I18nMessage } = {};
    await act(async () => {
      ret = await result.current.deleteCategory("cat-1");
    });
    expect(ret).toEqual({ error: { key: "category.error_system_protected" } });
  });
});
