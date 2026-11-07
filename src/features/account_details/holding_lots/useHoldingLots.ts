import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountDetailsGateway } from "../gateway";
import { type LotRow, lotsErrorToI18n, toLotRows } from "./presenter";

/** #515 (the validated mock), CBM-034 — the lots of one holding, read when the dialog opens. */
export function useHoldingLots(accountId: string, assetId: string) {
  const { i18n } = useTranslation();
  const [rows, setRows] = useState<LotRow[] | null>(null);
  const [error, setError] = useState<I18nMessage | null>(null);

  useEffect(() => {
    let current = true;
    accountDetailsGateway
      .getHoldingLots(accountId, assetId)
      .then((result) => {
        if (!current) return;
        if (result.status === "ok") {
          setRows(toLotRows(result.data.lots, i18n.language));
        } else {
          logger.warn("[useHoldingLots] lots not read", { code: result.error.code });
          setError(lotsErrorToI18n(result.error));
        }
      })
      .catch((err) => {
        logger.error("[useHoldingLots] lots threw", { err });
        if (current) setError({ key: "error.Unknown" });
      });
    return () => {
      current = false;
    };
  }, [accountId, assetId, i18n.language]);

  return { rows, error };
}
