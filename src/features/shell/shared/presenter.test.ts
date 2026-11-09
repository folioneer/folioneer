import { describe, expect, it } from "vitest";
import { profileErrorToI18n } from "./presenter";

describe("shell presenter", () => {
  // F27 — a known refusal its message; anything else the generic one.
  it("maps a refused read or choice of the profiles to its message", () => {
    expect(profileErrorToI18n({ code: "ProfileNotFound", profile_id: "p-1" })).toEqual({
      key: "error.ProfileNotFound",
    });
    expect(profileErrorToI18n({ code: "NameEmpty" })).toEqual({ key: "error.Unknown" });
  });
});
