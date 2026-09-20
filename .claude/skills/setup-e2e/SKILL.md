---
name: setup-e2e
description: One-time setup of the Tauri WebDriver E2E infrastructure. Installs npm packages, generates wdio.conf.ts from the project binary, adds test:e2e / test:e2e:ci npm scripts. Run once before the first test-writer-e2e invocation. Safe to re-run — skips steps already done.
tools: Read, Grep, Glob, Bash, Edit, Write
---

# Skill — `setup-e2e`

Invocation: `/setup-e2e`

Sets up the Tauri WebDriver E2E infrastructure for this project.
Safe to re-run — completed steps are detected and skipped where possible
(Step 2 reinstalls the full npm set if any package is missing).

After this skill completes, the `test-writer-e2e` agent can be used to write tests
without any further infrastructure work.

---

## When to use

- **Before the first E2E test** — once per project, to stand up the WebDriver infrastructure `test-writer-e2e` depends on
- **When `wdio.conf.ts` is missing or the suite won't start** — safe to re-run; every step skips work already done
- **Before the first E2E scenario** — run it once if E2E isn't set up yet

Not for writing or running tests — that's `test-writer-e2e` and `npm run test:e2e`. This skill only provisions the infrastructure.

---

## Known Pitfalls (read before debugging a broken suite)

These are real failures encountered when setting up Tauri v2 WebDriver on Linux/WSL2.
The `wdio.conf.ts` template below is designed to avoid all of them.

### 1 — Wrong port (4445 vs 4444)

**Symptom**: session creation hangs or connects to a stale tauri-driver from a previous run.
**Cause**: tauri-driver defaults to port **4444**, not 4445.
**Fix**: `host: "127.0.0.1", port: 4444` — already set in the template.

### 2 — "Failed to match capabilities" — BiDi rejection

**Symptom**: `Error: Failed to match capabilities` immediately on session start.
**Cause**: WebdriverIO v9 automatically injects `webSocketUrl: true` (WebDriver BiDi protocol)
into capabilities whenever `browserName` is present (even an empty string).
WebKitWebDriver on Linux does not support BiDi and rejects the session outright.
**Fix**: `"wdio:enforceWebDriverClassic": true` in capabilities — suppresses the BiDi injection.
Already set in the template. Do NOT add `browserName` to capabilities.

### 3 — tauri-driver in wrong hook (`onPrepare` vs `beforeSession`)

**Symptom**: session creation fails immediately; tauri-driver process is not alive when WDIO
tries to connect.
**Cause**: `onPrepare` runs in the WDIO launcher process. Workers are forked after it completes
and cannot inherit a child process spawned there. tauri-driver must be alive inside the worker.
**Fix**: spawn tauri-driver in `beforeSession` (runs inside the worker, just before session
creation) and kill it in `afterSession`. `onPrepare` is only for building the binary.
Already set in the template.

### 4 — Blank WebView / app shows `about:blank`

**Symptom**: the Tauri window opens but the WebView is completely blank.
**Cause**: building with plain `cargo build` produces a binary that tries to connect to the
Vite dev server at `devUrl` (e.g. `http://localhost:1420`). No dev server is running during
E2E, so the WebView has nothing to load.
**Fix**: build with `npx tauri build --debug --no-bundle`. The Tauri CLI build embeds the
compiled frontend dist into the binary. Plain `cargo build` must never be used for E2E.
Already set in the template's `onPrepare`.

### 5 — Wrong binary name (productName vs [[bin]])

**Symptom**: `ENOENT` — binary not found at `src-tauri/target/debug/{name}`.
**Cause**: `productName` in `tauri.conf.json` is the installer/bundle display name — it is NOT
the Rust binary name. The actual debug binary is named after the `name` field in the `[[bin]]`
section of `src-tauri/Cargo.toml` where `path = "src/main.rs"`.
**Fix**: read `[[bin]]` from `Cargo.toml`. Already handled in Step 1 of this skill.

---

## Step 1 — Detect binary name

Read `src-tauri/Cargo.toml`. Find the `[[bin]]` section where `path = "src/main.rs"`.
Use its `name` field as `{binary-name}`.

**Do not use `productName` from `tauri.conf.json`** — that is the installer bundle name,
not the debug binary name.

If no `[[bin]]` section is found, report:

