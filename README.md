# Folioneer

[![coverage](https://img.shields.io/codecov/c/github/folioneer/folioneer?label=coverage)](https://codecov.io/gh/folioneer/folioneer)
[![frontend](https://img.shields.io/codecov/c/github/folioneer/folioneer?flag=frontend&label=frontend)](https://codecov.io/gh/folioneer/folioneer?flags=frontend)
[![backend](https://img.shields.io/codecov/c/github/folioneer/folioneer?flag=backend&label=backend)](https://codecov.io/gh/folioneer/folioneer?flags=backend)

Personal portfolio manager for desktop — track your financial assets across multiple accounts, record purchases, monitor positions, and follow your cost basis over time. All data is stored locally on your device.

Built with Tauri 2, React 19, and Rust.

## Quick Start

### Setup (Linux)

The steps below bootstrap a fresh machine end-to-end. Run them in order.

**1. System libraries (Tauri Linux build deps — requires sudo)**

```bash
sudo apt update && sudo apt install -y pkgconf libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev libssl-dev libayatana-appindicator3-dev libxdo-dev
```

(On Ubuntu 22.04 and earlier, replace `pkgconf` with `pkg-config`.)

**2. Rust toolchain (user-local)**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
. "$HOME/.cargo/env"
rustup toolchain install   # in the repository: installs the version rust-toolchain.toml names
```

**3. `just` task runner + `sqlx-cli`**

```bash
cargo install just
cargo install sqlx-cli --no-default-features --features sqlite
```

**4. Node.js via nvm (user-local)**

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
. "$HOME/.nvm/nvm.sh"
nvm install --lts
nvm use --lts
```

**5. Clone and install project deps**

```bash
git clone https://github.com/folioneer/folioneer.git
cd folioneer
npm install
```

**6. Verify**

```bash
just check-full
```

This runs lint + format + typecheck + tests for both backend and frontend. If it goes green, the environment is ready.

(Version requirements: `package.json` and `src-tauri/Cargo.toml`. macOS/Windows: install Rust via https://rustup.rs and Node via https://nodejs.org; system-library step is Linux-only.)

### Development

```bash
./scripts/start-app.sh
```

App opens automatically with hot reload.

### E2E setup (optional — run once)

E2E tests drive the real Tauri app via WebDriver and need two extra binaries that aren't part of the normal Tauri toolchain:

**1. WebKit WebDriver + Xvfb (system, requires sudo)**

```bash
sudo apt install -y webkit2gtk-driver xvfb
```

`webkit2gtk-driver` provides the `WebKitWebDriver` binary that `tauri-driver` proxies to. `xvfb` is only required for headless runs (`npm run test:e2e:ci`); skip it if you only need `npm run test:e2e` against your real display.

**2. tauri-driver (user-local, must match the project's Tauri version)**

```bash
cargo install tauri-driver --version 2.0.5 --locked
```

The version must match what `.github/workflows/e2e.yml` installs. Run a fresh `cargo install ... --locked` whenever the workflow's pinned version changes.

**3. Run the suite**

```bash
npm run test:e2e      # uses your current $DISPLAY
npm run test:e2e:ci   # uses xvfb-run in its own D-Bus session, as CI does
```

Specs live in `e2e/` and follow `docs/e2e-rules.md`. Each spec seeds its own state via IPC (`e2e/helpers/seed.ts`) and tears down via the wdio harness — no shared global fixtures.

### Build

```bash
npm run tauri -- build
```

Output: `src-tauri/target/release/bundle/`

## Code Quality

```bash
python3 scripts/check.py   # Full check (tests, lint, build, types)
npm run test               # Frontend tests only
cd src-tauri && cargo test # Backend tests only
```

## Command line

The installed program records an opening balance, a purchase or a sale without opening a window. Close the window first: a command that records refuses while it is open; the lists answer anyway.

```
folioneer holding open --account "PEA" --asset CW8 --quantity 10 --total-cost 4950 [--date 2026-09-28] [--json]
folioneer holding buy  --account "PEA" --asset CW8 --quantity 2 --price 495.10 [--fees 1.99] [--rate 1] [--date …] [--note "…"] [--json]
folioneer holding buy  --account "PEA" --asset CW8 --quantity 2 --total 992.19 [--fees 1.99] …
folioneer holding sell …same options as buy…
folioneer account list        # the accounts: what --account takes
folioneer asset list          # the assets: what --asset takes (--archived for the archived ones)
folioneer asset add --name "ASML Holding" --reference ASML --class Stocks --currency EUR
folioneer --help              # the commands, with examples
folioneer holding buy --help  # one command's options and defaults
```

`--account` is the account's name; `--asset` is the asset's name or reference; case does not matter. Amounts are decimals with a dot. A command prints one `Recorded:` or `Refused:` line — or one JSON object with `--json` — and exits with 0 when it recorded, 1 when it was refused, 2 when the command itself is wrong — it then prints the mistake, the closest command or option when one was mistyped, and the help to run.

- **Linux** — the program itself: `folioneer holding …` once the `.deb` is installed, or the AppImage's path with the same arguments (`~/Applications/Folioneer.AppImage holding …`).
- **Windows (PowerShell)** — the console program installed beside the main one:
  `& "$env:LOCALAPPDATA\Folioneer\folioneer-cli.exe" holding buy --account "PEA" --asset CW8 --quantity 2 --price 495.10`
  then `$LASTEXITCODE` holds the exit code.
- **WSL** — the same Windows program by its path, working on the Windows portfolio:
  `/mnt/c/Users/<you>/AppData/Local/Folioneer/folioneer-cli.exe holding …`
  (the Linux program run inside WSL keeps a separate portfolio of its own, and refuses while none exists).

## Documentation

- [Architecture](ARCHITECTURE.md) — system design, bounded contexts, data flow
- [Frontend Rules](docs/frontend-rules.md) — React/TS conventions
- [Backend Rules](docs/backend-rules.md) — Rust/DDD conventions

## License

Copyright (c) 2026 Philippe Eggel.

Folioneer is free software under the **GNU Affero General Public License, version 3 or (at your option) any later version** — see [LICENSE](LICENSE).

Why the AGPL: a tool people trust with their finances should stay open to inspection, and whoever builds on it — including by running it as a service — should publish what they change. The licence applies to everyone but does not bind the copyright holder, who may also offer the code under other terms; contributions are accepted under a [contributor agreement](CLA.md) so that this stays possible. Everything the application ships is under licences compatible with it (MIT, Apache-2.0, ISC, BSD, Zlib, Unicode, Unlicense, 0BSD, OFL-1.1 for the fonts, and five transitive MPL-2.0 crates); `just licence-check` refuses a dependency that would change that.
