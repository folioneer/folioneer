# Contracts — format and rules

A contract (`docs/contracts/{domain}-contract.md`) is the wire record of one bounded
context's commands: what the frontend calls, with which arguments, what comes back and how
it fails, so both sides build the same shape. The agent writes or amends it from its spec
and the generated bindings, and `contract-reviewer` checks it before anything is built from
it (owner, 2026-10-18). Two sources say what it holds: `src/bindings.ts` (`just
generate-types` after a Rust change) for the names, the arguments and the types, which
`scripts/contract-check.py` holds mechanically on every pull request; the spec and the
command's own code, down to the service it calls, for which error codes it returns and
under which rule.

## Rules

- **One contract per bounded context** — the `context/{domain}/` aggregate root that owns
  the commands, never a page or a UI concern. Commands from `use_cases/` belong to the
  contract of the aggregate they mainly mutate or read; a module with no aggregate of its
  own — a use case, or `agent_connection/` — keeps a contract of its own
  (`scheduled-fetch`, `update`, `agent-connection`). A gateway calling
  several contracts is expected; one command in two contracts is not.
- **Frontend-callable commands only**, each backed by a spec rule: an action triggered from
  the UI. Internal logic (background jobs, startup tasks, schedulers) is no command; a spec
  with no frontend-callable backend rule has no contract. A rule unclear on who triggers it
  is asked, never classified internal silently. `log_frontend` is exempt
  (`NO_CONTRACT` in `scripts/contract-check.py`).
- **Errors are exhaustive and named** — every failure a spec rule describes is a variant
  (`NotFound`, `InvalidStatus`); a catch-all (`DatabaseError`, `Unknown`) only beside them.
  The Errors column lists wire-visible codes, with the spec rule when it helps
  (`ProcessAlreadyRunning (ORD-113)`); Rust type names (`*Error` enums, composites) stay out
  (`docs/error-model.md` says how they are built), but for a member type that stands for
  all its codes (§ Row format).
- **Shared types** — business meaning only: `snake_case` fields, `PascalCase` names, no
  storage types or derives; an `Option<>` only where `None` has a business meaning, said
  in the field's comment. **Events** are those the spec's state transitions
  name; `Last updated by` names the spec or specs the changes came from, with the rule IDs when
  it helps.
- **An amendment** appends new commands and updates changed rows; a command is never
  removed silently. Each change adds a Changelog line (the section added where missing).
- **The check** — `python3 scripts/contract-check.py` after each change. A new gap is fixed;
  a line of `contract-gaps.json` the change closed is dropped with `--shrink`. A code the
  code returns and no rule asks for is written with `_(no rule)_` and reported in the pull request body.

## Row format — what `scripts/contract-check.py` reads

| Command        | Args                                 | Return          | Errors                                                                  |
| -------------- | ------------------------------------ | --------------- | ----------------------------------------------------------------------- |
| `rename_thing` | `thing_id: String, new_name: String` | `Option<Thing>` | `NotFound` (THG-010), `NameTaken { holder }` (THG-011), `DatabaseError` |
| `count_things` | —                                    | `u32`           | _(infallible — reads what is in memory)_                                |

- **Args**: `name: Type` pairs in the command's order, or the name of its single input
  struct; `—` for none. **Return**: the Rust form of the generated type — `Vec<T>`,
  `Option<T>`, `()`.
- **Errors**: each code in backticks, outside any parentheses, followed, when it helps, by
  its rule in parentheses. Anything in parentheses or in italics is a note: a code named there is not a
  promise. A member type of the command's error type may stand for all its codes
  (`` `AccountError` codes ``) only where the command passes every one of them through.

## Format

````markdown
# Contract — {Domain}

> Domain: {domain}
> Last updated by: {spec-name}

## Commands

| Command           | Args                        | Return       | Errors                       |
| ----------------- | --------------------------- | ------------ | ---------------------------- |
| `snake_case_name` | `ArgStruct { field: Type }` | `ReturnType` | `ErrorVariant`, `OtherError` |

## Shared Types

```rust
// Business-level struct — no derives, no storage detail; Option only with a meaning
struct EntityName {
    field_name: FieldType,
}
```

## Events

| Event        | Payload       |
| ------------ | ------------- |
| `event_name` | `PayloadType` |

## Changelog

- {YYYY-MM-DD} — Added by `{spec-name}`: {command list}
````