```
## /setup-e2e — BLOCKED: no [[bin]] in src-tauri/Cargo.toml

Add one pointing to src/main.rs before re-running:

    [[bin]]
    name = "your-app"
    path = "src/main.rs"
```

Then stop — this is a terminal state, not a warning to continue past.

---

## Step 2 — Check npm packages

Read `package.json` `devDependencies`. Check for:

- `@wdio/cli`
- `@wdio/local-runner`
- `@wdio/mocha-framework`
- `@wdio/spec-reporter`
- `webdriverio`
- `@wdio/globals`

If any are missing, install them:

```bash
npm install --save-dev @wdio/cli @wdio/local-runner @wdio/mocha-framework \
    @wdio/spec-reporter webdriverio @wdio/globals
```

If the install fails (no network, registry error), stop and report the failure
verbatim — do not continue to Step 5, which would write a `wdio.conf.ts` that
imports packages the project does not have. Carry the install status into the
Step 8 report (`✅ installed` vs `⚠️ manual install needed`).

If all are present, report `✅ npm E2E packages already installed.` and skip.

---

## Step 3 — Check tauri-driver

```bash
ls ~/.cargo/bin/tauri-driver
```

If the command exits with an error (binary not found), report:

```
⚠️  tauri-driver not found. Install it manually (takes ~1 min):

    cargo install tauri-driver

Then re-run /setup-e2e or continue — the remaining steps do not require it.
```

Continue regardless — tauri-driver is only needed at test run time.

If found, report `✅ tauri-driver found.`

---

## Step 4 — Check WebKitWebDriver (Linux only)

```bash
which WebKitWebDriver
```

If the command exits with an error (not found on Linux):

```
⚠️  WebKitWebDriver not found. Install it (required on Linux):

    sudo apt-get install -y webkit2gtk-driver

This provides WebKitWebDriver, which tauri-driver proxies to on Linux.
Not needed on macOS (uses SafariDriver) or Windows (uses EdgeDriver).
```

Continue regardless.

---

## Step 5 — Generate wdio.conf.ts

Glob for `wdio.conf.ts` and `wdio.conf.js` at the project root.

If found, compare it against the template below. If it matches the current
template, report `✅ wdio.conf.ts already exists — skipping generation.` and
skip. If it diverges (an older or hand-broken config — the usual reason for a
re-run when "the suite won't start"), show the difference and ask the user
whether to regenerate before overwriting. Do not silently skip a stale config.

If absent, write `wdio.conf.ts` at the project root using the template below,
substituting `{binary-name}` with the value from Step 1:

