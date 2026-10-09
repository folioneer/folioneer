import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { isDateValid, isPriceValid } from "./validatePriceForm";

describe("isPriceValid", () => {
  it("accepts a positive number and nothing else", () => {
    expect(isPriceValid("12.5")).toBe(true);
    expect(isPriceValid("")).toBe(false);
    expect(isPriceValid("0")).toBe(false);
    expect(isPriceValid("abc")).toBe(false);
  });
});

describe("isDateValid just after local midnight", () => {
  // 00:30 in Paris on 9 October is still the 8th in UTC.
  beforeEach(() => {
    vi.stubEnv("TZ", "Europe/Paris");
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-08T22:30:00Z"));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllEnvs();
  });

  it("accepts today as the user lives it and refuses tomorrow or a malformed date", () => {
    expect(isDateValid("2026-10-09")).toBe(true);
    expect(isDateValid("2026-10-10")).toBe(false);
    expect(isDateValid("09/10/2026")).toBe(false);
    expect(isDateValid("")).toBe(false);
  });
});
