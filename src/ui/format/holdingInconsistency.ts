import type { HoldingInconsistency } from "@/bindings";
import { microToFormatted, microToFormattedQuantity } from "@/lib/microUnits";
import type { I18nMessage } from "./i18n";

/**
 * SYN-040 / CFR-042 — the display message of a derived holding inconsistency, its
 * micro-unit amount formatted. Shared by the holding row and the sync status (F28).
 */
export function formatHoldingInconsistency(reason: HoldingInconsistency): I18nMessage {
  if ("Oversold" in reason) {
    return {
      key: "sync.inconsistency.oversold",
      vars: { quantity: microToFormattedQuantity(reason.Oversold.quantity) },
    };
  }
  return {
    key: "sync.inconsistency.cash_overdrawn",
    vars: { amount: microToFormatted(reason.CashOverdrawn.amount, 2) },
  };
}
