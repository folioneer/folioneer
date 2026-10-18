---
name: plan-batch
description: Shows where the Project stands (Ready, Inbox, needs-owner, open pull requests), refines the candidates against their code and proposes the next batch for the owner to approve in chat; on the yes, sets the batch on the Project. Chat only. Use when the batch is empty or short, after a batch ends, or when returning to the project.
---

# Skill — `plan-batch`

The owner sets the batch (`docs/workflow.md` § 2). This skill prepares that decision:
it shows where every item stands and proposes an order. It never decides, and it never
commits: the backlog is on GitHub.

## Step 1 — Collect

```bash
gh project item-list 1 --owner folioneer --format json --limit 500
gh issue list --state open --limit 500 --json number,title,labels,body
gh pr list --state open --json number,title,headRefName,author,statusCheckRollup
```

From them: the items by Status, the `needs-owner` issues, open issues not on the Project,
open Dependabot pull requests, open pull requests with their checks, and what the last
batches merged (the last two `batch-audit` issues). A call that fails is reported as
unknown, never as none.

## Step 2 — Check what is not Ready

- **Inbox**: give each its label, its form's fields and its size (`docs/workflow.md` § 2);
  one that changes what a user sees gets its mock (`/design-proposal`); Ready as § 2
  defines it.
- **Task**: obsolete — the path is gone or the observation no longer holds (grep the
  code, read the git log): propose closing it with the reason. Owner's call (a visible
  change, a product call): its question goes to **Needs you**. One theme is one line.
- **Flow**: workable once it carries a Decision; otherwise its question goes to the owner.

## Step 3 — Draft and refine

A feature or bug without a User value or a Done when cannot be queued: draft both from
its text (the User value in the user's words, the Done when as clauses a test can
prove), or say what is missing when the text is too thin. Then, for every item about to
be proposed: read the code it names, check its size, run the decision scan
(`docs/workflow.md` § 3). Its questions join **Needs you**. Name every programme and
propose its first slice as a sub-issue.

## Step 4 — Propose

1. **Needs you** — the decisions and drafts of Steps 2 and 3, one line each.
2. **In flight** — open pull requests and their checks. The sum of the proposal's `size:`
   labels beside the sums of the last batches, the proposal stopping at the latter; a
   batch with no feature or bug ends without a release (§ 3), say so.
3. **Proposed batch** — in the order of `docs/workflow.md` § 2: each line `#123`, a
   concise summary, at most a clause on why it sits there; where two ordering rules
   disagree, one line says so. Nothing waiting on the owner is in it.
4. **Left out** — one line per theme, with why, and each open issue or Dependabot pull
   request not proposed.

No value scores, no hour estimates, no "do now": the reason on each line is the argument.

## Step 5 — Set what the owner approves

Ask for the batch first: accept, or edit. Then each decision alone, with its context,
what each option changes and a recommendation; an answer that changes the batch shows it
again. On the yes, on GitHub only:

- each answer and accepted draft on its issue (`gh issue edit`, `gh issue comment`),
  `needs-owner` removed once nothing is open, its `size:` label set;
- each item of the batch: Batch = the date, Status = Queued, in the approved row order
  (`gh project item-edit`; the order through `gh api graphql`,
  `updateProjectV2ItemPosition`); what the batch before left unshipped goes back to Ready;
- a Dependabot pull request added as itself (`gh project item-add`);
- the obsolete issues closed with their reason, once the owner agrees.

## Rules

1. Chat only. A headless run never sets a batch.
2. Nothing is written before the owner's yes, and nothing beyond what the yes covers.
3. One proposal per invocation; `/run-batch` runs what was queued.
