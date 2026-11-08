import { describe, expect, it } from "vitest";
import {
  accountCountMessage,
  overviewValueText,
  overviewYtdText,
  overviewYtdTone,
  profileDisplayName,
  profileErrorToI18n,
} from "./presenter";

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

  // PFL-055 — a value to 2 decimals; a performance signed, or "—" without one.
  it("formats a profile's value and performance this year", () => {
    expect(overviewValueText(1_000_000_000)).toMatch(/1.?000.00|1.?000,00/);
    expect(overviewYtdText(null)).toBe("—");
    expect(overviewYtdText(6_420_000)).toMatch(/^\+6[.,]42%$/);
    expect(overviewYtdText(-1_000_000)).toMatch(/^-1[.,]00%$/);
  });

  // PFL-055 — no performance or none at all is neutral; a loss and a gain have their tone.
  it("gives a performance its tone", () => {
    expect(overviewYtdTone(null)).toBe("neutral");
    expect(overviewYtdTone(0)).toBe("neutral");
    expect(overviewYtdTone(-1)).toBe("loss");
    expect(overviewYtdTone(1)).toBe("gain");
  });
});
