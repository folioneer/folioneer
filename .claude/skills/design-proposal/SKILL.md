---
name: design-proposal
description: Produces the design proposal a todo entry needs before anything the user sees changes — rendered mocks of the target state in light and dark under screenshots/design/NNN-*.png plus a five-line note — and flips the entry's Design line to proposed. Invoked as `/design-proposal NNN`. The human validates by editing the entry.
tools: Read, Glob, Grep, Write, Edit, Bash
---

# Skill — `design-proposal`

Invocation: `/design-proposal NNN` (a `#NNN` todo reference).

The proposal is what the human validates instead of the pull request. It shows the
target state with the real components and tokens, so what they approve is what ships.

## Step 1 — Read the entry

Load `## #NNN` from `docs/todo.md`. From its text and Done when, list the screens and
states that change. One state per distinct thing the human must see (the row with the
new column, the dialog with the moved buttons, the empty case if it changes).

## Step 2 — Build the target state as a preview

Follow `/visual-proof` Steps 0 (config), 3 (write `preview.html` and
`src/__preview__/main.tsx`) and 4 (capture script and Playwright present) with one
difference: the preview renders the **proposed** component tree, not the current one. Copy the current component into the
preview where the change is layout-only, or compose the target from existing `ui/`
components with sample data when it is a new surface. Never modify the real component
for a proposal; the preview is disposable.

## Step 3 — Capture into the design folder

`/visual-proof` Step 5, with the output directory changed: start the Vite server in
the background, wait until the preview answers, capture, stop the server.

```bash
npx vite --port {port} --host {host} > /dev/null 2>&1 &
until curl -sf http://{host}:{port}/preview.html > /dev/null; do sleep 1; done
VP_PORT={port} VP_HOST={host} VP_NAME={NNN} VP_STATES={state1,state2} VP_OUT_DIR=screenshots/design node scripts/visual-proof-capture.mjs
lsof -ti tcp:{port} | xargs kill
```

Produces `screenshots/design/NNN-{light|dark}-{state}.png`. Delete `preview.html` and
`src/__preview__/` afterwards, as `/visual-proof` does.

## Step 4 — Write the note

`screenshots/design/NNN.md`, five lines at most: what moves, what is added, what is
removed, what stays the same, one open point if any.

## Step 5 — Flip the entry

In `docs/todo.md`, the entry's line becomes
`**Design:** proposed (screenshots/design/NNN-*.png)`. Commit the images, the note and
the todo edit as one `docs:` commit, open the PR, merge on green. Report the image
paths in one line. The human validates by editing the line to `validated`, or adds an
open question.

## Rules

1. Real components and tokens only; no drawing tools, no hand-made images.
2. Both themes for every state.
3. The proposal never touches production code.
4. Delete the proposal images in the closure commit of the work that ships them.
