import { describe, expect, it } from "vitest";
import { chipProfileName, profileChipErrorToI18n } from "./presenter";

describe("profile chip presenter", () => {
  // PFL-011 — the default profile, unnamed, reads in the window's language.
  it("names the profile in use, the unnamed default in the window's language", () => {
    expect(chipProfileName({ id: "default-profile", name: null }, "Me")).toBe("Me");
    expect(chipProfileName({ id: "p-1", name: "Parent" }, "Me")).toBe("Parent");
    expect(chipProfileName(null, "Me")).toBe("Me");
  });

  // F27 — a known refusal its message; anything else the generic one.
  it("maps a refused choice to its message", () => {
    expect(profileChipErrorToI18n({ code: "ProfileNotFound", profile_id: "p-1" })).toEqual({
      key: "error.ProfileNotFound",
    });
    expect(profileChipErrorToI18n({ code: "NameEmpty" })).toEqual({ key: "error.Unknown" });
  });
});
