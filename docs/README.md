# Documentation map

Each kind of rule has one home. Other documents link to it and never restate it; `scripts/rule-homes.py` fails when a rule ID is defined twice, or when a skill, an agent prompt, a script or a git hook has no line in `agentic-setup.md`.

| Kind          | Home                                                                                                                                              | Holds                                                                                                                                            |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Domain        | `spec/` (`TRIGRAM-NNN`; its `README.md` holds the numbering, the trigrams and the format), `ubiquitous-language.md`                               | business rules, and what the domain words mean — validated by the owner                                                                          |
| Rules         | `backend-rules.md` (B), `frontend-rules.md` (F), `e2e-rules.md` (E), `i18n-rules.md`, `test-rules.md`, `commit-rules.md`, `visual-proof-rules.md` | how code, tests, strings, commits and visual proofs are written                                                                                  |
| Rule examples | `backend-patterns.md`, `error-model.md`                                                                                                           | worked shapes for rules already stated in the rules docs                                                                                         |
| Decisions     | `adr/` (its `README.md` holds the gate and the format)                                                                                            | technical choices costly to reverse, never domain ones                                                                                           |
| Contracts     | `contracts/` (its `README.md` holds the rules and the format)                                                                                     | the wire record of each command                                                                                                                  |
| Workflow      | `workflow.md`                                                                                                                                     | how work moves from an issue to `main`; the backlog itself is on GitHub (issues and the folioneer Project), never in the repository              |
| Records       | `lessons.md`, `roadmap.md`, `../CHANGELOG.md`                                                                                                     | what was learned, planned or shipped                                                                                                             |
| Reference     | `ddd-reference.md`, `ddd-divergences.md`, `design-system.md`, `external-dependencies.md`                                                          | background an agent reads when a task needs it                                                                                                   |
| Map           | `../ARCHITECTURE.md`                                                                                                                              | where code lives                                                                                                                                 |
| Index         | `../CLAUDE.md`                                                                                                                                    | authority, forbidden actions, and pointers to all of the above                                                                                   |
| Agents        | `../.claude/agents/`, `../.claude/skills/`, `agentic-setup.md`                                                                                    | reviewer and skill prompts, which apply the rules docs and never restate one; what each skill, agent prompt, script and hook is for, in one line |

## When two documents disagree

The home wins: a spec over anything about business behaviour, a rules doc over anything about how code is written, an ADR over the code it guards. The copy is removed, not reconciled.

## Where a new statement goes

- A business behaviour → a spec rule (and the vocabulary if a new word appears).
- How to write code, tests, strings or commits → the matching `*-rules.md`, with an ID where it can be checked.
- A technical choice costly to reverse → an ADR with its guard (the gate of `adr/README.md`); code organisation and a domain decision are never an ADR.
- A deliberate deviation from a rule → next to that rule, or in `ddd-divergences.md` when it is a DDD textbook deviation.
- Anything else an agent needs every turn → `CLAUDE.md`, as one line and a pointer.
