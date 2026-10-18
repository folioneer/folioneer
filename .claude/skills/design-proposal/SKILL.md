---
name: design-proposal
description: Produces the design proposal required before anything the user sees changes — rendered mocks of the target state in light and dark under screenshots/design/, plus a five-line note — for an issue or a chat request. Invoked as `/design-proposal <id>` (an issue number, or a short slug for a chat request). The human validates; nothing visible is built before that.
argument-hint: "[123 | slug]"
---

# Skill — `design-proposal`

The proposal is what the human validates instead of the pull request (`docs/workflow.md`
§ 4). It shows the target state with the real components and tokens, so what they
approve is what ships.

## Step 1 — Read the task

Load the issue (`gh issue view <id> --comments`) or take the chat request. From it and
its Done when, list the screens and states that change: one state per distinct thing the
human must see (the row with the new column, the dialog with the moved buttons, the
empty case if it changes).

## Step 2 — Build the target state as a preview

Follow `/visual-proof` Steps 0 (config) and 3 (write `preview.html` and
`src/__preview__/main.tsx`), with one difference: the preview renders the **proposed**
component tree, not the current one. Copy the current component into the preview where
the change is layout-only, or compose the target from existing `ui/` components with
sample data for a new surface. Sample data is invented, never copied from the installed
app. Never modify the real component for a proposal; the preview is disposable.

## Step 3 — Capture into the design folder

`/visual-proof` Step 4 with the output directory changed: start Vite in the background,
wait until the preview answers, capture, stop Vite.

```bash
VP_PORT={port} VP_HOST={host} VP_NAME={id} VP_STATES={state1,state2} VP_OUT_DIR=screenshots/design node scripts/visual-proof-capture.mjs
```

Produces `screenshots/design/{id}-{light|dark}-{state}.png`. Delete `preview.html` and
`src/__preview__/` afterwards.

## Step 4 — Write the note

`screenshots/design/{id}.md`, five lines at most: what moves, what is added, what is
removed, what stays the same, one open point if any.

## Step 5 — Ask, or record

- **Chat:** show the images and the note, and ask. On a yes, set the issue's Design to
  `validated` (`gh issue edit`; a chat request has no issue: say so in the PR body) and
  continue the task. On a no, write what the human wants on the issue and stop.
- **Headless:** commit the images and the note on a `design/{id}` branch, push it, and
  post the note on the issue with each image linked from that branch, with
  `needs-owner`. The human validates in the thread.

## Rules

1. Real components and tokens only; no drawing tools, no hand-made images.
2. Both themes for every state.
3. The proposal never touches production code.
4. The proposal never reaches `main`: the issue holds it; the visual proofs are the record.
