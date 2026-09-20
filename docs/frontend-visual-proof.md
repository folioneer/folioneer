# Frontend Visual Proof Rules

> Required by F18 in [`frontend-rules.md`](frontend-rules.md) § Tests.

Any change that touches a `.tsx`, `.css`, or visual asset file **MUST** include committed
screenshots in `screenshots/` before merging. Screenshots must always cover both **light and
dark mode**. Use `/visual-proof` to automate the full capture workflow.

---

## When visual proof is not required

State it explicitly at the top of the PR description or commit message:

> No visual impact — internal refactor / Rust-only change.

Then screenshot at least one screen that _consumes_ the modified code as a non-regression proof.

---

## What to capture

| Change type                                                          | Required artefact                                   |
| -------------------------------------------------------------------- | --------------------------------------------------- |
| New component or layout change                                       | Screenshot of every affected state, light + dark    |
| Interaction (hover, animation, modal open/close, loading transition) | Playwright video clip saved as `.webm`              |
| Shared / design-system component                                     | Screenshot of 2–3 distinct call sites, light + dark |
| Dark mode                                                            | **Always required** — capture both modes every time |

**States to cover for every component:** idle · loading · results/content · empty · error.

**Screenshot naming:** `screenshots/{ComponentName}-{light|dark}-{state}.png`

---

## Project config

On first run, `/visual-proof` discovers and writes `.claude/visual-proof.json` — owned by the
downstream project and never overridden by the kit:

```json
{
  "vite_preview_port": 1422,
  "vite_preview_host": "127.0.0.1",
  "global_css_import": "src/styles/index.css",
  "i18n_import": "src/infra/i18n/index.ts"
}
```

- **`vite_preview_port`**: must not conflict with the Tauri dev port (1420). Default: `1422`.
- **`vite_preview_host`**: default `127.0.0.1`. Set to `0.0.0.0` for WSL2 / VM access.
- **`global_css_import`**: path from project root to the global CSS entry point.
- **`i18n_import`**: path from project root to the i18n initializer.

---

## Process

### 1 — Create a preview entry

Create two temporary files (delete them before the final commit):

**`preview.html`** at the project root:

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

**`src/__preview__/main.tsx`** — renders the component in every relevant state with hardcoded
mock data. No Tauri `invoke()` calls; the gateway pattern keeps components decoupled from the IPC
layer.

Import the real i18n initializer and global CSS using paths relative to `src/__preview__/` (strip
the leading `src/`, prefix with `../`):

```tsx
import "../styles/index.css"; // {global_css_import} with src/ → ../
import { setupI18n } from "../infra/i18n"; // {i18n_import} with src/ → ../
import { MyComponent } from "../features/domain/MyComponent";

setupI18n();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <div id="state-idle" style={{ padding: 24 }}>
      <MyComponent
        isLoading={false}
        error={null}
        items={[{ id: 1, name: "Example" }]}
      />
    </div>
    <div id="state-loading" style={{ padding: 24 }}>
      <MyComponent isLoading={true} error={null} items={[]} />
    </div>
    <div id="state-error" style={{ padding: 24 }}>
      <MyComponent isLoading={false} error="Something went wrong" items={[]} />
    </div>
  </React.StrictMode>,
);
```

Each state is wrapped in `<div id="state-{name}">` so Playwright can target it for a per-element
screenshot.

#### Modal components — panel-only pattern

Modal containers typically render a 50%-opacity black scrim across the viewport (e.g.
`bg-m3-scrim/50 backdrop-blur-[2px]`) to dim the app shell behind the dialog. In a standalone
preview there is no app shell — the scrim covers an empty wrapper, and in dark mode
50% black over a dark surface produces a near-black image that misrepresents the component.

**Render the modal panel directly, without the scrim wrapper.** Copy the panel's chrome
(rounded corners, surface tier, shadow elevation, header / scrollable content / footer) and
skip the modal container. Visual proof verifies the component, not the shell — and because
Playwright captures the `#state-{name}` element, dropping the full-viewport scrim makes the
screenshot panel-sized rather than viewport-sized, which is easier to scan in PR review.

