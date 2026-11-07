import type { AccountError, HoldingDetail, HoldingLot } from "@/bindings";
import { formatQuantity, microToFormatted } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { I18nMessage } from "@/ui/format/i18n";
import type { HoldingLotsTarget } from "../shared/types";

/** #515 (the validated mock), CBM-034 — one lot as the dialog shows it. */
export interface LotRow {
  key: string;
  acquiredOn: string;
  quantity: string;
  cost: string;
  remaining: string;
  /** Taken whole: shown dimmed. */
  spent: boolean;
  /** Free shares or interest: named beside its date. */
  free: boolean;
}

/** #515 (the validated mock), CBM-034 — the lots, oldest first, as the dialog shows them. */
export function toLotRows(lots: HoldingLot[], locale: string): LotRow[] {
  return lots.map((lot, index) => ({
    key: `${lot.acquired_on}-${index}`,
    acquiredOn: formatIsoDateNumeric(lot.acquired_on, locale),
    quantity: formatQuantity(lot.quantity),
    cost: microToFormatted(lot.cost, 2),
    remaining: formatQuantity(lot.remaining),
    spent: Number(lot.remaining) === 0,
    free: lot.free,
  }));
}

/** #515 (the validated mock) — the holding the dialog opens on, its figures formatted. */
export function toLotsTarget(holding: HoldingDetail, currency: string): HoldingLotsTarget {
  return {
    assetId: holding.asset_id,
    assetName: holding.asset_name,
    quantity: formatQuantity(holding.quantity),
    costBasis: microToFormatted(holding.cost_basis, 2),
    currency,
  };
}

/** CBM-034 — why the lots could not be read. */
export function lotsErrorToI18n(err: AccountError): I18nMessage {
  return err.code === "AccountNotFound" || err.code === "DatabaseError"
    ? { key: `error.${err.code}` }
    : { key: "error.Unknown" };
}
