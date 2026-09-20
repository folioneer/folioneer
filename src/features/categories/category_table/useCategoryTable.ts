import { useMemo, useState } from "react";
import type { AssetCategory } from "@/bindings";

export type SortConfig = {
  key: keyof AssetCategory;
  direction: "asc" | "desc";
};

export function useCategoryTable(categories: AssetCategory[], searchTerm: string) {
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: "name",
    direction: "asc",
  });

  const handleSort = (key: keyof AssetCategory) => {
    setSortConfig((prev) => ({
      key,
      direction: prev.key === key && prev.direction === "asc" ? "desc" : "asc",
    }));
  };

  const sortedAndFilteredCategories = useMemo(() => {
    let result = [...categories];

    // Filter
    if (searchTerm) {
      const lowerSearch = searchTerm.toLowerCase();
      result = result.filter((cat) => cat.name.toLowerCase().includes(lowerSearch));
    }

    // Sort
    result.sort((a, b) => {
      const aValue = a[sortConfig.key];
      const bValue = b[sortConfig.key];

      if (aValue < bValue) return sortConfig.direction === "asc" ? -1 : 1;
      if (aValue > bValue) return sortConfig.direction === "asc" ? 1 : -1;
      return 0;
    });

    return result;
  }, [categories, searchTerm, sortConfig]);

  return {
    sortedAndFilteredCategories,
    sortConfig,
    handleSort,
  };
}
