import { describe, expect, it } from "vitest";
import { isRetryable, presentUpdateError } from "./presenter";

describe("presentUpdateError", () => {
  // UPD-028 — a refused access has its own message: retrying cannot fix it.
  it("names a refused access", () => {
    expect(presentUpdateError({ code: "AccessRefused" })).toBe("update.error_access_refused");
  });

  // UPD-023 — every other failure keeps the generic download message.
  it("keeps the generic message for any other failure", () => {
    expect(presentUpdateError({ code: "OperationFailed" })).toBe("update.error");
    expect(presentUpdateError({ code: "NoDownloadedUpdate" })).toBe("update.error");
  });
});

describe("isRetryable", () => {
  // UPD-029 — trying again cannot lift a refusal; any other failure can be retried (UPD-024).
  it("is false for a refused access only", () => {
    expect(isRetryable({ code: "AccessRefused" })).toBe(false);
    expect(isRetryable({ code: "OperationFailed" })).toBe(true);
  });
});
