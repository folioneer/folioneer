import type { AccountError, Profile } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/**
 * PFL-011 — a profile's name as the chip shows it: the default profile has none until named,
 * and reads as `defaultName` (the window's language). Pure.
 */
export function chipProfileName(profile: Profile | null, defaultName: string): string {
  return profile?.name ?? defaultName;
}

/** F27 — a refused read or choice of the profile in use, as a message. Pure. */
export function profileChipErrorToI18n(err: AccountError): I18nMessage {
  switch (err.code) {
    case "ProfileNotFound":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}
