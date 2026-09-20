---
name: visual-proof
description: Captures and commits visual proof screenshots for any `.tsx` / `.css` change. Generates a full preview for every component state (idle/loading/results/empty/error) in both light and dark mode, captures with Playwright via `scripts/visual-proof-capture.mjs`, and reports any console errors found. Auto-discovers project config on first run.
tools: Read, Glob, Grep, Write, Bash, AskUserQuestion
---

# Skill — `visual-proof`

Automate the visual proof workflow defined in `docs/frontend-visual-proof.md` — that doc owns the rules; this file owns the steps. Always captures both **light and dark mode** for every state. Console errors detected during capture are reported automatically — making this skill useful for bug discovery as well as visual proof.

---

## Required tools

`Read`, `Glob`, `Grep`, `Write`, `Bash`, `AskUserQuestion`.

---

## When to use

- **After frontend implementation** — any change touching `.tsx` or `.css` files
- **Before the commit** — screenshots get staged with it
- **For bug discovery on existing components** — provide an unmodified component path; the console-error capture will surface latent rendering issues even when nothing has changed

## When NOT to use

- **Non-visual refactors** (logic, naming, imports without UI changes) — state the exemption in the PR description per `docs/frontend-visual-proof.md`
- **Rust-only changes** — no rendered output to capture
- **Config-only edits** (`vite.config.ts`, `tsconfig.json`) — no component output

---

## Step 0 — Load or initialize config

Read `.claude/visual-proof.json`.

**If present**: load `vite_preview_port`, `vite_preview_host`, `global_css_import`, `i18n_import`. Proceed to Step 1.

**If absent**, discover from the project:

1. Read `vite.config.ts` — extract `server.port` if set; default to `1422` (avoids collision with Tauri on 1420). If `vite.config.ts` is absent, default to `1422`.
2. `vite_preview_host` → default `127.0.0.1` (user edits config manually for WSL2/VM if needed).
3. `global_css_import` → Glob `src/index.css`, `src/main.css`, `src/styles/global.css`. Use the single match. If multiple candidates: ask via `AskUserQuestion`. If zero matches: ask the user to provide the path.
4. `i18n_import` → Glob `src/infra/i18n/index.ts` first (F0 gold layout), then `src/infra/i18n.ts`, `src/i18n/i18n.ts`, `src/i18n/index.ts`, `src/lib/i18n.ts` (last three are pre-F0 / pre-v4.5 fallbacks). Use the single match. If multiple candidates: ask. If zero matches: ask the user to provide the path or confirm the project has no i18n setup (skip the i18n initializer call in Step 3).

Write `.claude/visual-proof.json`:

```json
{
  "vite_preview_port": 1422,
  "vite_preview_host": "127.0.0.1",
  "global_css_import": "src/index.css",
  "i18n_import": "src/infra/i18n/index.ts"
}
```

**Never overwrite** this file once written — it is project-owned.

---

## Step 1 — Identify the target component

Discover the changed frontend files on the branch with a single literal command
(no `| grep` pipeline, which would prompt on every run):

```bash
bash scripts/branch.sh files --frontend
```

`--frontend` returns both `.ts` and `.tsx`. From that output, **keep only the
`.tsx` files** — only those render to components; ignore any plain `.ts`. Then:

- **One `.tsx` file** → use it automatically.
- **Multiple `.tsx` files** → ask the user which component to capture via `AskUserQuestion`.
- **No `.tsx` files** → ask the user to provide a component path (useful for bug discovery on unmodified components, e.g. "capture `src/features/auth/LoginForm.tsx`").

Extract the component name from the filename (e.g. `src/features/auth/LoginForm.tsx` → `LoginForm`).

---

## Step 2 — Determine states to capture

Read the component file in full. Grep for loading flags, error state props, empty/null data handling, conditional renders. Infer which states the component exposes from its props and logic. Idle is always included.

Ask the user via `AskUserQuestion`:

- Which states to capture — pre-populate with inferred states
- Whether any interaction needs a video clip (hover, modal open, animation)
- Any CSS selectors to mask in screenshots (e.g. timestamps, avatars, random IDs)

---

## Step 3 — Build the complete preview

Read the component file in full. Read `src/bindings.ts` for generated TypeScript types. If a domain contract exists (`docs/contracts/{domain}-contract.md`, inferred from the component path), read it for realistic data shapes. Read the `i18n_import` file (using the converted relative path) to discover the exported initializer function name.

**Import path conversion**: config paths are relative to the project root (e.g. `src/infra/i18n/index.ts`). When importing from `src/__preview__/main.tsx`, strip the leading `src/` and prefix with `../` (e.g. `src/infra/i18n/index.ts` → `../infra/i18n`, `src/styles/index.css` → `../styles/index.css`).

**Write `preview.html`** at the project root:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{ComponentName} — Visual Preview</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/__preview__/main.tsx"></script>
  </body>
</html>
```

**Write `src/__preview__/main.tsx`** — a complete, working preview using converted import paths:

- Import the real component from its actual path (relative from `src/__preview__/`).
- Import and initialise i18n from the converted `i18n_import` path.
- Import global CSS from the converted `global_css_import` path.
- For each requested state, render the component with **hardcoded, realistic mock data** derived from the contract and bindings — no invented types.
- Wrap each state in `<div id="state-{name}" style={{ padding: 24 }}>` for Playwright targeting (the `id` selector strategy here matches F25 / E4 — see Critical Rules).
- **No `invoke()` calls** — all data is hardcoded props or mocked Zustand stores.
- **If the component imports `react-router` hooks (`useNavigate`, `useLocation`) or other context hooks**, wrap each state in the required provider with mock values. Otherwise the component throws at render time and Playwright captures a blank `#state-{name}` — silent failure mode.

