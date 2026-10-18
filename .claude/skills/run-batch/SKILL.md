---
name: run-batch
description: Runs a whole batch in chat — every Queued item of the Project through /run-entry, one after the other, then the batch's end (a release or a batch tag), then /flow-audit. Chains the existing skills and decides nothing new. Use when the owner says to run the batch.
---

# Skill — `run-batch`

A batch from its Queued items to its audit (`docs/workflow.md` § 3 and § 9). Every step
below is an existing skill or rule; this file only puts them in order, so the steps are
loaded and not remembered.

## Step 0 — Preconditions

Chat only. `git status --short` is empty and `main` is fresh (`git pull --ff-only`);
otherwise stop and report.

## Step 1 — Read the batch and show it

`gh project item-list 1 --owner folioneer --format json --limit 500`: the items with
Status Queued and the latest Batch date, in row order, and those already Done.
`TaskCreate` one task per remaining item in order, then "Closing brief and batch end"
and "Flow audit". Mark each task `in_progress` when it starts, `completed` once merged;
`/run-entry` creates none under it.

## Step 2 — Ask first

For every remaining item, before any of them starts: the decision scan of
`docs/workflow.md` § 3, and `/design-proposal` for each that changes what the user sees
and is not `validated`. Put the questions and the mocks to the owner one at a time;
write each answer on its issue. The batch then runs without waiting on the owner.

## Step 3 — Run the items

For each remaining item that is ready, in order: `/run-entry #123`. While a pull request
waits for its checks, the next task may be prepared on a branch not pushed yet
(`docs/workflow.md` § 11).

Stop, and say where, when `/run-entry` stops by its own rules (an open question, the
same gate red three times, the three-hour budget). An item that is not ready is said so
and skipped.

## Step 4 — Say what the batch achieved, then end it

When every item of the batch has been run, skipped or stopped on, the closing brief of the batch, before
anything else: what shipped (each issue with its pull request), what a user notices (the
`feat` and `fix` titles, or "nothing"), what was filed during the run, and what was left
— skipped, stopped or not ready — with why.

Then end the batch in the order `docs/workflow.md` § 3 "Ending a batch" gives — the
release sweep (§ Release sweep), then, with a `feat` or `fix`, `just release-check` and
`/dep-audit` on the final `main` and the hand-over to the owner naming the runs; without,
`just batch-end` on a fresh `main`.

## Step 5 — Publish and audit

Once tagged, or released and published (`docs/workflow.md` § 3): `/flow-audit`, once. The
owner publishes the draft: when it is not published yet, stop and say that `/flow-audit`
runs once it is.

## Rules

1. It chains; it decides nothing. A choice a step does not settle goes to the owner.
2. Never add to or reorder the batch, never run `just release`, never skip an item
   silently.
3. One batch per invocation: it ends with the audit.
