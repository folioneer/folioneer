import i18next from "i18next";
import { describe, expect, it } from "vitest";
import en from "./locales/en/common.json";
import fr from "./locales/fr/common.json";

/**
 * Recursively collects every leaf key path under a non-empty object `obj`
 * (e.g. `sync.errors.SyncPaused`). An empty object at any level collects
 * nothing for that branch — this only walks objects that already exist, it
 * does not manufacture a fake leaf for a missing/undefined namespace.
 */
function collectKeyPaths(obj: Record<string, unknown>, prefix: string): string[] {
  return Object.entries(obj).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === "object") {
      return collectKeyPaths(value as Record<string, unknown>, path);
    }
    return [path];
  });
}

// SYN — every locale must declare the same `sync.*` key set (i18n-rules.md).
// A locale-only PR (this one) is the only place both files change together, so
// this test is where a drifted key set would first be caught.
describe("i18n locales — sync.* key parity (fr/en)", () => {
  it("declares the sync.* namespace as a non-empty object in each locale", () => {
    const enSync = (en as Record<string, unknown>).sync;
    const frSync = (fr as Record<string, unknown>).sync;

    expect(enSync).toBeTypeOf("object");
    expect(frSync).toBeTypeOf("object");
    expect(enSync).not.toBeNull();
    expect(frSync).not.toBeNull();
  });

  it("fr and en declare the identical sync.* key set", () => {
    const enSync = ((en as Record<string, unknown>).sync ?? {}) as Record<string, unknown>;
    const frSync = ((fr as Record<string, unknown>).sync ?? {}) as Record<string, unknown>;

    const enSyncKeys = collectKeyPaths(enSync, "sync").sort();
    const frSyncKeys = collectKeyPaths(frSync, "sync").sort();

    expect(enSyncKeys.length).toBeGreaterThan(0);
    expect(frSyncKeys).toEqual(enSyncKeys);
  });
});

// SYN-019/069 — the folder problem nested inside the folder-unavailable sentence resolves
// through i18next's interpolate-then-nest order, in both locales.
describe("i18n locales — folder problem nesting", () => {
  it.each([
    [
      "en",
      en,
      "The shared folder is unavailable: The folder's drive is not mounted. Reconnect it, then sync again or resume.",
    ],
    [
      "fr",
      fr,
      "Le dossier partagé est indisponible : Le disque du dossier n'est pas monté. Reconnectez-le, puis relancez la synchronisation ou reprenez.",
    ],
  ])("resolves sync.errors.FolderUnavailable in %s", async (lng, resources, expected) => {
    const instance = i18next.createInstance();
    await instance.init({
      lng,
      resources: { [lng]: { translation: resources } },
      interpolation: { escapeValue: false },
    });

    expect(instance.t("sync.errors.FolderUnavailable", { problem: "Unmounted" })).toBe(expected);
  });
});

// #004 — the price-age and FX-rate-age labels on the holding row carry the age
// alone; the same day reads as zero days so every row lines up.
describe("i18n locales — staleness labels carry the age alone (#004)", () => {
  it.each([
    ["en", en, "0d", "{{days}}d"],
    ["fr", fr, "0 j", "{{days}} j"],
  ])("%s: price and FX-rate age", (_lang, locale, today, daysAgo) => {
    expect(locale.mkt.staleness_today).toBe(today);
    expect(locale.mkt.staleness_days_ago).toBe(daysAgo);
    expect(locale.currency.rate_staleness_today).toBe(today);
    expect(locale.currency.rate_staleness_days_old).toBe(daysAgo);
  });
});
