import type { AccountError, Profile } from "@/bindings";
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
