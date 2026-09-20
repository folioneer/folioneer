import { useEffect } from "react";
import { UnpricedPricesModal } from "@/features/unpriced_prices/UnpricedPricesModal";
import { logger } from "@/lib/logger";
import { selectUnpricedModalOpen, useAppStore } from "@/lib/store";

/**
 * Shell-level mount for the unupdated-prices modal (MKT-172). Watches the
 * `unpricedAssets` store slice — populated when a fetch task completes with assets
 * it could not price (MKT-170) — and overlays the manual-fill modal while the slice
 * is non-empty. Dismissing or resolving every row clears the slice (MKT-177).
 */
export function UnpricedPricesModalMount() {
  const isOpen = useAppStore(selectUnpricedModalOpen);
  const unpricedAssets = useAppStore((state) => state.unpricedAssets);
  const clearUnpricedAssets = useAppStore((state) => state.clearUnpricedAssets);

  useEffect(() => {
    logger.info("[UnpricedPricesModalMount] mounted");
  }, []);

  if (!isOpen) return null;

  return <UnpricedPricesModal assets={unpricedAssets} onClose={clearUnpricedAssets} />;
}
