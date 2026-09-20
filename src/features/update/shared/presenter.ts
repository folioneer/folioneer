import type { UpdateError } from "@/lib/updateGateway";

/** Maps an update error to the i18n key of the message the banner shows. */
export function presentUpdateError(error: UpdateError): string {
  switch (error.code) {
    case "AccessRefused":
      return "update.error_access_refused";
    default:
      return "update.error";
  }
}

/** Whether trying again can succeed: a refused access cannot, until the application restarts. */
export function isRetryable(error: UpdateError): boolean {
  return error.code !== "AccessRefused";
}
