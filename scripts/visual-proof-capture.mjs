#!/usr/bin/env node

/**
 * Playwright capture script for the /visual-proof skill.
 *
 * Reads environment variables, opens the project's Vite preview at light+dark,
 * screenshots one element per state, records any console errors. Invoked from
 * `.claude/skills/visual-proof/SKILL.md` Step 5 and `.claude/skills/design-proposal/SKILL.md`
 * Step 3; not intended for direct CLI use.
 *
 * Env contract:
 *   VP_PORT   — Vite preview port (required)
 *   VP_HOST   — Vite preview host (required)
 *   VP_NAME   — component name for output filenames (required)
 *   VP_STATES — comma-separated state names matching `#state-{name}` ids (default: "idle")
 *   VP_MASK   — comma-separated CSS selectors to mask in screenshots (optional)
 *   VP_OUT_DIR — output directory (optional, default "screenshots"; design
 *               proposals use "screenshots/design")
 *
 * Outputs (stdout reserved for future machine-readable output; progress and
 * warnings go to stderr):
 *   {VP_OUT_DIR}/{VP_NAME}-{scheme}-{state}.png per scheme×state combination.
 *   {VP_OUT_DIR}/.console-errors.json if any console errors were observed.
 *
 * Exit codes: 0 ok, 2 usage error (missing env), 1 runtime/Playwright error.
 */

import { mkdir, writeFile } from "node:fs/promises";
import { chromium } from "playwright";

const PORT = process.env.VP_PORT;
const HOST = process.env.VP_HOST;
const NAME = process.env.VP_NAME;
const STATES = (process.env.VP_STATES || "idle").split(",");
const MASK_SELECTORS = (process.env.VP_MASK || "").split(",").filter(Boolean);
const OUT_DIR = process.env.VP_OUT_DIR || "screenshots";

if (!PORT || !HOST || !NAME) {
  console.error("error: VP_PORT, VP_HOST, and VP_NAME are required");
  process.exit(2);
}

const consoleErrors = [];
await mkdir(OUT_DIR, { recursive: true });

const browser = await chromium.launch({ args: ["--no-sandbox"] });
try {
  for (const scheme of ["light", "dark"]) {
    const context = await browser.newContext({
      colorScheme: scheme,
      viewport: { width: 1600, height: 900 },
    });
    try {
      const page = await context.newPage();

      page.on("console", (msg) => {
        if (msg.type() === "error") {
          consoleErrors.push({ scheme, text: msg.text() });
        }
      });

      const url = `http://${HOST}:${PORT}/preview.html${scheme === "dark" ? "?theme=dark" : ""}`;
      await page.goto(url, { waitUntil: "domcontentloaded" });
      await page.waitForSelector(`#state-${STATES[0]}`, { timeout: 10000 });

      const masks = MASK_SELECTORS.map((sel) => page.locator(sel));

      for (const state of STATES) {
        const el = page.locator(`#state-${state}`);
        if ((await el.count()) > 0) {
          const path = `${OUT_DIR}/${NAME}-${scheme}-${state}.png`;
          await el.screenshot({ path, mask: masks });
          console.error(`  → ${path}`);
        }
      }
    } finally {
      await context.close();
    }
  }

  if (consoleErrors.length > 0) {
    console.error("\n⚠️  Console errors detected during capture:");
    for (const err of consoleErrors) {
      console.error(`  [${err.scheme}] ${err.text}`);
    }
    await writeFile(`${OUT_DIR}/.console-errors.json`, JSON.stringify(consoleErrors, null, 2));
  }
} finally {
  await browser.close();
}
