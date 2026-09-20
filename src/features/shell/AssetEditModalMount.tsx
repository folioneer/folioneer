import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { EditAssetModal } from "@/features/assets/edit_asset_modal/EditAssetModal";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";
import { useAppStore } from "@/lib/store";

/**
 * Shell-level URL-driven mount for the Edit Asset modal (MKT-032 Interactivity).
 *
 * Subscribes to URL search params (`modal=edit-asset&editAssetId=...&focusField=reference|isin`)
 * and overlays EditAssetModal when present. Closing the modal clears the params,
 * removing the modal from the URL.
 *
 * Lets sibling features open the Edit Asset modal by mutating URL params only —
 * no cross-feature import of EditAssetModal needed at the call site.
 */
export function AssetEditModalMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();
  const assets = useAppStore((s) => s.assets);

  useEffect(() => {
    logger.info("[AssetEditModalMount] mounted");
  }, []);

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const editAssetId = typeof search.editAssetId === "string" ? search.editAssetId : undefined;
  const focusField =
    search.focusField === "reference" || search.focusField === "isin"
      ? (search.focusField as "reference" | "isin")
      : undefined;

  const handleClose = useCallback(() => {
    patchModalSearch(
      navigate,
      { modal: undefined, editAssetId: undefined, focusField: undefined },
      { replace: true },
    );
  }, [navigate]);

  if (modal !== "edit-asset" || !editAssetId) return null;

  const asset = assets.find((a) => a.id === editAssetId) ?? null;
  if (asset === null) return null;

  return (
    <EditAssetModal isOpen={true} onClose={handleClose} asset={asset} focusField={focusField} />
  );
}
