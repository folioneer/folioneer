import { useNavigate, useSearch } from "@tanstack/react-router";
import { useCallback, useEffect } from "react";
import { RecordRateModal } from "@/features/currency/record_rate/RecordRateModal";
import { logger } from "@/lib/logger";
import { patchModalSearch } from "@/lib/modalSearch";

/**
 * Shell-level URL-driven mount for the currency Record-Rate modal (FXR-012).
 *
 * Subscribes to URL search params (`modal=record-fx-rate&fxFrom=...&fxTo=...`)
 * and overlays RecordRateModal pre-filled with the pair when present. Closing
 * clears the params. Lets `account_details` open the rate modal from a foreign
 * holding row by mutating URL params only — no cross-feature import (F26).
 */
export function CurrencyRateEditMount() {
  const search = useSearch({ strict: false }) as Record<string, unknown>;
  const navigate = useNavigate();

  useEffect(() => {
    logger.info("[CurrencyRateEditMount] mounted");
  }, []);

  const modal = typeof search.modal === "string" ? search.modal : undefined;
  const fxFrom = typeof search.fxFrom === "string" ? search.fxFrom : undefined;
  const fxTo = typeof search.fxTo === "string" ? search.fxTo : undefined;

  const handleClose = useCallback(() => {
    patchModalSearch(
      navigate,
      { modal: undefined, fxFrom: undefined, fxTo: undefined },
      { replace: true },
    );
  }, [navigate]);

  if (modal !== "record-fx-rate" || !fxFrom || !fxTo) return null;

  return (
    <RecordRateModal
      isOpen
      fromCurrency={fxFrom}
      toCurrency={fxTo}
      onClose={handleClose}
      onSuccess={handleClose}
    />
  );
}
