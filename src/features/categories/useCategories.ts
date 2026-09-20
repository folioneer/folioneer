import { useCallback } from "react";
import type { I18nMessage } from "@/ui/format/i18n";
import { useAppStore } from "../../lib/store";
import { categoryGateway } from "./gateway";
import { categoryMutationErrorToI18n } from "./shared/presenter";

export function useCategories() {
  const categories = useAppStore((state) => state.categories);
  const loading = useAppStore((state) => state.isLoadingCategories);
  const error = useAppStore((state) => state.categoriesError);
  const fetchCategories = useAppStore((state) => state.fetchCategories);

  const addCategory = useCallback(async (label: string): Promise<{ error?: I18nMessage }> => {
    const result = await categoryGateway.addCategory(label);
    if (result.status === "error") return { error: categoryMutationErrorToI18n(result.error) };
    return {};
  }, []);

  const updateCategory = useCallback(
    async (id: string, label: string): Promise<{ error?: I18nMessage }> => {
      const result = await categoryGateway.updateCategory(id, label);
      if (result.status === "error") return { error: categoryMutationErrorToI18n(result.error) };
      return {};
    },
    [],
  );

  const deleteCategory = useCallback(async (id: string): Promise<{ error?: I18nMessage }> => {
    const result = await categoryGateway.deleteCategory(id);
    if (result.status === "error") return { error: categoryMutationErrorToI18n(result.error) };
    return {};
  }, []);

  return {
    categories,
    loading,
    error,
    fetchCategories,
    addCategory,
    updateCategory,
    deleteCategory,
  };
}
