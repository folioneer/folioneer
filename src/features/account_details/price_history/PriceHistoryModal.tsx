import { Pencil, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AssetPrice, HoldingDetail } from "@/bindings";
import { logger } from "@/lib/logger";
import { microToFormattedPrice } from "@/lib/microUnits";
import { Button } from "@/ui/components/button/Button";
import { IconButton } from "@/ui/components/button/IconButton";
import { ConfirmationDialog } from "@/ui/components/modal/Dialog";
import { ListModal } from "@/ui/components/modal/ListModal";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { formatIsoDateNumeric } from "@/ui/format/date";
import { PriceModal } from "../account_details_view/PriceModal";
import { formatSource } from "../shared/presenter";
import type { PriceableAsset } from "../shared/types";
import { EditPriceForm } from "./EditPriceForm";
import { usePriceHistory } from "./usePriceHistory";

interface PriceHistoryModalProps {
  isOpen: boolean;
  onClose: () => void;
  holding: HoldingDetail;
  /** Account whose holdings are selectable in the add-price combobox (MKT-011). */
  accountId: string;
  /** Active non-cash holdings selectable when adding a price. */
  priceableAssets: PriceableAsset[];
}

export function PriceHistoryModal({
  isOpen,
  onClose,
  holding,
  accountId,
  priceableAssets,
}: PriceHistoryModalProps) {
  const { t, i18n } = useTranslation();
  const showSnackbar = useSnackbar();
  const { prices, isLoading, fetchError, deleteError, deletingDate, confirmDelete, refetch } =
    usePriceHistory({ assetId: holding.asset_id });

  const [editTarget, setEditTarget] = useState<AssetPrice | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<AssetPrice | null>(null);
  const [showAddPrice, setShowAddPrice] = useState(false);

  useEffect(() => {
    logger.info("[PriceHistoryModal] mounted", { assetId: holding.asset_id });
  }, [holding.asset_id]);

  // MKT-075 — "Add price" re-uses the existing PriceModal; refetch on success
  const handleAddPriceSuccess = useCallback(() => {
    setShowAddPrice(false);
    refetch();
  }, [refetch]);

  // MKT-086 — snackbar + refetch after successful edit
  const handleEditSuccess = useCallback(() => {
    setEditTarget(null);
    refetch();
    showSnackbar(t("price_history.edit_success"));
  }, [refetch, showSnackbar, t]);

  // MKT-093 — only dismiss dialog when delete succeeded; keep open on failure so user can retry
  // MKT-092 — snackbar on successful delete
  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return;
    const ok = await confirmDelete(deleteTarget.date);
    if (ok) {
      setDeleteTarget(null);
      showSnackbar(t("price_history.delete_success"));
    }
  }, [deleteTarget, confirmDelete, showSnackbar, t]);

  if (editTarget) {
    return (
      <EditPriceForm
        isOpen={isOpen}
        assetId={holding.asset_id}
        assetName={holding.asset_name}
        assetCurrency={holding.asset_currency}
        target={editTarget}
        onSuccess={handleEditSuccess}
        onBack={() => setEditTarget(null)}
        onClose={onClose}
      />
    );
  }

  return (
    <>
      <ListModal
        isOpen={isOpen}
        onClose={onClose}
        title={`${t("price_history.title")} — ${holding.asset_name}`}
        maxWidth="max-w-2xl"
        footer={
          /* MKT-075 — always-visible "Add price" action */
          <div className="flex justify-end">
            <Button
              variant="primary"
              size="sm"
              icon={<Plus size={14} />}
              onClick={() => setShowAddPrice(true)}
            >
              {t("price_history.add_price")}
            </Button>
          </div>
        }
      >
        {isLoading ? (
          <div className="animate-pulse space-y-2 p-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-8 bg-m3-surface-variant rounded" />
            ))}
          </div>
        ) : fetchError ? (
          /* MKT-074 — inline error with retry affordance */
          <div className="flex flex-col items-center gap-3 py-4">
            <p role="alert" className="text-sm text-m3-error">
              {t("price_history.fetch_error")}
            </p>
            <Button variant="secondary" size="sm" onClick={refetch}>
              {t("action.retry")}
            </Button>
          </div>
        ) : prices.length === 0 ? (
          /* MKT-073 — empty state with "Add price" CTA */
          <div className="flex flex-col items-center gap-3 py-4">
            <p className="text-sm text-m3-on-surface-variant italic">{t("price_history.empty")}</p>
            <Button
              variant="primary"
              size="sm"
              icon={<Plus size={14} />}
              onClick={() => setShowAddPrice(true)}
            >
              {t("price_history.add_price")}
            </Button>
          </div>
        ) : (
          <table className="w-full border-collapse">
            <thead>
              <tr>
                <th className="m3-th text-left">{t("price_history.column_date")}</th>
                <th className="m3-th text-right">
                  {t("price_history.column_price")} ({holding.asset_currency})
                </th>
                <th className="m3-th text-left">{t("price_history.column_source")}</th>
                <th className="m3-th" />
              </tr>
            </thead>
            <tbody>
              {prices.map((row) => {
                const sourceLabel = formatSource(row.source);
                return (
                  <tr key={row.date} className="m3-tr">
                    <td className="m3-td">{formatIsoDateNumeric(row.date, i18n.language)}</td>
                    <td className="m3-td text-right tabular-nums">
                      {microToFormattedPrice(row.price)}
                    </td>
                    <td className="m3-td">
                      {sourceLabel && (
                        <span className="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded bg-m3-surface-container-highest text-m3-on-surface-variant">
                          {t(sourceLabel)}
                        </span>
                      )}
                    </td>
                    <td className="m3-td">
                      <div className="flex items-center gap-1 justify-end">
                        <IconButton
                          icon={<Pencil size={14} />}
                          size="sm"
                          aria-label={t("price_history.action_edit")}
                          onClick={() => setEditTarget(row)}
                        />
                        <IconButton
                          icon={<Trash2 size={14} />}
                          size="sm"
                          variant="error"
                          aria-label={t("price_history.action_delete")}
                          onClick={() => setDeleteTarget(row)}
                          disabled={deletingDate === row.date}
                        />
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}

        {deleteError && (
          <p role="alert" className="text-sm text-m3-error mt-2">
            {t("price_history.delete_error")}
          </p>
        )}
      </ListModal>

      {/* MKT-075 — "Add price" modal (same form as MKT-020–MKT-029) */}
      {showAddPrice && (
        <PriceModal
          isOpen
          onClose={() => setShowAddPrice(false)}
          assets={priceableAssets}
          initialAssetId={holding.asset_id}
          accountId={accountId}
          onSubmitSuccess={handleAddPriceSuccess}
          onRecorded={(recordedAssetId) => {
            // Only the displayed asset's history needs refreshing; a price for a
            // switched-to asset shows on its own history (MKT-014).
            if (recordedAssetId === holding.asset_id) refetch();
          }}
        />
      )}

      {/* MKT-089/093 — Delete confirmation */}
      {deleteTarget && (
        <ConfirmationDialog
          isOpen
          onCancel={() => setDeleteTarget(null)}
          onConfirm={handleDeleteConfirm}
          title={t("price_history.delete_confirm_title")}
          message={t("price_history.delete_confirm_message", {
            date: formatIsoDateNumeric(deleteTarget.date, i18n.language),
          })}
          confirmLabel={t("action.delete")}
          cancelLabel={t("action.cancel")}
          variant="danger"
        />
      )}
    </>
  );
}
