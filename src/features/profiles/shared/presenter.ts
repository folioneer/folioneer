import type { AccountError, Profile } from "@/bindings";
import { microToFormatted } from "@/lib/microUnits";
import type { I18nMessage } from "@/ui/format/i18n";

/**
 * PFL-011 — a profile's name as shown: the default profile has none until named, and reads
 * as `defaultName` (the window's language). Pure: the component passes the translated name.
 */
export function profileDisplayName(profile: Profile, defaultName: string): string {
  return profile.name ?? defaultName;
}

/** PFL-023 — how many accounts a profile holds, as a message. */
export function accountCountMessage(count: number): I18nMessage {
  return count === 0
    ? { key: "profile.manage.no_account" }
    : { key: "profile.manage.account_count", vars: { count } };
}

/**
 * F27 — a profile action's error (create, rename, remove, choose) as a message. Pure.
 */
export function profileErrorToI18n(err: AccountError): I18nMessage {
  switch (err.code) {
    case "ProfileNameEmpty":
    case "ProfileNameAlreadyExists":
    case "ProfileNotFound":
    case "ProfileNotEmpty":
    case "DefaultProfileKept":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}

/** PFL-055 — a profile's value (reference-currency micros) to 2 decimals. */
export function overviewValueText(micros: number): string {
  return microToFormatted(micros, 2);
}

/** PFL-055 — a profile's performance this year as a signed percentage; "—" without one. */
export function overviewYtdText(pct: number | null): string {
  if (pct === null) return "—";
  return `${pct >= 0 ? "+" : ""}${microToFormatted(pct, 2)}%`;
}

/** PFL-055 — the tone of a performance: none, a loss or a gain. */
export function overviewYtdTone(pct: number | null): "neutral" | "loss" | "gain" {
  if (pct === null || pct === 0) return "neutral";
  return pct < 0 ? "loss" : "gain";
}
