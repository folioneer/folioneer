# 2026-09-27 — DEBT-043 — The reviewer agents run without a turn cap and load the whole CLAUDE.md

- Found by: the main agent, comparing the agent files with the Claude Code sub-agent documentation
- Where: `.claude/agents/reviewer-*.md` (frontmatter), `.github/workflows/review.yml`
- Severity: 🔵
- Observation: The sub-agent format now offers `maxTurns` (stop a runaway review), `effort` (per agent) and `omitClaudeMd` (skip the project `CLAUDE.md` when the prompt is self-contained). None is set: every CI review loads `CLAUDE.md` seven times per pull request, and nothing bounds a review that loops.
- User value: None directly — cheaper, bounded reviews on every pull request.
- Done when: one reviewer runs with `omitClaudeMd: true` and a `maxTurns` cap on a sample diff and reports the same findings as without; if it does, the settings extend to every reviewer, with the review time and cost before and after recorded.
