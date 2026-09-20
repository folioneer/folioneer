import { useNavigate } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import type { Asset } from "@/bindings";
import { patchModalSearch } from "@/lib/modalSearch";

export type SortConfig = {
  key: "name" | "reference" | "class" | "category" | "currency" | "risk_level";
  direction: "asc" | "desc";
};

export function useAssetTable(assets: Asset[], searchTerm: string, showArchived: boolean) {
  const navigate = useNavigate();
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: "name",
    direction: "asc",
  });

  const handleSort = (key: SortConfig["key"]) => {
    setSortConfig((prev) => ({
      key,
      direction: prev.key === key && prev.direction === "asc" ? "desc" : "asc",
    }));
  };

  // Opens the router-driven Edit Asset modal (shell-mounted) so it overlays in
  // the current route context without a cross-feature modal import.
  const openEditAsset = (assetId: string) => {
    patchModalSearch(navigate, { modal: "edit-asset", editAssetId: assetId });
  };

  const sortedAndFilteredAssets = useMemo(() => {
    // CSH-015 — system Cash Assets are infrastructure, never shown in Asset Manager.
    const nonCashAssets = assets.filter((a) => a.class !== "Cash");

    // R7/R19: filter by archive state first
    const visibleAssets = showArchived
      ? nonCashAssets
      : nonCashAssets.filter((a) => !a.is_archived);

    // R16: fuzzy search applies only to currently displayed assets
    const filtered = visibleAssets.filter(
      (a) =>
        a.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        a.reference.toLowerCase().includes(searchTerm.toLowerCase()) ||
        a.class.toLowerCase().includes(searchTerm.toLowerCase()) ||
        a.category.name.toLowerCase().includes(searchTerm.toLowerCase()),
    );

    return [...filtered].sort((a, b) => {
      let aValue: string | number = "";
      let bValue: string | number = "";

      if (sortConfig.key === "category") {
        aValue = a.category.name;
        bValue = b.category.name;
      } else {
        const val = a[sortConfig.key];
        const valB = b[sortConfig.key];
        aValue = (typeof val === "string" || typeof val === "number" ? val : "") ?? "";
        bValue = (typeof valB === "string" || typeof valB === "number" ? valB : "") ?? "";
      }

      if (aValue < bValue) return sortConfig.direction === "asc" ? -1 : 1;
      if (aValue > bValue) return sortConfig.direction === "asc" ? 1 : -1;
      // AST-017 — every primary sort breaks ties by name ascending (the default
      // order), independent of the primary direction, so equal-key rows stay
      // alphabetical. A no-op when the primary key is already name.
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    });
  }, [assets, searchTerm, showArchived, sortConfig]);

  return {
    sortedAndFilteredAssets,
    sortConfig,
    handleSort,
    openEditAsset,
  };
}
