import { afterEach, describe, expect, it } from "vitest";
import { getWhatsNewLastSeenVersion, setWhatsNewLastSeenVersion } from "./whatsNewStorage";

describe("whatsNewStorage", () => {
  afterEach(() => localStorage.clear());

  it("returns null on a fresh install (nothing stored)", () => {
    expect(getWhatsNewLastSeenVersion()).toBeNull();
  });

  it("round-trips a stored version", () => {
    setWhatsNewLastSeenVersion("0.33.2");
    expect(getWhatsNewLastSeenVersion()).toBe("0.33.2");
  });

  it("overwrites a stored version (last write wins)", () => {
    setWhatsNewLastSeenVersion("0.33.2");
    setWhatsNewLastSeenVersion("0.34.0");
    expect(getWhatsNewLastSeenVersion()).toBe("0.34.0");
  });
});
