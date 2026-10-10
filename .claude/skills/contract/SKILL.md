---
name: contract
description: Derives or updates a domain contract (docs/contracts/{domain}-contract.md) from a validated feature spec and the generated bindings, in the row format scripts/contract-check.py reads. Adds and amends rows without dropping any. Run when a command, a type or an error changes, before contract-reviewer. Not for judging a contract — that is contract-reviewer.
argument-hint: "[docs/spec/<feature>.md | <domain>]"
---

# Skill — `contract`

A contract is the wire record of a domain's commands: what the window sends, what it
gets back, and every way a command refuses. Two sources say what it holds:
`src/bindings.ts`, generated from the core, for the names, the arguments and the types;
the spec and the command's own code for which error codes it returns and under which
rule. `scripts/contract-check.py` holds the first half mechanically; this skill does
both halves.

## Required tools

`Read`, `Grep`, `Glob`, `Edit`, `Write`, `Bash`.

## Steps

1. **Load** — the spec given (`docs/spec/<feature>.md`), or every spec the domain's
   contract names in `> Last updated by:` when a domain is given. Regenerate the
   bindings first when the Rust changed: `just generate-types`.
2. **Domain** — the contract a command belongs to is the one that already holds its
   neighbours (`grep` the command's bounded context in `docs/contracts/`). A command
   lives in one contract only. A feature with no command the window calls gets no
   contract.
3. **Commands** — for each command the spec's rules reach, read its signature in
   `src/bindings.ts` and its body in Rust, down to the service it calls. Collect: the
   arguments as generated, the return type as generated, and every error code the
   command can return — each with the rule that asks for it. A code the error type
   carries and this command never returns is left out. A code the code returns and no
   rule asks for is written with `_(no rule)_` and reported in the hand-off.
4. **Types** — every struct and enum a row names, with the fields and variants of the
   generated type, in the same order; the comment beside a field says its business
   meaning and its rule, never its storage.
5. **Write** — amend the file in place. Never drop a row, a type or a changelog line
   without saying so in the hand-off. Chat: show the rows added and changed and ask
   before writing. Headless: write, and list them in the pull request body.
6. **Changelog** — one line under `## Changelog`: `- {YYYY-MM-DD} — {rule or entry}:
{what changed on the wire}`.
7. **Check** — `python3 scripts/contract-check.py`. A new gap is fixed here; a line of
   `contract-gaps.json` this work closed is dropped with `--shrink`. Then
   `contract-reviewer` on the file.

## Row format

```markdown
| Command        | Args                                 | Return          | Errors                                                                  |
| -------------- | ------------------------------------ | --------------- | ----------------------------------------------------------------------- |
| `rename_thing` | `thing_id: String, new_name: String` | `Option<Thing>` | `NotFound` (THG-010), `NameTaken { holder }` (THG-011), `DatabaseError` |
| `add_thing`    | `AddThingDTO`                        | `()`            | `DatabaseError`                                                         |
| `count_things` | —                                    | `u32`           | _(infallible — reads what is in memory)_                                |
```

- **Command**: the name the window invokes, in backticks.
- **Args**: `name: Type` pairs in the command's order, or the name of its single input
  struct (defined under `## Shared Types`, or inline as `Name { field: Type }`); `—`
  for none.
- **Return**: the Rust form of the generated type — `Vec<T>`, `Option<T>`, `()`.
- **Errors**: each code in backticks, outside any bracket, followed by its rule in
  brackets. Anything in brackets or in italics is a note: a code named there is not a
  promise. A member type of the command's error type may stand for all its codes
  (`` `AccountError` codes ``) only where the command passes every one of them through.

## Rules

1. One command, one contract.
2. Errors are exhaustive and named: a row whose only code is `DatabaseError` is right
   only for a command that validates nothing.
3. No command without a rule: a command no spec asks for is reported, not documented
   as if one did.
4. The skill writes the contract; `contract-reviewer` judges it; the check keeps it
   true afterwards.
