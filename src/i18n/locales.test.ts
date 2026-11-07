import i18next from "i18next";
import { describe, expect, it } from "vitest";
import de from "./locales/de/common.json";
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

// #390 — every language carries every key of the English one, and no other (i18n-rules.md).
describe("i18n locales — every key in every language", () => {
  const english = collectKeyPaths(en as Record<string, unknown>, "").sort();
  it.each([
    ["fr", fr],
    ["de", de],
  ])("%s has exactly the English keys", (_lang, locale) => {
    expect(collectKeyPaths(locale as Record<string, unknown>, "").sort()).toEqual(english);
  });

  // Every value keeps the placeholders and nested keys of its English one.
  it.each([
    ["fr", fr],
    ["de", de],
  ])("%s keeps every placeholder", (_lang, locale) => {
    const marks = (text: string) => (text.match(/\{\{\s*\w+\s*\}\}|\$t\([^)]*\)/g) ?? []).sort();
    const valueAt = (tree: unknown, path: string) =>
      path.split(".").reduce<unknown>((node, key) => (node as Record<string, unknown>)[key], tree);
    for (const path of english) {
      expect([path, marks(String(valueAt(locale, path)))]).toEqual([
        path,
        marks(String(valueAt(en, path))),
      ]);
    }
  });

  it("German resolves its interpolations", async () => {
    const instance = i18next.createInstance();
    await instance.init({
      lng: "de",
      resources: { de: { translation: de } },
      interpolation: { escapeValue: false },
    });
    expect(instance.t("account_performance.period_current_year", { year: 2026 })).toBe(
      "2026 (YTD)",
    );
  });
});

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

// TODO-004 — the price-age and FX-rate-age labels on the holding row carry the age
// alone; the same day reads as zero days so every row lines up.
describe("i18n locales — staleness labels carry the age alone (TODO-004)", () => {
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

// PRF-036 / PRF-044 — the gain column and the current year's row, in every language.
describe("i18n locales — the performance table names its gain and its running year", () => {
  it.each([
    ["en", en, "Gain for the period"],
    ["fr", fr, "Gain de la période"],
  ])("%s", async (lng, resources, gainColumn) => {
    const instance = i18next.createInstance();
    await instance.init({
      lng,
      resources: { [lng]: { translation: resources } },
      interpolation: { escapeValue: false },
    });

    expect(instance.t("account_performance.column_period_over_period")).toBe(gainColumn);
    expect(instance.t("account_performance.period_current_year", { year: 2026 })).toBe(
      "2026 (YTD)",
    );
  });
});

function collectValues(obj: Record<string, unknown>): string[] {
  return Object.values(obj).flatMap((value) =>
    value !== null && typeof value === "object"
      ? collectValues(value as Record<string, unknown>)
      : [String(value)],
  );
}

// #673 — a reader may picture a person behind "agent": every string says "AI agent" /
// "agent IA" (the `{{agent}}` placeholder is the program's own name, not the word).
describe("i18n locales — an agent is always an AI agent", () => {
  it("never says agent alone in English", () => {
    const bare = collectValues(en)
      .map((text) => text.replaceAll("{{agent}}", ""))
      .filter((text) => /(?<!AI )\bagents?\b/i.test(text));
    expect(bare).toEqual([]);
  });

  it("never says agent alone in French", () => {
    const bare = collectValues(fr)
      .map((text) => text.replaceAll("{{agent}}", ""))
      .filter((text) => /\bagents?\b(?! IA)/i.test(text));
    expect(bare).toEqual([]);
  });
});
