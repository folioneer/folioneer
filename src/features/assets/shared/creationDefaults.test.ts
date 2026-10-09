import { describe, expect, it } from "vitest";
import { ASSET_CREATION_DEFAULTS } from "@/bindings";
import {
  classForKind,
  DEFAULT_ASSET_KIND,
  DEFAULT_CATEGORY_ID,
  defaultRiskOf,
  KIND_FORMS,
  kindFormOf,
  RISK_LEVELS,
} from "./creationDefaults";

describe("creationDefaults", () => {
  // AST-037 / CSH-015 — the kinds and their classes are the core's: cash is never offered,
  // neither as a kind nor as a class, and the kind preselected is among them.
  it("offers the core's kinds and classes, never cash", () => {
    expect(KIND_FORMS.map((form) => form.kind)).not.toContain("Cash");
    expect(kindFormOf(DEFAULT_ASSET_KIND)).not.toBeNull();
    expect(kindFormOf("Cash")).toBeNull();
    for (const form of KIND_FORMS) {
      const classes = form.classes.map((entry) => entry.class as string);
      expect(classes).not.toContain("Cash");
      expect(classes).toContain(form.class);
    }
    expect(DEFAULT_CATEGORY_ID).toBe(ASSET_CREATION_DEFAULTS.category_id);
  });

  // AST-037 — a class the kind offers is kept; any other gives way to the class the core
  // preselects for that kind.
  it("keeps a class the kind offers and replaces any other by the core's", () => {
    for (const form of KIND_FORMS) {
      for (const entry of form.classes) {
        expect(classForKind(form.kind, entry.class)).toBe(entry.class);
      }
      expect(classForKind(form.kind, "Cash")).toBe(form.class);
      expect(classForKind(form.kind, null)).toBe(form.class);
    }
    expect(classForKind("Cash", "Cash")).toBe("Cash");
  });

  // R3 — every class offered has a risk level of the scale; a class the core does not offer
  // takes the level the core preselects, never one decided here.
  it("reads each class's default risk level from the core", () => {
    for (const form of KIND_FORMS) {
      for (const entry of form.classes) {
        expect(RISK_LEVELS).toContain(defaultRiskOf(entry.class));
        expect(defaultRiskOf(entry.class)).toBe(entry.default_risk);
      }
    }
    expect(defaultRiskOf("Cash")).toBe(ASSET_CREATION_DEFAULTS.risk_level);
  });
});
