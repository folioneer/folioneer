import { useCallback, useEffect, useRef, useState } from "react";
import type { UnpricedAsset } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";
import { unpricedPricesGateway } from "./gateway";
import { recordPriceErrorToI18n } from "./shared/presenter";

/** Today's local calendar date as an ISO `yyyy-mm-dd` string (project convention). */
const today = () => new Date().toISOString().slice(0, 10);

/** A modal row: the unpriced asset plus its per-row submission state (MKT-178). */
export interface UnpricedRow extends UnpricedAsset {
  isSubmitting: boolean;
  error: I18nMessage | null;
}

export interface UseUnpricedPrices {
  rows: UnpricedRow[];
  /** MKT-175 — record a `Manual` price for the asset, dated to today. */
  record: (assetId: string, price: number) => Promise<void>;
  /** MKT-176 — drop the row without recording anything. */
  skip: (assetId: string) => void;
}

/**
 * Drives the unupdated-prices modal (MKT-174–179): a per-row state machine over the
 * unpriced assets from a completed fetch. A recorded (MKT-175) or skipped (MKT-176)
 * row leaves the list; when the last row is resolved, `onClose` fires (MKT-177).
 */
export function useUnpricedPrices(assets: UnpricedAsset[], onClose: () => void): UseUnpricedPrices {
  const [rows, setRows] = useState<UnpricedRow[]>(() =>
    assets.map((asset) => ({ ...asset, isSubmitting: false, error: null })),
  );

  // MKT-177 — close once every row has been resolved, but not on an initial empty
  // mount: only after the list has held at least one row.
  const hadRows = useRef(false);
  useEffect(() => {
    if (rows.length > 0) {
      hadRows.current = true;
      return;
    }
    if (hadRows.current) onClose();
  }, [rows, onClose]);

  const record = useCallback(async (assetId: string, price: number) => {
    setRows((prev) =>
      prev.map((row) =>
        row.asset_id === assetId ? { ...row, isSubmitting: true, error: null } : row,
      ),
    );
    const result = await unpricedPricesGateway.recordPrice(assetId, today(), price);
    if (result.status === "ok") {
      setRows((prev) => prev.filter((row) => row.asset_id !== assetId));
    } else {
      const error = recordPriceErrorToI18n(result.error);
      setRows((prev) =>
        prev.map((row) =>
          row.asset_id === assetId ? { ...row, isSubmitting: false, error } : row,
        ),
      );
    }
  }, []);

  const skip = useCallback((assetId: string) => {
    setRows((prev) => prev.filter((row) => row.asset_id !== assetId));
  }, []);

  return { rows, record, skip };
}
