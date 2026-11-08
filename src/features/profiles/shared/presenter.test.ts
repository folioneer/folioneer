import { describe, expect, it } from "vitest";
import { accountCountMessage, profileDisplayName, profileErrorToI18n } from "./presenter";

describe("profiles presenter", () => {
  // PFL-011 — the default profile, unnamed, reads in the window's language.
  it("names the unnamed default profile in the window's language", () => {
    expect(profileDisplayName({ id: "default-profile", name: null }, "Me")).toBe("Me");
    expect(profileDisplayName({ id: "p-1", name: "Parent" }, "Me")).toBe("Parent");
  });

  // PFL-023 — no account, or how many.
  it("says how many accounts a profile holds", () => {
    expect(accountCountMessage(0)).toEqual({ key: "profile.manage.no_account" });
    expect(accountCountMessage(3)).toEqual({
      key: "profile.manage.account_count",
      vars: { count: 3 },
    });
  });

  // F27 — each refusal its own message; anything else the generic one.
  it("maps a profile refusal to its message", () => {
    expect(profileErrorToI18n({ code: "ProfileNameAlreadyExists" })).toEqual({
      key: "error.ProfileNameAlreadyExists",
    });
    expect(profileErrorToI18n({ code: "NameEmpty" })).toEqual({ key: "error.Unknown" });
  });
});
