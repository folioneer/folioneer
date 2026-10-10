---
name: run-queue
description: Runs the owner's whole Next queue in a chat session — one task per queued entry so the owner sees it advance, the opening block of questions and mock-ups once, then `/next-todo` for each ready entry in turn, the next one prepared behind the one in review. Use when the owner says go to the queue. It decides nothing new: it chains what `docs/workflow.md` § 3 and `/next-todo` already say, and never starts a release.
---

# Skill — `run-queue`

A batch in chat (`docs/workflow.md` § 3, "A batch in chat"). The rules are there and in
`/next-todo`; this file is the order they are run in.

## Step 0 — Preconditions

`git status --short` is empty and the branch is `main`, fresh (`git pull --ff-only`).
Otherwise stop and say why.

## Step 1 — The queue as tasks

`python3 scripts/whats-next.py`; read its `queued` list — each reference with its title,
its `state` and what it `waits_on`. `TaskCreate`, in the queue's order:

- one task "Opening block";
- one task per queued entry, `N. REF — title`; an entry that is not ready says what it
  waits on in its subject;
- one task "Hand-over".

An empty queue: say so, name the hand-over steps (Step 4) and stop.

## Step 2 — The opening block

As `docs/workflow.md` § 3 says: read every queued entry and the code it touches, challenge
each `DEBT-NNN` against the code, draw the mock-ups (`/design-proposal`) of the entries
that change what the user sees, then ask — one question at a time, each with its context
— and write every answer on its entry. One docs pull request, merged on green. What is
assumed without asking is said to the owner and written in that pull request's body.

## Step 3 — One entry at a time

For each task, in order:

- An entry still not ready after the opening block: leave its task pending, say what it
  waits on, go to the next.
- Otherwise mark the task `in_progress` and invoke `/next-todo <REF>` with the `Skill`
  tool — invoked, not followed from memory: at the first entry, and again after any pull
  request of the batch that changed the skill. The steps are those of the skill as loaded.
- While that entry's pull request is in CI, prepare the next one behind it
  (`docs/workflow.md` § 11): its branch is cut from the branch in review, and it is pushed
  only once the one below has merged. `just watch-pr <number>` takes the number, since
  the checked-out branch has moved on.
- Mark the task `completed` when the entry's pull request has merged. An entry that stops
  by `/next-todo`'s own rules (an open question, the same gate red three times, three
  hours) keeps its task `in_progress` with the reason in its subject; say where it
  stopped and go on to the next entry unless it depends on the stopped one.

## Step 4 — Hand-over

When every task is completed or stopped: say which entries merged and which stopped, then
run the hand-over of `docs/workflow.md` § 3 — `/dep-audit`, `spec-checker` on every spec
the batch touched (`bash scripts/batch-specs.sh`), what they find filed as debt — and the
closing block. Say that `main` is ready only once the runs of its head commit have ended
green, naming them.

## Rules

1. Never start a release, never add to or reorder Next.
2. The owner is asked in the opening block and nowhere else; a question that appears
   later is written on its entry (`docs/workflow.md` § 3).
3. One task per entry: the task list is how the owner watches the queue.
