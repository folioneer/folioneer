import { describe, expect, it } from "vitest";
import { extractSectionFor, extractSectionsBetween } from "./parseChangelog";

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
