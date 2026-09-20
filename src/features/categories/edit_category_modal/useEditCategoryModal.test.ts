import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useEditCategoryModal } from "./useEditCategoryModal";

const mockUpdateCategory = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("../useCategories", () => ({
  useCategories: () => ({
    addCategory: vi.fn(),
    updateCategory: mockUpdateCategory,
    deleteCategory: vi.fn(),
    categories: [],
    loading: false,
    error: null,
    fetchCategories: vi.fn(),
  }),
}));

const fakeCategory = { id: "cat-1", name: "Bonds" };
const fakeSubmit = { preventDefault: vi.fn() } as unknown as React.FormEvent;

describe("useEditCategoryModal", () => {
  beforeEach(() => {
    mockUpdateCategory.mockReset();
  });

  it("calls updateCategory and closes on success", async () => {
    mockUpdateCategory.mockResolvedValue({});
    const onClose = vi.fn();
    const { result } = renderHook(() => useEditCategoryModal({ category: fakeCategory, onClose }));

    act(() => {
      result.current.handleChange({
        target: { value: "New Name" },
      } as React.ChangeEvent<HTMLInputElement>);
    });

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(mockUpdateCategory).toHaveBeenCalledWith("cat-1", "New Name");
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(result.current.error).toBeNull();
  });

  it("stores duplicate I18nMessage when updateCategory returns it (presenter mapping verified upstream)", async () => {
    mockUpdateCategory.mockResolvedValue({ error: { key: "category.error_duplicate" } });
    const { result } = renderHook(() =>
      useEditCategoryModal({ category: fakeCategory, onClose: vi.fn() }),
    );

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "category.error_duplicate" });
  });

  it("stores system_readonly I18nMessage when updateCategory returns it", async () => {
    mockUpdateCategory.mockResolvedValue({ error: { key: "category.error_system_readonly" } });
    const { result } = renderHook(() =>
      useEditCategoryModal({ category: fakeCategory, onClose: vi.fn() }),
    );

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });

    expect(result.current.error).toEqual({ key: "category.error_system_readonly" });
  });

  it("resets name and clears error when category prop changes", async () => {
    mockUpdateCategory.mockResolvedValue({ error: { key: "category.error_duplicate" } });
    const onClose = vi.fn();
    const { result, rerender } = renderHook(
      ({ category }) => useEditCategoryModal({ category, onClose }),
      { initialProps: { category: fakeCategory } },
    );

    await act(async () => {
      await result.current.handleSubmit(fakeSubmit);
    });
    expect(result.current.error).not.toBeNull();

    const newCategory = { id: "cat-2", name: "Equities" };
    rerender({ category: newCategory });

    expect(result.current.name).toBe("Equities");
    expect(result.current.error).toBeNull();
  });
});
