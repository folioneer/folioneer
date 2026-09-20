import { useNavigate } from "@tanstack/react-router";
import { Archive, ArchiveRestore, Edit2, ShoppingCart, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import { Button } from "@/ui/components/button/Button";
import { IconButton } from "@/ui/components/button/IconButton";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { SortIcon } from "@/ui/components/SortIcon";
import type { I18nMessage } from "@/ui/format/i18n";
import { getRiskBadgeClasses } from "../shared/presenter";
import { useAssets } from "../useAssets";
import { useAssetTable } from "./useAssetTable";

interface AssetTableProps {
  searchTerm: string;
  showArchived: boolean;
}

export function AssetTable({ searchTerm, showArchived }: AssetTableProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { archiveAsset, unarchiveAsset, assets, loading, fetchError, fetchAssets } = useAssets();
  const [selectedAssetId, setSelectedAssetId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    logger.info("[AssetTable] mounted");
  }, []);

  const { sortedAndFilteredAssets, sortConfig, handleSort, openEditAsset } = useAssetTable(
    assets,
    searchTerm,
    showArchived,
  );

  // Archive state
  const [isArchiveDialogOpen, setIsArchiveDialogOpen] = useState(false);
  const [assetToArchive, setAssetToArchive] = useState<{
    id: string;
    name: string;
  } | null>(null);

  // Unarchive state
  const [isUnarchiveDialogOpen, setIsUnarchiveDialogOpen] = useState(false);
  const [assetToUnarchive, setAssetToUnarchive] = useState<{
    id: string;
    name: string;
  } | null>(null);

  const handleArchiveConfirm = async () => {
    if (!assetToArchive) return;
    const result = await archiveAsset(assetToArchive.id);
    setIsArchiveDialogOpen(false);
    setAssetToArchive(null);
    if (result.error) setActionError(result.error);
  };

  const handleUnarchiveConfirm = async () => {
    if (!assetToUnarchive) return;
    const result = await unarchiveAsset(assetToUnarchive.id);
    setIsUnarchiveDialogOpen(false);
    setAssetToUnarchive(null);
    if (result.error) setActionError(result.error);
  };

  const isSearching = searchTerm.trim().length > 0;

  return (
    <div className="m3-table-container flex-1">
      {actionError && (
        <div
          role="alert"
          className="mb-3 flex items-center justify-between gap-2 text-sm text-m3-error px-2"
        >
          <span>{t(actionError.key, actionError.vars)}</span>
          <IconButton
            icon={<X size={14} />}
            size="sm"
            aria-label={t("action.close")}
            onClick={() => setActionError(null)}
          />
        </div>
      )}
      <table className="w-full border-collapse">
        <thead className="sticky top-0 bg-m3-surface-container z-10">
          <tr>
            <th className="m3-th cursor-pointer" onClick={() => handleSort("name")}>
              <div className="flex items-center">
                {t("asset.column_name")}
                <SortIcon active={sortConfig.key === "name"} direction={sortConfig.direction} />
              </div>
            </th>
            <th className="m3-th cursor-pointer" onClick={() => handleSort("reference")}>
              <div className="flex items-center">
                {t("asset.column_reference")}
                <SortIcon
                  active={sortConfig.key === "reference"}
                  direction={sortConfig.direction}
                />
              </div>
            </th>
            <th className="m3-th cursor-pointer" onClick={() => handleSort("class")}>
              <div className="flex items-center">
                {t("asset.column_class")}
                <SortIcon active={sortConfig.key === "class"} direction={sortConfig.direction} />
              </div>
            </th>
            <th className="m3-th cursor-pointer" onClick={() => handleSort("category")}>
              <div className="flex items-center">
                {t("asset.column_category")}
                <SortIcon active={sortConfig.key === "category"} direction={sortConfig.direction} />
              </div>
            </th>
            <th className="m3-th text-center cursor-pointer" onClick={() => handleSort("currency")}>
              <div className="flex items-center justify-center">
                {t("asset.column_currency")}
                <SortIcon active={sortConfig.key === "currency"} direction={sortConfig.direction} />
              </div>
            </th>
            <th
              className="m3-th text-center cursor-pointer"
              onClick={() => handleSort("risk_level")}
            >
              <div className="flex items-center justify-center">
                {t("asset.column_risk")}
                <SortIcon
                  active={sortConfig.key === "risk_level"}
                  direction={sortConfig.direction}
                />
              </div>
            </th>
            <th className="m3-th">{t("asset.column_status")}</th>
            <th className="m3-th text-right">{t("asset.column_actions")}</th>
          </tr>
        </thead>
        <tbody>
          {loading ? (
            <tr>
              <td colSpan={8} className="m3-td text-center py-12">
                <span className="text-m3-on-surface-variant animate-pulse">
                  {t("asset.loading")}
                </span>
              </td>
            </tr>
          ) : assets.length === 0 ? (
            <tr>
              <td colSpan={8} className="m3-td text-center py-12 text-m3-on-surface-variant italic">
                {t("asset.empty")}
              </td>
            </tr>
          ) : fetchError ? (
            <tr>
              <td colSpan={8} className="m3-td text-center py-12">
                <div className="flex flex-col items-center gap-3">
                  <span className="text-m3-error text-sm">{t("asset.error_load")}</span>
                  <Button variant="outline" size="sm" onClick={fetchAssets}>
                    {t("action.retry")}
                  </Button>
                </div>
              </td>
            </tr>
          ) : sortedAndFilteredAssets.length === 0 ? (
            <tr>
              <td colSpan={8} className="m3-td text-center py-12 text-m3-on-surface-variant italic">
                {isSearching ? t("asset.no_search_results") : t("asset.empty")}
              </td>
            </tr>
          ) : (
            sortedAndFilteredAssets.map((asset) => (
              <tr
                key={asset.id}
                id={`asset-row-${asset.id}`}
                tabIndex={0}
                aria-label={
                  asset.is_archived ? undefined : t("asset.open_edit", { name: asset.name })
                }
                onClick={() => setSelectedAssetId(asset.id)}
                onDoubleClick={() => {
                  if (!asset.is_archived) openEditAsset(asset.id);
                }}
                onKeyDown={(e) => {
                  // Enter/Space on an inner interactive element (action buttons)
                  // bubbles up to the row — never treat it as a row action.
                  if (e.defaultPrevented) return;
                  if ((e.target as HTMLElement).closest("button, a, input, select, textarea")) {
                    return;
                  }
                  if (e.key === "Enter") {
                    e.preventDefault();
                    if (!asset.is_archived) openEditAsset(asset.id);
                  } else if (e.key === " ") {
                    e.preventDefault();
                    setSelectedAssetId(asset.id);
                  }
                }}
                className={`m3-tr ${selectedAssetId === asset.id ? "m3-tr-selected" : ""} ${
                  asset.is_archived ? "opacity-50" : ""
                }`}
              >
                <td className="m3-td font-medium text-m3-on-surface">{asset.name}</td>
                <td className="m3-td font-mono text-m3-on-surface-variant">{asset.reference}</td>
                <td className="m3-td">
                  <span className="m3-chip-outline">{asset.class}</span>
                </td>
                <td className="m3-td text-m3-on-surface-variant">{asset.category.name}</td>
                <td className="m3-td text-center text-m3-on-surface font-bold text-xs">
                  {asset.currency}
                </td>
                <td className="m3-td text-center">
                  <span
                    className={`inline-flex items-center justify-center w-7 h-7 rounded-full text-[11px] font-bold ${getRiskBadgeClasses(asset.risk_level)}`}
                  >
                    {asset.risk_level}
                  </span>
                </td>
                <td className="m3-td">
                  {asset.is_archived && (
                    <span className="m3-chip-outline text-xs text-m3-on-surface-variant">
                      {t("asset.badge_archived")}
                    </span>
                  )}
                </td>
                <td className="m3-td text-right">
                  <div className="flex items-center justify-end gap-1">
                    <IconButton
                      icon={<ShoppingCart size={16} />}
                      size="sm"
                      aria-label={t("transaction.action_buy")}
                      onClick={(e) => {
                        e.stopPropagation();
                        navigate({
                          to: "/transactions/new",
                          search: {
                            prefillAssetId: asset.id,
                            prefillAccountId: undefined,
                          },
                        });
                      }}
                    />
                    <IconButton
                      icon={<Edit2 size={16} />}
                      size="sm"
                      id={`action-edit-asset-${asset.id}`}
                      disabled={asset.is_archived}
                      aria-label={t("asset.action_edit")}
                      onClick={(e) => {
                        e.stopPropagation();
                        openEditAsset(asset.id);
                      }}
                    />
                    {asset.is_archived ? (
                      <IconButton
                        icon={<ArchiveRestore size={16} />}
                        size="sm"
                        id={`action-unarchive-asset-${asset.id}`}
                        aria-label={t("asset.action_unarchive")}
                        onClick={(e) => {
                          e.stopPropagation();
                          setAssetToUnarchive({
                            id: asset.id,
                            name: asset.name,
                          });
                          setIsUnarchiveDialogOpen(true);
                        }}
                      />
                    ) : (
                      <IconButton
                        icon={<Archive size={16} />}
                        size="sm"
                        id={`action-archive-asset-${asset.id}`}
                        aria-label={t("asset.action_archive")}
                        onClick={(e) => {
                          e.stopPropagation();
                          setAssetToArchive({ id: asset.id, name: asset.name });
                          setIsArchiveDialogOpen(true);
                        }}
                      />
                    )}
                  </div>
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>

      {/* Archive Confirmation Dialog — R13 */}
      <ConfirmationDialog
        isOpen={isArchiveDialogOpen}
        onCancel={() => {
          setIsArchiveDialogOpen(false);
          setAssetToArchive(null);
        }}
        onConfirm={handleArchiveConfirm}
        title={t("asset.archive_confirm_title")}
        message={t("asset.archive_confirm_message")}
        confirmLabel={t("asset.action_archive")}
        cancelLabel={t("action.cancel")}
        variant="default"
      />

      {/* Unarchive Confirmation Dialog — R20 */}
      <ConfirmationDialog
        isOpen={isUnarchiveDialogOpen}
        onCancel={() => {
          setIsUnarchiveDialogOpen(false);
          setAssetToUnarchive(null);
        }}
        onConfirm={handleUnarchiveConfirm}
        title={t("asset.unarchive_confirm_title")}
        message={t("asset.unarchive_confirm_message")}
        confirmLabel={t("asset.action_unarchive")}
        cancelLabel={t("action.cancel")}
        variant="default"
      />
    </div>
  );
}
