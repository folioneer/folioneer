import { describe, expect, it } from "vitest";
import type { WebLookupError } from "@/bindings";
import { presentWebLookupError } from "./presenter";

// Pure-function tests — no mocks, no React, no t() runtime.
// The presenter maps error codes to i18n keys; the component calls t(key) at
// render time. One test per error variant (F27 + WEB-033).

describe("web lookup presenter", () => {
  // WEB-025 / WEB-033 — InvalidIsinFormat maps to the ISIN-specific copy key
  it("maps InvalidIsinFormat to asset.web_lookup.error_invalid_isin", () => {
    const error: WebLookupError = { code: "InvalidIsinFormat" };
    expect(presentWebLookupError(error)).toBe("asset.web_lookup.error_invalid_isin");
  });

  // WEB-025 / WEB-033 — RateLimited maps to the wait-and-retry copy key
  it("maps RateLimited to asset.web_lookup.error_rate_limit", () => {
    const error: WebLookupError = { code: "RateLimited" };
    expect(presentWebLookupError(error)).toBe("asset.web_lookup.error_rate_limit");
  });

  // WEB-025 / WEB-033 — NetworkError maps to the generic network copy key
  it("maps NetworkError to asset.web_lookup.error_network", () => {
    const error: WebLookupError = { code: "NetworkError" };
    expect(presentWebLookupError(error)).toBe("asset.web_lookup.error_network");
  });
});
