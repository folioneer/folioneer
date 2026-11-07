import { afterEach, describe, expect, it, vi } from "vitest";
import { resolveBrowserLang } from "./config";

// NUM-002 / #390 — with no choice in the settings, the system's language when the window
// speaks it, English otherwise.
describe("resolveBrowserLang", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it.each([
    ["de-DE", "de"],
    ["de-AT", "de"],
    ["fr-FR", "fr"],
    ["en-GB", "en"],
    ["es-ES", "en"],
  ])("%s gives %s", (system, expected) => {
    vi.stubGlobal("navigator", { language: system });
    expect(resolveBrowserLang()).toBe(expected);
  });
});
