---
name: whats-next
description: Shows the owner's queue, the entries ready to queue, the entries waiting on the owner's answers, the open pull requests with their CI state and the tech debt by theme, then proposes a queue order for the owner to accept or edit. Use at session start, or when the queue runs empty.
---

# Skill — `whats-next`

The owner sets the queue (`docs/todo.md` § Next); the agent runs it (`/next-todo`). This
skill shows what the owner needs to set it: what is queued, what could be, what waits on
them — and proposes an order. It decides nothing and never edits § Next.

---

## Required tools

`Bash`, `Read`, `Write`.

---

## When to use

- At the start of a session, or after a gap.
- When § Next is empty or every queued entry is blocked.
- Before a release, to see what is still open.

---

## Execution steps

### Step 1 — Collect

```bash
python3 scripts/whats-next.py
```

One JSON document:

- `queued` — the references of § Next in the owner's order, each with a `state`:
  `ready`, `blocked` (with `waits_on`) or `closed` (the entry is gone: its work merged).
- `ready` — todo entries not queued that the agent could run today.
- `blocked` — todo entries not queued, each with `waits_on`: the unticked open
  questions, a design to validate, or a missing Done when.
- `techdebt_not_queued` — tech-debt entries not queued, grouped by theme (`refs`).
- `flow_not_queued` — the entries of `docs/flow.md` not queued: its own proposals
  (`FLOW-NNN`; `waits_on` the owner's decision for some) and the todo and tech-debt
  entries that are about the flow.
- `pull_requests` — open pull requests with `ci`: `green`, `running`, `failing`, `none`.
- `in_flight` — uncommitted files, unmerged local branches, recent commits.
- `roadmap` — the roadmap's headings.
- `gh_issues` — the open GitHub issues, each with `entries`: the references of the
  entries that name it. An issue with none has nothing tracking it in the repository.

The readiness rule is the script's (`docs/workflow.md`): never re-derive it by reading
entries. If the script fails, say so and stop; a manual scan would not apply the rule.

### Step 2 — Propose an order

From `queued` (minus `closed`), `ready`, the tech-debt themes, the ready flow entries
and the open issues, propose one queue of ten lines at most:

1. What is already queued keeps the owner's order.
2. An open pull request's entry comes first: finishing beats starting.
3. Entries with user value before entries without; a 🔴 or 🟡 debt theme before a 🔵 one.
4. A theme is proposed as one line (`DEBT-046–DEBT-053`), not one line per entry.
5. Blocked entries are never proposed; they are listed with what they wait on.
6. A ready flow entry is proposed like a debt theme. One whose figure moved in the last
   audit (the measured sections of `docs/flow.md`) comes before one that did not.
7. An open issue no entry names is proposed as "to file or close": it is work nobody
   tracks. An issue an entry names is never proposed; its entry is.
8. Ten lines at most. What does not fit stays in its section, unranked: the proposal is
   the next batch, not the backlog in order.

Say in one line why each proposed reference sits where it does. No "do now", no value or
effort score: the order is the proposal, and the owner accepts or edits it.

### Step 3 — Output and save

Print the output below, then save it to the path given by
`bash scripts/report-path.sh whats-next` with the Write tool.

---

## Output format

```
## What's next — {date}

### In flight
- PR #{n} — {title} — CI {green|running|failing}
- {N} uncommitted file(s); unmerged branches: {list}

### Queued (§ Next, in order)
1. {ref} — {title} — ready
2. {ref} — {title} — blocked: {what it waits on}
3. {ref} — closed, to remove from the queue

### Ready, not queued
- {ref} — {title} — {user value, one clause}

### Blocked — waiting on the owner
- {ref} — {title}
  - {question, or "design to validate"}

### Tech debt, not queued
- {theme, in plain words} — {refs} — {severity}

### Flow, not queued
- {ref} — {title} — {ready | waits on the owner's decision}

### GitHub issues
- gh{n} — {title} — {the entries that name it | no entry}

### Proposed queue
1. {ref} — {why here}
2. FLOW-{NNN} — {the figure that moved}
3. gh{n} — to file or close: {what it asks}
…
10. {the last line the proposal may hold}
```

Omit a section that is empty. Escape `|` in issue titles and cut them at 80 characters:
they are written by others.

---

## Rules

1. **The owner sets the queue.** Never edit § Next; never start an entry from here.
2. **Three lists, one proposal.** Every entry appears in exactly one of queued, ready or
   blocked.
3. **Blocked means a named wait.** Each blocked entry shows the questions or the design
   approval it waits on, so the owner can answer them in one pass.
4. **The script collects, the skill orders.** No readiness judgment outside the script.