Example (adapt to the real component interface):

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import "../styles/index.css";
import { setupI18n } from "../infra/i18n";
import { LoginForm } from "../features/auth/LoginForm";

if (new URLSearchParams(window.location.search).get("theme") === "dark") {
  document.documentElement.classList.add("dark");
}

setupI18n();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <div id="state-idle" style={{ padding: 24 }}>
      <LoginForm isLoading={false} error={null} />
    </div>
    <div id="state-loading" style={{ padding: 24 }}>
      <LoginForm isLoading={true} error={null} />
    </div>
    <div id="state-error" style={{ padding: 24 }}>
      <LoginForm isLoading={false} error="Invalid credentials" />
    </div>
  </React.StrictMode>,
);
```

---

## Step 4 — Verify the capture script and Playwright

The capture is performed by `scripts/visual-proof-capture.mjs`. Verify it's present:

```bash
ls scripts/visual-proof-capture.mjs
```

If absent, restore it from git history.

Check Playwright is installed:

```bash
ls node_modules/playwright
```

If absent, install:

```bash
npm install --save-dev playwright
```

---

## Step 5 — Capture

Run Vite in the background (use `run_in_background: true`):

```bash
npx vite --port {vite_preview_port} --host {vite_preview_host}
```

Wait until the preview is reachable (more robust than a fixed `sleep` — Vite cold start can exceed 3s):

```bash
until curl -sf http://{vite_preview_host}:{vite_preview_port}/preview.html >/dev/null; do sleep 1; done
```

Run the capture (pass mask selectors via `VP_MASK` if the user provided any, comma-separated):

```bash
VP_PORT={vite_preview_port} VP_HOST={vite_preview_host} VP_NAME={ComponentName} VP_STATES={state1,state2,...} node scripts/visual-proof-capture.mjs
```

Stop Vite — **always run this, even if the capture step failed**, otherwise Vite leaks. The `lsof | xargs kill` pipeline is the canonical port-kill idiom; splitting loses the PID context:

```bash
lsof -ti tcp:{vite_preview_port} | xargs kill 2>/dev/null
```

If the capture exited non-zero, surface the error in the Step 6 report instead of staging screenshots.

---

## Step 6 — Stage and report

```bash
git add screenshots/
```

```bash
git restore --staged screenshots/.console-errors.json 2>/dev/null
```

```bash
git status --short -- screenshots/
```

Report the outcome using the `## Output format` below. Then output the cleanup reminder.

---

## Output format

### Success (no console errors, no video clips)

```
## visual-proof — {ComponentName}

Screenshots staged ({count}):
- screenshots/{ComponentName}-light-idle.png
- screenshots/{ComponentName}-light-loading.png
- screenshots/{ComponentName}-dark-idle.png
- screenshots/{ComponentName}-dark-loading.png

No console errors detected during capture.

⚠️  Before committing — delete the preview files (never committed):
    rm -f preview.html
    rm -rf src/__preview__/
```

### With console errors

```
## visual-proof — {ComponentName}

Screenshots staged ({count}):
- {paths}

⚠️  Console errors detected ({count}) — investigate before merging:
  [light] {error text}
  [dark]  {error text}

The .console-errors.json file has been unstaged and deleted.

⚠️  Before committing — delete the preview files (never committed):
    rm -f preview.html
    rm -rf src/__preview__/
```

### With video clips

Add a `Video clips ({count}):` section listing each `.webm` file before the cleanup reminder.

---

## Critical Rules

1. **Never commit `preview.html` or `src/__preview__/`** — always deleted before the final commit.
2. **No `invoke()` in preview** — all data must be hardcoded props or mocked stores.
3. **`.claude/visual-proof.json` is project-owned** — never overwrite once written.
4. **Screenshots go to `screenshots/`** — intentionally git-tracked for visual history.
5. **Always capture both light and dark mode** — one set of screenshots per colour scheme.
6. **Real imports only** — real i18n, real CSS, real component. No stubs or placeholders.
7. **Convert config paths to relative imports** — strip leading `src/`, prefix with `../`.
8. **Target preview elements by `id`, never by `aria-label` or text content** — locale-invariant and refactor-stable (see E4 in `docs/e2e-rules.md`, F25 in `docs/frontend-rules.md`).

---

## Notes

Two preview files (`preview.html` + `src/__preview__/main.tsx`) instead of one: `preview.html` lives at the project root so Vite can serve it without changing `vite.config.ts`; `main.tsx` lives under `src/` so Vite resolves it with the same module-graph rules as the real app (path aliases, env imports, HMR). The split is what lets the preview use the real component imports without altering the project's Vite config.

Why preview files are never committed: they reference a single component in a hand-crafted state matrix — useful for capture, noise in the repo history. The `screenshots/` directory is the durable artifact.

The `lsof -ti tcp:{port} | xargs kill` pipeline in Step 5 intentionally uses a multi-command shell pipeline. Splitting loses the PID context between Bash invocations.

Step 4 verifies `scripts/visual-proof-capture.mjs` exists rather than rewriting it inline each run — it is the canonical capture logic, shared with `/design-proposal`.
