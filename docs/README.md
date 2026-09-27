# Documentation map

Each kind of rule has one home. Other documents link to it and never restate it; `scripts/rule-homes.py` fails when a rule ID is defined twice.

| Kind          | Home                                                                                                                                              | Holds                                                                   |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Domain        | `spec/` (`TRIGRAM-NNN`), `ubiquitous-language.md`                                                                                                 | business rules, and what the domain words mean — validated by the owner |
| Rules         | `backend-rules.md` (B), `frontend-rules.md` (F), `e2e-rules.md` (E), `i18n-rules.md`, `test-rules.md`, `commit-rules.md`, `visual-proof-rules.md` | how code, tests, strings, commits and visual proofs are written         |
| Rule examples | `backend-patterns.md`, `error-model.md`                                                                                                           | worked shapes for rules already stated in the rules docs                |
| Decisions     | `adr/`                                                                                                                                            | technical choices costly to reverse, each with its guard                |
| Contracts     | `contracts/`                                                                                                                                      | the wire record of each command                                         |
| Workflow      | `workflow.md`                                                                                                                                     | how work moves from an entry to `main`                                  |
| Records       | `todo.md`, `techdebt.md`, `lessons.md`, `roadmap.md`                                                                                              | what is owed, known or learned                                          |
| Reference     | `ddd-reference.md`, `ddd-divergences.md`, `design-system.md`, `external-dependencies.md`, `spec-index.md`                                         | background an agent reads when a task needs it                          |
| Map           | `../ARCHITECTURE.md`                                                                                                                              | where code lives                                                        |
| Index         | `../CLAUDE.md`                                                                                                                                    | authority, forbidden actions, and pointers to all of the above          |

## When two documents disagree

The home wins: a spec over anything about business behaviour, a rules doc over anything about how code is written, an ADR over the code it guards. The copy is removed, not reconciled.

## Where a new statement goes

- A business behaviour → a spec rule (and the vocabulary if a new word appears).
- How to write code, tests, strings or commits → the matching `*-rules.md`, with an ID where it can be checked.
- A technical choice costly to reverse → an ADR with its guard (the `adr-writer` gate); code organisation is never an ADR.
- A deliberate deviation from a rule → next to that rule, or in `ddd-divergences.md` when it is a DDD textbook deviation.
- Anything else an agent needs every turn → `CLAUDE.md`, as one line and a pointer.