```tsx
function PreviewModalPanel({ title, children, footer }) {
  return (
    <div
      role="dialog"
      aria-modal="true"
      className="relative bg-{surface-container-lowest} rounded-[28px] shadow-elevation-4 w-full max-w-2xl overflow-hidden flex flex-col"
    >
      {/* same header / content / footer chrome as the real modal */}
    </div>
  );
}
```

Wrap it in a `min-h-screen bg-{surface-container}` outer container so the dark-mode tone
matches the production card surface, not page-level near-black.

### 2 — Capture with Playwright (light + dark)

Start the Vite dev server on the configured port:

```bash
npx vite --port {vite_preview_port} --host {vite_preview_host}
```

Run Playwright against the preview page. Iterate over both colour schemes and capture each state
element individually:

```js
for (const scheme of ["light", "dark"]) {
  const context = await browser.newContext({
    colorScheme: scheme,
    viewport: { width: 1600, height: 900 },
  });
  const page = await context.newPage();

  await page.goto(
    `http://{vite_preview_host}:{vite_preview_port}/preview.html`,
    {
      waitUntil: "domcontentloaded",
    },
  );
  await page.waitForSelector("#state-idle", { timeout: 10000 });

  for (const state of ["idle", "loading", "error"]) {
    const el = page.locator(`#state-${state}`);
    await el.screenshot({
      path: `screenshots/{ComponentName}-${scheme}-${state}.png`,
    });
  }
}
```

Use `domcontentloaded` + `waitForSelector` instead of `networkidle` — more reliable for
frontend hydration, and `networkidle` is officially discouraged in Playwright.

For interaction clips, use video recording:

```js
const context = await browser.newContext({
  colorScheme: "light",
  recordVideo: { dir: "screenshots/" },
});
```

### 3 — Console error monitoring (bug discovery)

Attach a console listener before navigation to catch any runtime errors during render:

```js
page.on("console", (msg) => {
  if (msg.type() === "error") consoleErrors.push({ scheme, text: msg.text() });
});
```

If errors are found, write them to `screenshots/.console-errors.json` and report them. This makes
`/visual-proof` useful as a **bug discovery tool** even on unmodified components — run it on any
component you suspect has render errors, missing translations, or broken data shapes.

### 4 — Commit the artefacts

```bash
git add screenshots/
```

`screenshots/` is intentionally tracked in git — each commit is a point-in-time record. Browse
visual history with:

```bash
git log --oneline -- "screenshots/{ComponentName}-*.png"
git show <sha>:screenshots/{ComponentName}-light-idle.png > /tmp/old.png
```

### 5 — Clean up the preview files

Delete `preview.html` and `src/__preview__/` before the final commit on the branch. These are
build scaffolding — never committed.

---

## Preview fidelity

Because the preview page imports the same source files as the real app, design parity is high:

| Design element              | In preview?                                           |
| --------------------------- | ----------------------------------------------------- |
| Design-system color tokens  | ✅ same CSS import                                    |
| Custom fonts (npm packages) | ✅ resolved by Vite                                   |
| Tailwind utilities          | ✅ same Vite plugin                                   |
| Component code              | ✅ direct import                                      |
| i18n translations           | ✅ same initializer import                            |
| Dark mode                   | ✅ Playwright `colorScheme` context                   |
| Modal backdrop / app shell  | ⚠️ absent — use panel-only pattern (Process §1)       |
| Platform WebView rendering  | ⚠️ preview uses Chromium — minor subpixel differences |

The two caveats are cosmetic. If a change specifically touches modal chrome or backdrop blur, note
it in the commit message.

---

## Never do

- Merge a frontend change without committed screenshots (light + dark)
- Call `invoke()` directly in preview components — use hardcoded props or mocked stores
- Leave `preview.html` or `src/__preview__/` committed on the branch
- Hardcode the Vite port or import paths — read them from `.claude/visual-proof.json`
