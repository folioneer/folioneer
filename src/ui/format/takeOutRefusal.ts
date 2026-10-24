import { microToFormattedQuantity } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "./date";
import type { I18nMessage } from "./i18n";

/**
 * SEL-046 / WOF-055 — the refusal of a sale or a write-off for lack of units, recorded or
 * corrected, as it shows under the quantity: what was held of `reference` on `isoDate`, or
 * that a later transaction would be left short; null for any other refusal. Shared by the
 * sale and write-off forms (F28).
 */
export function takeOutRefusal(
  error: { code: string; available?: number },
  reference: string,
  isoDate: string,
  locale: string,
): I18nMessage | null {
  const date = formatIsoDateNumeric(isoDate, locale);
  switch (error.code) {
    case "Oversell":
      if (error.available === undefined) return null;
      return {
        key: "transaction.error_over_held_on",
        vars: { held: microToFormattedQuantity(error.available), reference, date },
      };
    case "ClosedPosition":
      return { key: "transaction.error_none_held_on", vars: { reference, date } };
    case "CascadingOversell":
      return { key: "transaction.error_later_left_short" };
    default:
      return null;
  }
}
