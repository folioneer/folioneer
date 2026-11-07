import { describe, expect, it } from "vitest";

/** Every source under src that can name a class: tests, previews and the generated bindings aside. */
const SOURCES = import.meta.glob<string>(
  ["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}", "!/src/__preview__/**", "!/src/bindings.ts"],
  { query: "?raw", import: "default", eager: true },
);

// The neutral scale has no dark values (`.dark` in global.css redefines only the m3 tokens),
// so a class drawn from it reads light-on-light or dark-on-dark in one of the two themes.
const UNTHEMED = /\b(?:[a-z-]+:)*(?:text|bg|border|divide|ring|outline)-neutral-\d+\b/g;

describe("theme tokens (#713)", () => {
  it("reads the component sources", () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(100);
  });

  it("no component colours itself from the unthemed neutral scale", () => {
    const found = Object.entries(SOURCES).flatMap(([file, source]) =>
      [...source.matchAll(UNTHEMED)].map((match) => `${file}: ${match[0]}`),
    );
    expect(found).toEqual([]);
  });
});
