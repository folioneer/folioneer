import type { AccountError } from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/** F27 — a refused read or choice of the profiles, as a message. Pure. */
export function profileErrorToI18n(err: AccountError): I18nMessage {
  switch (err.code) {
    case "ProfileNotFound":
    case "DatabaseError":
      return { key: `error.${err.code}` };
    default:
      return { key: "error.Unknown" };
  }
}
