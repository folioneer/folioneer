import type { WebLookupError } from "@/bindings";

/**
 * Maps a `WebLookupError` to the i18n key for its inline copy.
 * Pure function — call `t(key)` at render time (F27).
 */
export function presentWebLookupError(error: WebLookupError): string {
  switch (error.code) {
    case "InvalidIsinFormat":
      return "asset.web_lookup.error_invalid_isin";
    case "RateLimited":
      return "asset.web_lookup.error_rate_limit";
    case "NetworkError":
      return "asset.web_lookup.error_network";
  }
}
