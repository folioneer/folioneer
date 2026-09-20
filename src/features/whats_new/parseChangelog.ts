/** One released changelog entry: `## [version] - date` plus its markdown body (WNW-040). */
export interface ChangelogSection {
  version: string;
  date: string;
  /** Plain-text body lines: `### ` subheads kept as-is, bullets merged with their continuation lines. */
  body: string[];
}

type ParsedVersion = [number, number, number];

const SECTION_HEADER_PATTERN = /^## \[([^\]]+)\](?:\s*-\s*(.*))?$/;

/** Numeric `x.y.z` (or `x.y`, patch defaulting to 0), or null when the string is not a version. */
function parseVersion(version: string): ParsedVersion | null {
  const match = /^(\d+)\.(\d+)(?:\.(\d+))?$/.exec(version.trim());
  if (!match) return null;
  return [Number(match[1]), Number(match[2]), match[3] === undefined ? 0 : Number(match[3])];
}

function compareVersions(a: ParsedVersion, b: ParsedVersion): number {
  if (a[0] !== b[0]) return a[0] - b[0];
  if (a[1] !== b[1]) return a[1] - b[1];
  return a[2] - b[2];
}

/** Drops inline markdown decoration (links, bold, italics, inline code) keeping the text content. */
function stripInlineMarkdown(text: string): string {
  return text
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1");
}

/** Folds raw markdown lines into plain-text body lines, merging bullet continuation lines. */
function buildBodyLines(rawLines: string[]): string[] {
  const body: string[] = [];
  for (const rawLine of rawLines) {
    const trimmed = rawLine.trim();
    if (trimmed === "") continue;
    const lastIndex = body.length - 1;
    const lastLine = body[lastIndex];
    if (/^\s/.test(rawLine) && lastLine !== undefined && lastLine.startsWith("- ")) {
      body[lastIndex] = `${lastLine} ${stripInlineMarkdown(trimmed)}`;
      continue;
    }
    if (trimmed.startsWith("### ")) {
      body.push(trimmed);
      continue;
    }
    body.push(
      trimmed.startsWith("- ")
        ? `- ${stripInlineMarkdown(trimmed.slice(2))}`
        : stripInlineMarkdown(trimmed),
    );
  }
  return body;
}

interface RawSection {
  parsed: ParsedVersion;
  version: string;
  date: string;
  rawBody: string[];
}

/** Every parseable `## [x.y.z]` section of the changelog, in file order. */
function parseSections(changelogText: string): RawSection[] {
  const lines = changelogText.split(/\r?\n/);
  const sections: RawSection[] = [];
  let current: RawSection | null = null;

  for (const line of lines) {
    const header = SECTION_HEADER_PATTERN.exec(line);
    if (header) {
      const versionToken = header[1] ?? "";
      const parsed = parseVersion(versionToken);
      current =
        parsed === null
          ? null
          : { parsed, version: versionToken.trim(), date: (header[2] ?? "").trim(), rawBody: [] };
      if (current) sections.push(current);
      continue;
    }
    if (line.startsWith("## ")) {
      current = null;
      continue;
    }
    current?.rawBody.push(line);
  }
  return sections;
}

function toChangelogSection(section: RawSection): ChangelogSection {
  return {
    version: section.version,
    date: section.date,
    body: buildBodyLines(section.rawBody),
  };
}

/**
 * The changelog sections strictly newer than `afterVersion` and up to `throughVersion`
 * (inclusive), newest first (WNW-040). `[Unreleased]` and unparseable section headers are
 * skipped; a malformed changelog or unparseable version bound yields `[]` (WNW-070).
 */
export function extractSectionsBetween(
  changelogText: string,
  afterVersion: string,
  throughVersion: string,
): ChangelogSection[] {
  const after = parseVersion(afterVersion);
  const through = parseVersion(throughVersion);
  if (after === null || through === null) return [];

  return parseSections(changelogText)
    .filter(
      (section) =>
        compareVersions(section.parsed, after) > 0 && compareVersions(section.parsed, through) <= 0,
    )
    .sort((a, b) => compareVersions(b.parsed, a.parsed))
    .map(toChangelogSection);
}

/**
 * The single changelog section matching `version`, or `[]` when the version is
 * unparseable or has no section — the fresh-start content (WNW-030), degrading
 * to silent seeding per WNW-070.
 */
export function extractSectionFor(changelogText: string, version: string): ChangelogSection[] {
  const wanted = parseVersion(version);
  if (wanted === null) return [];

  return parseSections(changelogText)
    .filter((section) => compareVersions(section.parsed, wanted) === 0)
    .slice(0, 1)
    .map(toChangelogSection);
}
