// wdio.conf.ts
// Following the official tauri-apps/webdriver-example v2 pattern.
//
// Prerequisites (one-time setup — run /setup-e2e):
//   npm install --save-dev @wdio/cli @wdio/local-runner @wdio/mocha-framework \
//               @wdio/spec-reporter webdriverio @wdio/globals
//   cargo install tauri-driver
//   sudo apt-get install -y webkit2gtk-driver   # Linux: provides WebKitWebDriver
//
// Run:
//   npm run test:e2e          # local (headed window)
//   npm run test:e2e:xvfb     # Linux with virtual framebuffer (no display)
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import os from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { Options } from "@wdio/types";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

// On test failure, save a screenshot here so flaky/broken tests can be
// diagnosed without re-running locally. The directory is git-ignored.
const SCREENSHOT_DIR = resolve(__dirname, "screenshots/e2e-failures");

// Binary name from [[bin]] in src-tauri/Cargo.toml where path = "src/main.rs".
// Must use `tauri build --debug --no-bundle`, NOT plain `cargo build`:
// plain cargo build produces a binary that connects to the Vite dev server (devUrl).
// Only the Tauri CLI build embeds the frontend dist into the binary.
const BINARY_NAME = "folioneer";
const BINARY_PATH = resolve(__dirname, "src-tauri/target/debug", BINARY_NAME);

// Ephemeral DB isolation: the Rust binary reads FOLIONEER_E2E_DATA_DIR and
// uses it as the data directory instead of the default Tauri app data dir.
// Each run gets a fresh temp dir → clean database, no leftover E2E rows.
const E2E_DATA_DIR = resolve(os.tmpdir(), `${BINARY_NAME}_e2e_data_${Date.now()}`);

// tauri-driver uses two ports that must stay in sync:
//   TAURI_DRIVER_PORT  — WebdriverIO connects to tauri-driver on this port (config.port below)
//   TAURI_NATIVE_PORT  — tauri-driver uses this to talk to WebKitWebDriver (Linux) or the native driver
// Default: 4444 / 4445. If another project already occupies 4444 on this machine,
// change both constants (e.g. 4446 / 4447) — no other edits needed.
const TAURI_DRIVER_PORT = 4444;
const TAURI_NATIVE_PORT = 4445;

let tauriDriver: ChildProcess;
let cleanShutdown = false;

