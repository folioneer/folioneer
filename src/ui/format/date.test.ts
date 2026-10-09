import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  formatIsoDateNumeric,
  formatIsoDateTime,
  formatIsoDateTimeNumeric,
  todayIso,
} from "./date";

/** The text of every source file of the application, by path. */
const SOURCES = import.meta.glob<string>("../../**/*.{ts,tsx}", {
  query: "?raw",
  import: "default",
  eager: true,
});

describe("todayIso", () => {
  // 00:30 in Paris on 9 October is still 22:30 on the 8th in UTC: the half hour in which
  // the UTC day is yesterday.
  const justAfterMidnightInParis = new Date("2026-10-08T22:30:00Z");

  beforeEach(() => {
    vi.stubEnv("TZ", "Europe/Paris");
    vi.useFakeTimers();
    vi.setSystemTime(justAfterMidnightInParis);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllEnvs();
  });

  it("is the day the user is living, not the UTC day", () => {
    expect(justAfterMidnightInParis.toISOString().slice(0, 10)).toBe("2026-10-08");
    expect(todayIso()).toBe("2026-10-09");
  });

  it("pads the month and the day", () => {
    expect(todayIso(new Date(2026, 0, 5, 12))).toBe("2026-01-05");
  });

  it("is the only way the interface reads today's date", () => {
    const utcDay = /toISOString\(\)\s*\.(slice\(0, 10\)|split\("T"\))/;
    const sources = Object.entries(SOURCES).filter(([path]) => !/\.test\.tsx?$/.test(path));
    expect(sources.length).toBeGreaterThan(100);
    const offenders = sources.filter(([, text]) => utcDay.test(text)).map(([path]) => path);
    expect(offenders).toEqual([]);
  });
});

describe("formatIsoDateNumeric", () => {
  it("formats an ISO date as French numeric DD/MM/YYYY", () => {
    expect(formatIsoDateNumeric("2026-06-14", "fr")).toBe("14/06/2026");
  });

  it("formats an ISO date as US numeric M/D/YYYY", () => {
    expect(formatIsoDateNumeric("2026-06-14", "en")).toBe("6/14/2026");
  });

  it("does not shift the day across a timezone offset (noon anchor)", () => {
    expect(formatIsoDateNumeric("2026-01-01", "fr")).toBe("01/01/2026");
    expect(formatIsoDateNumeric("2026-12-31", "fr")).toBe("31/12/2026");
  });

  it("returns the raw input unchanged when it does not parse", () => {
    expect(formatIsoDateNumeric("not-a-date", "fr")).toBe("not-a-date");
  });
});

describe("formatIsoDateTime", () => {
  it("formats an ISO date-time as a French medium date + short time", () => {
    expect(formatIsoDateTime("2026-07-12T19:00:12", "fr")).toBe("12 juil. 2026, 19:00");
  });

  it("formats an ISO date-time as a US medium date + short time", () => {
    expect(formatIsoDateTime("2026-07-12T19:00:12", "en")).toBe("Jul 12, 2026, 7:00 PM");
  });

  it("returns the raw input unchanged when it does not parse", () => {
    expect(formatIsoDateTime("not-a-timestamp", "fr")).toBe("not-a-timestamp");
  });
});

describe("formatIsoDateTimeNumeric", () => {
  it("formats an ISO date-time as a French numeric date + short time", () => {
    expect(formatIsoDateTimeNumeric("2026-09-19T08:14:03", "fr")).toBe("19/09/2026 08:14");
  });

  it("formats an ISO date-time as a US numeric date + short time", () => {
    expect(formatIsoDateTimeNumeric("2026-09-19T08:14:03", "en")).toBe("9/19/2026 8:14 AM");
  });

  it("returns the raw input unchanged when it does not parse", () => {
    expect(formatIsoDateTimeNumeric("not-a-timestamp", "fr")).toBe("not-a-timestamp");
  });
});