```typescript
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
//   npm run test:e2e:ci       # Linux CI (xvfb virtual display)
import os from "os";
import { existsSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { spawn, spawnSync, type ChildProcess } from "child_process";
import { fileURLToPath } from "url";
import type { Options } from "@wdio/types";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

// Binary name from [[bin]] in src-tauri/Cargo.toml where path = "src/main.rs".
// Must use `tauri build --debug --no-bundle`, NOT plain `cargo build`:
// plain cargo build produces a binary that connects to the Vite dev server (devUrl).
// Only the Tauri CLI build embeds the frontend dist into the binary.
const BINARY_NAME = "{binary-name}";
const BINARY_PATH = resolve(`./src-tauri/target/debug/${BINARY_NAME}`);

// OPTIONAL — Ephemeral DB isolation:
// If the app reads a custom DB path from an env var (e.g. set in main.rs / lib.rs),
// uncomment these lines to give each E2E run a clean, isolated database.
// Replace MY_APP_E2E_DB with the actual env var name your binary reads.
// const E2E_DB_PATH = resolve(os.tmpdir(), `${BINARY_NAME}_e2e.db`);

// tauri-driver uses two ports that must stay in sync:
//   TAURI_DRIVER_PORT  — WebdriverIO connects to tauri-driver on this port (config.port below)
//   TAURI_NATIVE_PORT  — tauri-driver uses this to talk to WebKitWebDriver (Linux) or the native driver
// Default: 4444 / 4445. If another project already occupies 4444 on this machine,
// change both constants (e.g. 4446 / 4447) — no other edits needed.
const TAURI_DRIVER_PORT = 4444;
const TAURI_NATIVE_PORT = 4445;

let tauriDriver: ChildProcess;
let exit = false;

export const config: Options.Testrunner = {
  host: "127.0.0.1",
  port: TAURI_DRIVER_PORT,
  // Suppress WebDriver protocol logs (COMMAND/POST/RESULT chatter) — keep warnings and errors only.
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

  // Build the binary once before any session starts.
  // --no-bundle: skip installer packaging, just produce the binary.
  // --debug: debug profile (faster compile, includes debug symbols).
  onPrepare: () => {
    const result = spawnSync(
      "npx",
      ["tauri", "build", "--debug", "--no-bundle"],
      {
        cwd: resolve(__dirname),
        stdio: "inherit",
        shell: true,
      },
    );
    if (result.status !== 0) {
      throw new Error(`tauri build failed with exit code ${result.status}`);
    }
  },

  // Start tauri-driver just before the WebDriver session is created.
  // beforeSession (not onPrepare) is correct: tauri-driver is a per-session
  // intermediary and must be alive when the worker creates the session.
  beforeSession: () => {
    // OPTIONAL — Ephemeral DB isolation (uncomment if using E2E_DB_PATH above):
    // if (existsSync(E2E_DB_PATH)) rmSync(E2E_DB_PATH);  // clean up any leftover from a previous interrupted run
    // process.env.MY_APP_E2E_DB = E2E_DB_PATH;           // expose path to the binary via env var
    //
    // Suppress verbose Rust/frontend tracing — only show warnings and errors.
    process.env.RUST_LOG = "warn";

    tauriDriver = spawn(
      resolve(os.homedir(), ".cargo", "bin", "tauri-driver"),
      [
        "--port",
        String(TAURI_DRIVER_PORT),
        "--native-port",
        String(TAURI_NATIVE_PORT),
      ],
      { stdio: [null, process.stdout, process.stderr] },
    );
    tauriDriver.on("error", (error) => {
      console.error("tauri-driver error:", error);
      process.exit(1);
    });
    tauriDriver.on("exit", (code) => {
      if (!exit) {
        console.error("tauri-driver exited unexpectedly with code:", code);
        process.exit(1);
      }
    });
  },

  // Kill tauri-driver cleanly after the session ends.
  afterSession: () => {
    exit = true;
    tauriDriver?.kill();
    // OPTIONAL — Ephemeral DB isolation (uncomment if using E2E_DB_PATH above):
    // if (existsSync(E2E_DB_PATH)) rmSync(E2E_DB_PATH);
  },
};

// Ensure tauri-driver is killed even on unexpected process exit (Ctrl+C, SIGTERM, etc.)
function onShutdown(fn: () => void) {
  const cleanup = () => {
    try {
      fn();
    } finally {
      process.exit();
    }
  };
  process.on("exit", cleanup);
  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);
  process.on("SIGHUP", cleanup);
}

onShutdown(() => {
  exit = true;
  tauriDriver?.kill();
  // OPTIONAL — Ephemeral DB isolation (uncomment if using E2E_DB_PATH above):
  // if (existsSync(E2E_DB_PATH)) rmSync(E2E_DB_PATH);
});
```

---

## Step 6 — Add npm scripts

Read `package.json`. Check if `"test:e2e"` and `"test:e2e:ci"` are present in `scripts`.

If both are present, report `✅ npm scripts already present.` and skip.

If either is missing, add them using Edit (do NOT overwrite the file):

```json
"test:e2e": "npx wdio run wdio.conf.ts",
"test:e2e:ci": "xvfb-run --auto-servernum npm run test:e2e"
```

`test:e2e` — local development (headed window visible on screen).
`test:e2e:ci` — Linux CI runners with no display (virtual framebuffer via xvfb).

---

## Step 7 — Create e2e/ directory

```bash
mkdir -p e2e
```

Report `✅ e2e/ directory ready.`

---

## Step 8 — Report

Output a summary:

```
## /setup-e2e — complete

Binary: {binary-name}  (from src-tauri/Cargo.toml [[bin]])

{✅ npm packages installed | ⚠️ manual install needed — see Step 2}
✅ wdio.conf.ts generated (or already present)
✅ npm scripts added (or already present)
✅ e2e/ directory ready

{Any ⚠️ warnings from Steps 3–4}

Next steps:
1. Install tauri-driver if not yet done:  cargo install tauri-driver
2. On Linux, install WebKitWebDriver:     sudo apt-get install -y webkit2gtk-driver
3. Run the test writer:                   use test-writer-e2e agent with a contract
4. Run the suite:                         npm run test:e2e
```