export const config: Options.Testrunner = {
  // tauri-driver runs on port 4444 by default.
  host: "127.0.0.1",
  port: TAURI_DRIVER_PORT,
  logLevel: "warn",

  framework: "mocha",
  specs: ["./e2e/**/*.test.ts"],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      // Prevent WebdriverIO v9 from injecting webSocketUrl:true (BiDi) —
      // WebKitWebDriver on Linux does not support BiDi and rejects the session.
      "wdio:enforceWebDriverClassic": true,
      // @ts-expect-error tauri-specific capability not in @wdio/types
      "tauri:options": { application: BINARY_PATH },
    },
  ],
  reporters: ["spec"],
  mochaOpts: { timeout: 60000 },

  // The webview's localStorage lives in the default WebKit profile and survives
  // across E2E runs — FOLIONEER_E2E_DATA_DIR redirects only the SQLite data
  // dir. If the app version bumped since the last run, the stored What's-new
  // last-seen version would open the WNW dialog over the UI and intercept every
  // click — and so would a missing key, since the fresh-start path now shows the
  // current version's section (WNW-030). Seeding a sentinel above any real
  // version leaves no changelog section in the (stored, current] interval, so
  // the launch silently re-seeds the current version (WNW-070) and shows nothing.
  before: async () => {
    // @ts-expect-error browser is injected by @wdio/globals into the runner scope
    await browser.execute(() => localStorage.setItem("whats_new_last_seen_version", "999.999.999"));
    // Driver-native reload — never navigate from inside execute(): the page can
    // start unloading before the driver captures the script result.
    // @ts-expect-error browser is injected by @wdio/globals into the runner scope
    await browser.refresh();
    // Wait for the shell to come back after the reload before any spec runs.
    // This hook is the sole gate in front of the whole suite, so the budget is
    // wider than the per-spec first-boot 15s.
    // @ts-expect-error browser is injected by @wdio/globals into the runner scope
    await browser.$("#nav-accounts").waitForExist({
      timeout: 25000,
      timeoutMsg:
        "what's-new pre-suite hook: #nav-accounts did not reappear after seeding the whats_new_last_seen_version sentinel and reloading",
    });
  },

  // Capture a screenshot on every failed test for post-mortem diagnosis.
  // File: screenshots/e2e-failures/{suite}-{test}-{timestamp}.png
  afterTest: async (test, _context, result) => {
    if (result.passed) return;
    try {
      if (!existsSync(SCREENSHOT_DIR)) mkdirSync(SCREENSHOT_DIR, { recursive: true });
      const sanitize = (s: string) => s.replace(/[^a-zA-Z0-9-_]+/g, "_").slice(0, 80);
      const suite = sanitize(test.parent ?? "unknown-suite");
      const title = sanitize(test.title);
      const ts = new Date().toISOString().replace(/[:.]/g, "-");
      // @ts-expect-error browser is injected by @wdio/globals into the runner scope
      await browser.saveScreenshot(resolve(SCREENSHOT_DIR, `${suite}-${title}-${ts}.png`));
    } catch (err) {
      console.error("[afterTest] screenshot capture failed:", err);
    }
  },

  // Build the binary once before any session starts.
  // --no-bundle: skip installer packaging, just produce the binary.
  // --debug: debug profile (faster compile, includes debug symbols).
  onPrepare: () => {
    const result = spawnSync("npx", ["tauri", "build", "--debug", "--no-bundle"], {
      cwd: resolve(__dirname),
      stdio: "inherit",
      env: { ...process.env, SQLX_OFFLINE: "true" },
    });
    if (result.status !== 0) {
      throw new Error(`tauri build failed with exit code ${result.status}`);
    }
  },

  // Start tauri-driver just before the WebDriver session is created.
  // beforeSession (not onPrepare) is correct: tauri-driver is a per-session
  // intermediary and must be alive when the worker creates the session.
  beforeSession: () => {
    process.env.RUST_LOG = "warn";
    // Force English locale so that all translated aria-labels and text content
    // match the selectors in the test files regardless of the system locale.
    process.env.LANG = "en_US.UTF-8";
    process.env.LANGUAGE = "en";
    process.env.LC_ALL = "en_US.UTF-8";
    // Ephemeral DB: wipe any leftover from a previous interrupted run, then
    // create a fresh dir and expose it to the binary via env var.
    if (existsSync(E2E_DATA_DIR)) rmSync(E2E_DATA_DIR, { recursive: true, force: true });
    try {
      mkdirSync(E2E_DATA_DIR, { recursive: true });
    } catch (err) {
      throw new Error(`Failed to create E2E data dir ${E2E_DATA_DIR}: ${err}`);
    }
    process.env.FOLIONEER_E2E_DATA_DIR = E2E_DATA_DIR;
    tauriDriver = spawn(
      resolve(os.homedir(), ".cargo", "bin", "tauri-driver"),
      ["--port", String(TAURI_DRIVER_PORT), "--native-port", String(TAURI_NATIVE_PORT)],
      { stdio: [null, process.stdout, process.stderr] },
    );
    tauriDriver.on("error", (error) => {
      console.error("tauri-driver error:", error);
      process.exit(1);
    });
    tauriDriver.on("exit", (code) => {
      if (!cleanShutdown) {
        console.error("tauri-driver exited unexpectedly with code:", code);
        process.exit(1);
      }
    });
  },

  // Kill tauri-driver cleanly after the session ends.
  afterSession: () => {
    cleanShutdown = true;
    if (existsSync(E2E_DATA_DIR)) rmSync(E2E_DATA_DIR, { recursive: true, force: true });
    tauriDriver?.kill();
  },
};

// Ensure tauri-driver is killed on unexpected process exit (Ctrl+C, SIGTERM, etc.)
// Only SIGINT/SIGTERM/SIGHUP are registered — not "exit", which fires after these
// handlers already call process.exit() and would invoke cleanup a second time.
function onShutdown(fn: () => void) {
  const cleanup = () => {
    try {
      fn();
    } finally {
      process.exit();
    }
  };
  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);
  process.on("SIGHUP", cleanup);
}

onShutdown(() => {
  cleanShutdown = true;
  if (existsSync(E2E_DATA_DIR)) rmSync(E2E_DATA_DIR, { recursive: true, force: true });
  tauriDriver?.kill();
});
