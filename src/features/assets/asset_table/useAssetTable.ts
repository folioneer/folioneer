import { useNavigate } from "@tanstack/react-router";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Asset, OtherListing } from "@/bindings";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { assetGateway } from "../gateway";

export type SortConfig = {
  key: "name" | "reference" | "class" | "category" | "currency" | "risk_level";
  direction: "asc" | "desc";
};

export function useAssetTable(assets: Asset[], searchTerm: string, showArchived: boolean) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: "name",
    direction: "asc",
  });

  // AST-039 — the other listings of each listed asset's instrument, as the core reports
  // them; read again whenever the assets change.
  const [otherListings, setOtherListings] = useState<Record<string, OtherListing[]>>({});
  useEffect(() => {
    if (assets.length === 0) {
      setOtherListings({});
      return;
    }
    let isCurrent = true;
    assetGateway
      .getOtherListings()
      .then((result) => {
        if (!isCurrent) return;
        if (result.status !== "ok") {
          setOtherListings({});
          showSnackbar(t(`error.${result.error.code}`), "error");
          return;
        }
        setOtherListings(
          Object.fromEntries(result.data.map((entry) => [entry.asset_id, entry.others])),
        );
      })
      .catch((e) => logger.error("[useAssetTable] other listings not read", { error: e }));
    return () => {
      isCurrent = false;
    };
  }, [assets, showSnackbar, t]);

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
    // R7/R19: filter by archive state first
    const visibleAssets = showArchived ? assets : assets.filter((a) => !a.is_archived);

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
    otherListings,
    sortConfig,
    handleSort,
    openEditAsset,
  };
}
