import type {
  AccountError,
  FetchAccountAssetPricesError,
  FetchAllAssetPricesError,
  Wrapper,
  WrapperLists,
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
    case "NoExternalProvider":
      return { key: "error.NoExternalProvider", severity: "info" };
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
    // ACC-041 — the account's country or wrapper it cannot take.
    case "InvalidCountry":
      return { key: "error.InvalidCountry", vars: { country: err.country } };
    case "WrapperNotAvailable":
      return { key: "error.WrapperNotAvailable", vars: { wrapper: err.wrapper } };
    case "NameEmpty":
    case "AccountNotFound":
    case "NameAlreadyExists":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

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

/** The label of a wrapper in the window's language: French, German, else English (ACC-040). */
function labelIn(wrapper: Wrapper, language: string): string {
  if (language.startsWith("fr")) return wrapper.label_fr;
  if (language.startsWith("de")) return wrapper.label_de;
  return wrapper.label_en;
}

/**
 * ACC-045/046 — a wrapper as the window names it; a wrapper the lists do not know (another
 * device's newer lists, ACC-040) is shown by its identifier. Null without a wrapper, and
 * until the lists are read.
 */
export function wrapperLabel(
  lists: WrapperLists | null,
  wrapperId: string | null,
  language: string,
): string | null {
  if (wrapperId === null || lists === null) return null;
  const wrapper = lists.wrappers.find((candidate) => candidate.id === wrapperId);
  return wrapper ? labelIn(wrapper, language) : wrapperId;
}

/**
 * ACC-048 — the wrappers the form suggests to an account of `country` ("" for the portfolio's),
 * in the order the core gives them: its country's list, then the generic one; the account's
 * `current` wrapper, when it is from another list or one the lists do not know, follows them,
 * so an edit keeps it (ACC-043).
 */
export function wrapperOptions(
  lists: WrapperLists,
  country: string,
  current: string,
  language: string,
): { value: string; label: string }[] {
  const effective = country || lists.portfolio_country;
  const ids =
    lists.offered.find((offer) => offer.country === effective)?.wrapper_ids ?? lists.generic;
  const suggested = current === "" || ids.includes(current) ? ids : [...ids, current];
  return suggested.map((id) => ({ value: id, label: wrapperLabel(lists, id, language) ?? id }));
}

/** A country's name in the window's language, by its two-letter code. */
function countryName(code: string, language: string): string {
  return new Intl.DisplayNames([language], { type: "region" }).of(code) ?? code;
}

/**
 * ACC-045 — the countries the form offers, every one, in the core's order: the portfolio's
 * first (value "", the account takes the portfolio's, ACC-041), named in `country` for its
 * label; then the countries with a list of their own; then the others.
 */
export function countryOptions(
  lists: WrapperLists,
  language: string,
): { value: string; label: string; country: string }[] {
  return lists.countries.map((code) => {
    const name = countryName(code, language);
    return { value: code === lists.portfolio_country ? "" : code, label: name, country: name };
  });
}

/**
 * ACC-047 — rows grouped by wrapper in the lists' order (the core's), wrappers the lists do
 * not know after them in the order the rows meet them, the rows without a wrapper last
 * (wrapper and label null); rows keep their order within a group. A group's heading is its
 * wrapper's label, or the identifier of one the lists do not know.
 */
export function groupByWrapper<T extends { wrapper: string | null }>(
  rows: T[],
  lists: WrapperLists | null,
  labelOf: (wrapperId: string | null) => string | null,
): { wrapper: string | null; label: string | null; rows: T[] }[] {
  const buckets = new Map<string | null, T[]>();
  for (const wrapper of lists?.wrappers ?? []) buckets.set(wrapper.id, []);
  const unknown = new Map<string | null, T[]>();
  for (const row of rows) {
    const bucket = buckets.get(row.wrapper) ?? unknown.get(row.wrapper) ?? [];
    if (!buckets.has(row.wrapper)) unknown.set(row.wrapper, bucket);
    bucket.push(row);
  }
  const groups: { wrapper: string | null; label: string | null; rows: T[] }[] = [];
  const withoutWrapper = unknown.get(null);
  unknown.delete(null);
  for (const [wrapper, groupRows] of [...buckets, ...unknown]) {
    if (groupRows.length > 0) groups.push({ wrapper, label: labelOf(wrapper), rows: groupRows });
  }
  if (withoutWrapper) groups.push({ wrapper: null, label: null, rows: withoutWrapper });
  return groups;
}
