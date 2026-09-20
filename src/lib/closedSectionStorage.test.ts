import { afterEach, describe, expect, it } from "vitest";
import { getClosedSectionOpen, setClosedSectionOpen } from "./closedSectionStorage";

describe("closedSectionStorage", () => {
  afterEach(() => localStorage.clear());

  it("defaults to open when no preference is stored for the account", () => {
    expect(getClosedSectionOpen("acc-1")).toBe(true);
  });

  it("defaults to open for an empty account id", () => {
    expect(getClosedSectionOpen("")).toBe(true);
  });

  it("round-trips a stored fold state per account", () => {
    setClosedSectionOpen("acc-1", false);
    setClosedSectionOpen("acc-2", true);
    expect(getClosedSectionOpen("acc-1")).toBe(false);
    expect(getClosedSectionOpen("acc-2")).toBe(true);
  });

  it("ignores a write with an empty account id", () => {
    setClosedSectionOpen("", false);
    expect(getClosedSectionOpen("")).toBe(true);
  });

  it('treats any non-"true" stored value as collapsed', () => {
    localStorage.setItem("closed_section_open_acc-1", "false");
    expect(getClosedSectionOpen("acc-1")).toBe(false);
  });
});
