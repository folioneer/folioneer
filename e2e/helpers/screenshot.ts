import { existsSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";

// Real-app captures land here; the E2E workflow uploads the folder as a run
// artifact and links it from the pull request. Git-ignored, never committed —
// the committed visual proofs under screenshots/ are component previews, these
// are the running application.
const SCREENSHOT_DIR = fileURLToPath(new URL("../../screenshots/e2e", import.meta.url));

/**
 * Captures the current screen in both themes as
 * `screenshots/e2e/{name}-light.png` and `screenshots/e2e/{name}-dark.png`.
 *
 * Call once per spec file, after the wait that settles the screen under test
 * (the dialog open, the row showing its final value). The pair is the pull
 * request's evidence of what the real app shows.
 *
 * The theme is forced through the `dark` root class the theme hook itself
 * sets (`useThemeToggle.applyTheme`); the app's stored mode is untouched, so
 * only the header's theme-toggle icon may disagree with the forced colours.
 * The class is restored afterwards, even when a capture throws, so the rest
 * of the spec runs under the app's own theme.
 */
export async function captureScreen(name: string): Promise<void> {
  if (!existsSync(SCREENSHOT_DIR)) mkdirSync(SCREENSHOT_DIR, { recursive: true });

  const wasDark = await browser.execute(() => document.documentElement.classList.contains("dark"));

  try {
    await freezeForCapture(true);
    await parkPointer();
    for (const scheme of ["light", "dark"] as const) {
      await setDark(scheme === "dark");
      await waitForRepaint();
      await browser.saveScreenshot(resolve(SCREENSHOT_DIR, `${name}-${scheme}.png`));
    }
  } finally {
    await freezeForCapture(false);
    await setDark(wasDark);
  }
}

// Two things the WebView paints on its own schedule, each large enough on its
// own to cross the workflow's 0.3 % regression threshold:
//
//   Motion — the app animates colour changes over 150–200 ms, so a capture two
//   frames after the theme flips lands mid-transition and differs from run to
//   run by a few units per pixel.
//
//   Scrollbars — an overlay scrollbar fades in when a scrollable pane is
//   touched and out again a moment later, so the same screen shows a thumb in
//   one run and not in the next. On the settings page that thumb is a 3 px bar
//   over 355 rows: 0.22 % of the screen.
//
// Both are switched off for the capture, so every run paints the settled state.
async function freezeForCapture(frozen: boolean): Promise<void> {
  await browser.execute((on: boolean) => {
    const id = "e2e-capture-freeze";
    document.getElementById(id)?.remove();
    if (!on) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = [
      "*, *::before, *::after { transition: none !important; animation: none !important; }",
      "* { scrollbar-width: none !important; }",
      "*::-webkit-scrollbar { display: none !important; }",
    ].join("\n");
    document.head.appendChild(style);
  }, frozen);
}

// The pointer rests wherever the last click left it, so a control under it
// paints its hover colour in one run and not in the next (a 48 px button is
// 0.45 % of the screen, over the regression threshold). It is parked in the
// top-left corner before the capture. Pointer actions are optional for a
// WebDriver implementation; when the driver refuses them the capture goes on
// as before and the run log says so.
async function parkPointer(): Promise<void> {
  try {
    await browser
      .action("pointer", { parameters: { pointerType: "mouse" } })
      .move({ x: 0, y: 0, origin: "viewport" })
      .perform();
  } catch (error) {
    console.warn(`captureScreen: pointer not parked (${String(error)})`);
  }
}

async function setDark(dark: boolean): Promise<void> {
  await browser.execute((value: boolean) => {
    document.documentElement.classList.toggle("dark", value);
  }, dark);
}

// Two animation frames: the first is scheduled before the class flip has been
// styled, the second runs after that frame painted. Tracks the WebView's own
// paint instead of guessing a delay that headless GTK rendering may exceed.
async function waitForRepaint(): Promise<void> {
  await browser.executeAsync((done: () => void) => {
    requestAnimationFrame(() => requestAnimationFrame(() => done()));
  });
}
