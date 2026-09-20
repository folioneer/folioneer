import type { AssetError } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/**
 * F27 — maps a manual-record (`record_asset_price`) error to an i18n key plus
 * optional interpolation vars. Pure function: no React, no useTranslation.
 * `err` is the BC-wide `AssetError` union, so the switch lists the codes the
 * record command can raise and falls back to a generic key for any other.
 */
export function recordPriceErrorToI18n(err: AssetError): I18nMessage {
  switch (err.code) {
    case "InvalidDateFormat":
      return { key: "error.InvalidDateFormat", vars: { date: err.date } };
    case "AssetNotFound":
    case "Archived":
    case "DatabaseError":
    case "PriceNotFound":
    case "NotPositive":
    case "NonFinite":
    case "DateInFuture":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}
