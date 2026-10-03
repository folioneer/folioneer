import { describe, expect, it } from "vitest";
import { ASSET_CREATION_DEFAULTS } from "@/bindings";
import { RISK_LEVELS } from "./constants";
import {
  ADDABLE_ASSET_CLASSES,
  DEFAULT_ASSET_CLASS,
  DEFAULT_CATEGORY_ID,
  defaultRiskOf,
} from "./creationDefaults";

describe("creationDefaults", () => {
  // CSH-015 — the classes offered are the core's: never Cash, and the preselected one is among them.
  it("offers the core's classes, never Cash", () => {
    expect(ADDABLE_ASSET_CLASSES).not.toContain("Cash");
    expect(ADDABLE_ASSET_CLASSES).toContain(DEFAULT_ASSET_CLASS);
    expect(DEFAULT_CATEGORY_ID).toBe(ASSET_CREATION_DEFAULTS.category_id);
  });

  // R3 — every class offered has a risk level of the scale; a class the core does not offer
  // takes the level the core preselects, never one decided here.
  it("reads each class's default risk level from the core", () => {
    for (const assetClass of ADDABLE_ASSET_CLASSES) {
      expect(RISK_LEVELS).toContain(defaultRiskOf(assetClass));
    }
    expect(defaultRiskOf(DEFAULT_ASSET_CLASS)).toBe(ASSET_CREATION_DEFAULTS.risk_level);
    expect(defaultRiskOf("Cash")).toBe(ASSET_CREATION_DEFAULTS.risk_level);
  });
});
