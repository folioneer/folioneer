import { describe, expect, it } from "vitest";
import { formatIsoDateNumeric, formatIsoDateTime, formatIsoDateTimeNumeric } from "./date";

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
