import type {
  AccountError,
  FetchAccountAssetPricesError,
  FetchAllAssetPricesError,
  UpdateFrequency,
} from "@/bindings";
import { microToFormatted } from "@/lib/microUnits";
import { formatIsoDateNumeric } from "@/ui/format/date";
import type { I18nMessage, SnackbarMessage } from "@/ui/format/i18n";

/**
 * ACC-023 — formats an account's account-wide unrealized P&L (account-currency
 * micros) to 2 decimals; "—" when the account has no computable holding (`null`).
 */
export function formatAccountRowTotalUnrealizedPnl(pnl: number | null): string {
  return pnl === null ? "—" : microToFormatted(pnl, 2);
}

/**
 * ACC-024 — formats an account's year-to-date performance (micro-percent) to a
 * signed percentage (e.g. "+8,00%" / "-3,70%"); "—" when `null` (no baseline /
 * zero Dietz denominator).
 */
export function formatAccountRowYtdPerformancePct(pct: number | null): string {
  if (pct === null) return "—";
  const sign = pct >= 0 ? "+" : "";
  return `${sign}${microToFormatted(pct, 2)}%`;
}

/**
 * F27 — Maps any asset-price fetch error (per-account or all-accounts) to a
 * snackbar message + severity. Pure function, no React, no useTranslation.
 *
 * Covers `FetchAccountAssetPricesError | FetchAllAssetPricesError` — both
 * compose AssetError + AccountError + FetchPriceTask on the wire.
 *
 * reviewer-arch FP: severity is intentionally narrower than SnackbarVariant
 * (no "success") because an error presenter never returns success — the narrow
 * union documents that constraint at the type level. See PR #NN.
 */
export function fetchPriceErrorToI18n(
  err: FetchAccountAssetPricesError | FetchAllAssetPricesError,
): SnackbarMessage {
  switch (err.code) {
    case "FetchAlreadyRunning":
      return { key: "mkt.fetch_already_running", severity: "info" };
    case "NoFetchableHoldings":
      return { key: "mkt.fetch_no_holdings", severity: "info" };
    case "AccountNotFound":
      return { key: "error.AccountNotFound", severity: "error" };
    case "NameAlreadyExists":
    case "DatabaseError":
    case "UnknownError":
      return { key: "error.DatabaseError", severity: "error" };
    default:
      return { key: "error.Unknown", severity: "error" };
  }
}

/**
 * F27 — Maps any account-BC mutation error (add / update / delete / deletion-summary)
 * to an i18n key + interpolation vars. Pure function, no React, no useTranslation.
 *
 * Covers the account-BC mutation surface (add / update / delete and the
 * pre-deletion summary lookup), all typed `AccountError`. Lists the reachable
 * codes and falls back to `error.Unknown` for the rest of the BC-wide union.
 */
export function accountMutationErrorToI18n(err: AccountError): I18nMessage {
  switch (err.code) {
    case "InvalidCurrency":
      return { key: "error.InvalidCurrency", vars: { currency: err.currency } };
    case "NameEmpty":
    case "AccountNotFound":
    case "NameAlreadyExists":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

// i18n keys for UpdateFrequency display labels
export const FREQUENCY_I18N_KEYS: Record<UpdateFrequency, string> = {
  Automatic: "account.frequency_automatic",
  ManualDay: "account.frequency_manual_day",
  ManualWeek: "account.frequency_manual_week",
  ManualMonth: "account.frequency_manual_month",
  ManualYear: "account.frequency_manual_year",
};

// Ordered list of all frequencies — derived from FREQUENCY_I18N_KEYS to stay in sync with Specta bindings
export const FREQUENCIES = Object.keys(FREQUENCY_I18N_KEYS) as UpdateFrequency[];

// R9 — logical sort order for UpdateFrequency (not alphabetical)
export const FREQUENCY_ORDER: Record<UpdateFrequency, number> = {
  Automatic: 0,
  ManualDay: 1,
  ManualWeek: 2,
  ManualMonth: 3,
  ManualYear: 4,
};

/**
 * PMV-021/022/034/040 — a value in micros as a 2-decimal display string. The
 * currency is a separate field the dialog renders alongside it, never combined
 * here: a row is in its account's currency (PMV-034), the total in the
 * report's reference currency (PMV-040).
 */
export function formatPriceMovementValue(micros: number): string {
  return microToFormatted(micros, 2);
}

/**
 * PMV-024 — a movement as a signed percentage. `null` renders as an em dash:
 * the backend decides whether a proportion exists at all (unmoved PMV-031/045,
 * or a non-positive earlier value PMV-025/044), and the presentation never
 * re-derives that judgement from the two values.
 */
export function formatPriceMovementPct(microPercent: number | null): string {
  if (microPercent === null) return "—";
  const sign = microPercent >= 0 ? "+" : "";
  return `${sign}${microToFormatted(microPercent, 2)}%`;
}

/**
 * PMV-027/046 — a movement as a signed amount in the currency it is expressed in.
 * `null` renders as an em dash: the backend reports no amount when the two
 * values are equal (PMV-031/045).
 */
export function formatPriceMovementAmount(micros: number | null, currency: string): string {
  if (micros === null) return "—";
  const sign = micros >= 0 ? "+" : "";
  return `${sign}${microToFormatted(micros, 2)} ${currency}`;
}

/**
 * PMV-053 — a value column's observation date in the display locale, or
 * an em dash when that side of the report carries no date. The dates are used
 * exactly as the backend gave them; nothing is compared or derived here.
 */
export function formatPriceMovementDate(isoDate: string | null, locale: string): string {
  return isoDate === null ? "—" : formatIsoDateNumeric(isoDate, locale);
}
