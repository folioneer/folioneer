import { describe, expect, it } from "vitest";
import commitRules from "../../../docs/commit-rules.md?raw";
import en from "../../i18n/locales/en/common.json";
import fr from "../../i18n/locales/fr/common.json";
import {
  APPLICATION_AREAS,
  applicationAreaLabelKey,
  extractSectionFor,
  extractSectionsBetween,
  splitApplicationAreaTag,
} from "./parseChangelog";

const CHANGELOG_FIXTURE = `# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.34.0] - 2026-07-05

### Added

- newest feature with a [link](https://example.com) and **bold** text

## [0.33.0] - 2026-07-04

### Fixed

- a fix headline
  Continuation line describing the \`fix\` in detail.

### Changed

- a behavior change

## [0.32.0] - 2026-07-01

### Added

- older feature
`;

describe("extractSectionsBetween", () => {
  it("returns sections strictly newer than afterVersion up to throughVersion, newest first", () => {
    const sections = extractSectionsBetween(CHANGELOG_FIXTURE, "0.32.0", "0.34.0");
    expect(sections.map((section) => section.version)).toEqual(["0.34.0", "0.33.0"]);
    expect(sections[0]?.date).toBe("2026-07-05");
  });

  it("returns [] when the stored version equals the current version", () => {
    expect(extractSectionsBetween(CHANGELOG_FIXTURE, "0.34.0", "0.34.0")).toEqual([]);
  });

  it("excludes sections newer than throughVersion", () => {
    const sections = extractSectionsBetween(CHANGELOG_FIXTURE, "0.32.0", "0.33.0");
    expect(sections.map((section) => section.version)).toEqual(["0.33.0"]);
  });

  it("tolerates a stored version absent from the changelog", () => {
    const sections = extractSectionsBetween(CHANGELOG_FIXTURE, "0.32.5", "0.34.0");
    expect(sections.map((section) => section.version)).toEqual(["0.34.0", "0.33.0"]);
  });

  it("treats a missing patch segment as zero", () => {
    const sections = extractSectionsBetween(CHANGELOG_FIXTURE, "0.33", "0.34.0");
    expect(sections.map((section) => section.version)).toEqual(["0.34.0"]);
  });

  it("ignores the [Unreleased] section", () => {
    const sections = extractSectionsBetween(CHANGELOG_FIXTURE, "0.0.0", "99.0.0");
    expect(sections.map((section) => section.version)).toEqual(["0.34.0", "0.33.0", "0.32.0"]);
  });

  it("returns [] on malformed changelog text", () => {
    expect(extractSectionsBetween("not a changelog at all", "0.32.0", "0.34.0")).toEqual([]);
  });

  it("returns [] when a version bound is not parseable", () => {
    expect(extractSectionsBetween(CHANGELOG_FIXTURE, "garbage", "0.34.0")).toEqual([]);
    expect(extractSectionsBetween(CHANGELOG_FIXTURE, "0.32.0", "")).toEqual([]);
  });

  it("sorts newest first even when the file lists versions oldest first", () => {
    const oldestFirst = `## [0.1.0] - 2026-01-01\n\n- first\n\n## [0.2.0] - 2026-02-01\n\n- second\n`;
    const sections = extractSectionsBetween(oldestFirst, "0.0.0", "0.2.0");
    expect(sections.map((section) => section.version)).toEqual(["0.2.0", "0.1.0"]);
  });

  it("keeps subheads, merges bullet continuations, and strips inline markdown", () => {
    const sections = extractSectionsBetween(CHANGELOG_FIXTURE, "0.32.0", "0.34.0");
    expect(sections[0]?.body).toEqual(["### Added", "- newest feature with a link and bold text"]);
    expect(sections[1]?.body).toEqual([
      "### Fixed",
      "- a fix headline Continuation line describing the fix in detail.",
      "### Changed",
      "- a behavior change",
    ]);
  });
});

describe("extractSectionFor", () => {
  it("returns exactly the requested version's section (WNW-030)", () => {
    const sections = extractSectionFor(CHANGELOG_FIXTURE, "0.33.0");
    expect(sections.map((section) => section.version)).toEqual(["0.33.0"]);
    expect(sections[0]?.date).toBe("2026-07-04");
  });

  it("returns [] when the version has no changelog section", () => {
    expect(extractSectionFor(CHANGELOG_FIXTURE, "0.99.0")).toEqual([]);
  });

  it("returns [] when the version is not parseable", () => {
    expect(extractSectionFor(CHANGELOG_FIXTURE, "...")).toEqual([]);
  });

  it("treats a missing patch segment as zero", () => {
    expect(extractSectionFor(CHANGELOG_FIXTURE, "0.34").map((s) => s.version)).toEqual(["0.34.0"]);
  });
});

describe("splitApplicationAreaTag (WNW-090)", () => {
  it("splits a known application area tag from the sentence", () => {
    expect(splitApplicationAreaTag("[transfer] Move a holding to another account")).toEqual({
      area: "transfer",
      text: "Move a holding to another account",
    });
    expect(splitApplicationAreaTag("[command-line] Record a price from a terminal").area).toBe(
      "command-line",
    );
  });

  it("leaves a line with no tag as written (WNW-091)", () => {
    expect(splitApplicationAreaTag("A refused sale now says how much you held")).toEqual({
      area: null,
      text: "A refused sale now says how much you held",
    });
  });

  it("reads a tag only when it is exact, lowercase and followed by a space", () => {
    expect(splitApplicationAreaTag("[Transfer] Move a holding").area).toBeNull();
    expect(splitApplicationAreaTag("[agent][transfer] Move a holding").area).toBeNull();
  });

  it("drops every space after the tag and keeps a second tag in the sentence (WNW-090)", () => {
    expect(splitApplicationAreaTag("[transfer]   Move a holding").text).toBe("Move a holding");
    expect(splitApplicationAreaTag("[transfer] [price] Move a holding")).toEqual({
      area: "transfer",
      text: "[price] Move a holding",
    });
  });

  it("leaves a tag with no sentence as written (WNW-091)", () => {
    expect(splitApplicationAreaTag("[transfer] ")).toEqual({ area: null, text: "[transfer] " });
  });

  it("leaves a line whose tag is not an application area as written (WNW-091)", () => {
    expect(splitApplicationAreaTag("[deps] bump a crate")).toEqual({
      area: null,
      text: "[deps] bump a crate",
    });
  });

  it("knows exactly the scopes of docs/commit-rules.md § Scope", () => {
    const table = commitRules.split("<!-- scopes:start -->")[1]?.split("<!-- scopes:end -->")[0];
    const scopes = [...(table ?? "").matchAll(/^\| *`([a-z-]+)`/gm)].map((match) => match[1]);
    expect(scopes.length).toBeGreaterThan(0);
    expect([...APPLICATION_AREAS].sort()).toEqual(scopes.sort());
  });

  it("names every application area in English and French (WNW-092)", () => {
    for (const area of APPLICATION_AREAS) {
      const key = applicationAreaLabelKey(area).replace("whats_new.application_areas.", "");
      expect(
        (en.whats_new.application_areas as Record<string, string>)[key],
        `en ${area}`,
      ).toBeTruthy();
      expect(
        (fr.whats_new.application_areas as Record<string, string>)[key],
        `fr ${area}`,
      ).toBeTruthy();
    }
  });
});
