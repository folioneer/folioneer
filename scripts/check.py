#!/usr/bin/env python3
import argparse
import os
import re
import subprocess
import sys
import threading
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# Semantic ANSI colors. Use these by *meaning* (INFO, SUCCESS, …), not by hue —
# keeps call sites readable and makes the palette swappable. Respects NO_COLOR=1.
if os.environ.get("NO_COLOR"):
    INFO = SUCCESS = FAILURE = WARNING = RESET = ""
else:
    INFO = "\033[0;34m"  # blue   — neutral/informational
    SUCCESS = "\033[0;32m"  # green  — pass
    FAILURE = "\033[0;31m"  # red    — fail
    WARNING = "\033[0;33m"  # orange — soft fail (errors counted, stale, etc.)
    RESET = "\033[0m"

# Metric status constants. Keep these explicit so call sites can't typo a
# state and silently miscategorise the result in the report.
STATUS_PASS = "Pass"
STATUS_SKIPPED = "SKIPPED"
STATUS_PENDING = "Pending"  # never-ran sentinel — rendered as Fail
STATUS_STALE = "Stale"
STATUS_UNCOMMITTED = "Uncommitted"
# Variable-detail values use a recognised prefix/suffix in `_format_status`:
#   suffix " errors" / " warnings" — e.g. "3 errors"  (TSC)
#   prefix "missing: "             — e.g. "missing: package.json absent"
#                                    (strict-mode marker absent — see
#                                    `_maybe_skip_for_stack`)

# Backend root directory. Default to the kit's `src-tauri/` convention;
# downstream forks with a different layout (e.g. `app/`, `tauri/`,
# multi-crate workspace at `crates/api/`) override this single constant
# to point check.py at their Rust root.
BACKEND_DIR = "src-tauri"

# Skip reasons for partial-stack projects. Use these everywhere a check
# is gated on a stack marker — single source of truth for the message
# that appears in inline output and the stack summary.
SKIP_FRONTEND_ABSENT = "package.json absent"
SKIP_BACKEND_ABSENT = f"{BACKEND_DIR}/Cargo.toml absent"
SKIP_SQLX_ABSENT = f"{BACKEND_DIR}/.sqlx/ absent"

# Markdown drift gate (gh#68). Biome doesn't cover .md, so this is
# the only formatting check for markdown. `just format`'s fixer runs
# prettier with the SAME args plus `--write` (gh#82), so checker and
# fixer stay in lockstep by construction — no reliance on a project's
# `format:docs` glob. Gated on package.json because prettier ships via
# the JS devDep stack; pure-backend projects skip rather than carry an
# npm dep just for md linting.
# `**/*.md` must reach prettier as a literal string (prettier
# handles globs internally via fast-glob); do NOT switch to
# shell=True or pre-expand the glob.
_PRETTIER_DOCS_CMD = [
    "npx",
    "prettier",
    "--check",
    "**/*.md",
    "--ignore-path",
    ".gitignore",
]

# Below this much allocatable RAM (GiB), a full run serialises the FE and BE
# groups and caps cargo's job count so it doesn't drive a low-memory desktop
# into swap (gh#81). Above it — and on CI — behaviour is unchanged.
_MEMORY_PRESSURE_GB = 8.0


def _available_ram_gb() -> float | None:
    """Best-effort allocatable-RAM probe, in GiB. Returns None when it can't be
    read (unknown OS, unreadable source) so callers fall back to today's
    uncapped behaviour — this must never raise. No third-party dependency.

    Reads host memory, not any cgroup limit, so a memory-capped CI container
    sees the host's (large) figure and does not throttle — the desired
    CI-unaffected behaviour (gh#81)."""
    # Linux: MemAvailable is the kernel's own estimate of what can be allocated
    # without swapping — the right signal for "how many build jobs fit?".
    try:
        with open("/proc/meminfo", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) / (1024 * 1024)  # kB → GiB
    except (OSError, ValueError, IndexError):
        pass
    # POSIX fallback: free physical pages × page size. On many platforms
    # (notably macOS) SC_AVPHYS_PAGES is absent from sysconf_names and this
    # raises → None → no throttle, which is the safe fallback. Counts only
    # free pages (ignores reclaimable cache), so it throttles a touch more
    # eagerly than MemAvailable would — acceptable for a safety cap.
    try:
        gb = (os.sysconf("SC_AVPHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")) / (1024**3)
        # sysconf may return -1 for an indeterminate limit without raising;
        # a non-positive result is meaningless, so treat it as unreadable.
        return gb if gb > 0 else None
    except (ValueError, OSError, AttributeError):
        return None


# Strip ANSI escape sequences when measuring visible cell width for the
# report table. Needed because `f"{s:<30}"` pads by string length, which
# would over-pad when the string carries color codes and under-pad under
# NO_COLOR=1.
_ANSI_RE = re.compile(r"\033\[[0-9;]*m")

# Emojis used in the quality report — Python `len()` counts each as 1, but
# every modern terminal renders them as 2 columns. Without compensation, every
# row carrying one of these is off by 1 column from the separator.
# Explicit codepoints (not the literal glyphs) so the VS-16 trap below is
# visible to the next maintainer. Add new report emojis here.
_WIDE_CHARS = frozenset(
    {
        "✅",  # ✅
        "⏩",  # ⏩
        "❌",  # ❌
        # ⚠ — Narrow per UAX#11; VS-16 below promotes to emoji-2col.
        # Kept as \u escape so ruff/format can't auto-fix the bare codepoint
        # into the VS-16 pair ⚠️ and silently re-introduce the over-pad.
        "\u26a0",
        "\U0001f680",  # 🚀
        "✨",  # ✨
    }
)
# Variation Selector-16 (U+FE0F) follows characters like ⚠ to force emoji
# presentation. Terminals render the pair as 2 columns; Python `len()` counts
# the VS as a separate char. Strip it before counting so width math stays
# correct without per-glyph special cases.
_VS16 = "️"


def _pad_visible(s: str, width: int) -> str:
    """Pad `s` to `width` visible columns, ignoring ANSI escape codes and
    compensating for wide characters that render as 2 terminal columns."""
    plain = _ANSI_RE.sub("", s).replace(_VS16, "")
    visible_cols = len(plain) + sum(1 for c in plain if c in _WIDE_CHARS)
    return s + " " * max(0, width - visible_cols)


class QualityChecker:
    def __init__(
        self,
        fast_mode: bool = False,
        verbose: bool = False,
        sequential: bool = False,
        frontend_only: bool = False,
        backend_only: bool = False,
        format_only: bool = False,
        skip_tests: bool = False,
        strict_mode: bool = False,
    ):
        self.repo_root = Path(__file__).parent.parent
        self.fast_mode = fast_mode
        self.verbose = verbose
        # Strict mode promotes stack-marker skips to failures when the marker
        # is "expected" (always-expected by default; sqlx is conditional on
        # src-tauri/migrations/ existing). release.py passes --strict so a
        # missing marker — e.g. accidentally deleted .sqlx/ — blocks the
        # release instead of silently skipping the check.
        self.strict_mode = strict_mode
        # Full mode runs frontend + backend groups concurrently by default.
        # --sequential forces the old serial order (useful for clean output
        # when debugging a single step's failure). Single-group modes
        # (--frontend / --backend / --format / --fast) imply sequential —
        # there's nothing to parallelise against.
        base_sequential = sequential or fast_mode or frontend_only or backend_only
        # gh#81 — on a low-memory machine a full run (Vite build + cargo
        # compiles, cargo spawning one rustc/linker per core) can exhaust RAM
        # and swap the whole desktop. Auto-throttle two ways, each with an env
        # override; both no-op on CI / high-RAM boxes and never change results,
        # only resource use. RAM unreadable → no throttle (today's behaviour).
        self._ram_gb = _available_ram_gb()
        self._throttle_notes: list[str] = []

        seq_override = os.environ.get("KIT_CHECK_SEQUENTIAL")
        if seq_override in ("0", "1"):
            mem_sequential = seq_override == "1"
            if mem_sequential and not base_sequential:
                self._throttle_notes.append(
                    "groups serialised (KIT_CHECK_SEQUENTIAL=1)"
                )
        elif self._ram_gb is not None and self._ram_gb <= _MEMORY_PRESSURE_GB:
            mem_sequential = True
            self._throttle_notes.append(
                f"groups serialised ({self._ram_gb:.1f} GiB RAM "
                f"≤ {_MEMORY_PRESSURE_GB:.0f} GiB)"
            )
        else:
            mem_sequential = False
        self.sequential = base_sequential or mem_sequential

        self.cargo_jobs_env = self._compute_cargo_jobs_env()
        capped = self.cargo_jobs_env.get("CARGO_BUILD_JOBS")
        if capped:
            self._throttle_notes.append(f"cargo jobs capped at {capped}")

        self.frontend_only = frontend_only
        self.backend_only = backend_only
        self.format_only = format_only
        # --skip-tests skips ONLY test execution (vitest, cargo test). Build,
        # lint, biome, tsc, sqlx, clippy, fmt still run. Use case: CI that
        # computes coverage separately (e.g. `npm run test:coverage` or
        # `cargo llvm-cov`) — avoids running tests twice. Contrast with
        # --fast which also skips build.
        self.skip_tests = skip_tests
        # Two distinct locks: `_lock` guards mutations to shared state
        # (metrics / failures / skip list); `_print_lock` serialises stdout
        # writes so parallel-mode progress lines don't interleave mid-write.
        # Keeping them separate avoids head-of-line blocking when a worker
        # writes output while another worker mutates state.
        self._lock = threading.Lock()
        self._print_lock = threading.Lock()
        self.metrics = {
            "react_tests": STATUS_SKIPPED,
            "rust_lib": STATUS_SKIPPED,
            "rust_beh": STATUS_SKIPPED,
            "build": STATUS_SKIPPED,
            "sqlx": STATUS_PENDING,
            "lint": STATUS_PENDING,
            "biome": STATUS_PENDING,
            "prettier_docs": STATUS_PENDING,
            "clippy": STATUS_PENDING,
            "rust_fmt": STATUS_PENDING,
            "tsc": STATUS_PENDING,
        }
        self.suite_failed = False
        self.failures: dict[str, str] = {}

        # Stack markers — presence-of-file gates each check.
        # Partial-stack projects (e.g. no-DB Tauri, FE-only, kit-only bootstrap)
        # skip the gated checks instead of failing.
        self.package_json = self.repo_root / "package.json"
        self.cargo_toml = self.repo_root / BACKEND_DIR / "Cargo.toml"
        self.sqlx_dir = self.repo_root / BACKEND_DIR / ".sqlx"
        # Used as the `expected_when` signal for sqlx: the .sqlx/ cache is only
        # required when the project actually has migrations. No migrations →
        # no-DB project → sqlx skip is legitimate even under --strict.
        # Cargo.toml-parsing for the `sqlx` dep would also work but is heavier
        # (TOML parser or fragile regex); migrations-dir presence matches
        # operator intuition: "I have a DB" ⇔ "I have migrations".
        self.migrations_dir = self.repo_root / BACKEND_DIR / "migrations"
        self._skipped_for_stack: list[tuple[str, str]] = []  # (reason, check_name)

    # --- Thread-safe state mutators -----------------------------------------
    # Every shared-state write goes through one of these. The lock stays
    # invisible at call sites, and the helpers document the only ways the
    # checker is allowed to mutate its results.

    def _set_metric(self, key: str, value: str) -> None:
        with self._lock:
            self.metrics[key] = value

    def _record_failure(self, step: str, output: str | None = None) -> None:
        with self._lock:
            self.suite_failed = True
            if output:
                self.failures[step] = output

    def _record_stack_skip(self, metric_key: str, check_name: str, reason: str) -> None:
        with self._lock:
            self.metrics[metric_key] = STATUS_SKIPPED
            self._skipped_for_stack.append((reason, check_name))

    def _safe_print(self, *args, file=None, **kwargs) -> None:
        """Lock-serialised `print`. Use for any user-visible output that may
        race with a parallel worker — keeps each `print` atomic so a step's
        progress line doesn't get spliced into another step's output."""
        with self._print_lock:
            print(*args, file=file or sys.stdout, **kwargs)

    def _vprint(self, *args, **kwargs):
        if self.verbose:
            self._safe_print(*args, **kwargs)

    def _maybe_skip_for_stack(
        self,
        metric_key: str,
        check_name: str,
        marker: Path,
        reason: str,
        *,
        expected_when: Path | None = None,
    ) -> bool:
        """Return True if the check should be skipped or fail-replaced (marker
        absent). In default mode, records a skip. In strict mode, promotes the
        skip to a failure when the marker is expected (always expected unless
        `expected_when` is given and absent — used by sqlx, which only requires
        .sqlx/ when src-tauri/migrations/ exists)."""
        if marker.exists():
            return False

        is_expected = expected_when is None or expected_when.exists()
        if self.strict_mode and is_expected:
            self._safe_print(
                f"  {check_name}... {FAILURE}❌ missing: {reason}{RESET}",
                flush=True,
            )
            self._set_metric(metric_key, f"missing: {reason}")
            self._record_failure(check_name, f"{reason} (required under --strict mode)")
            return True

        self._safe_print(
            f"  {check_name}... {INFO}⏩ skipped ({reason}){RESET}", flush=True
        )
        self._record_stack_skip(metric_key, check_name, reason)
        return True

    def _frontend_npm_check_step(
        self, metric_key: str, name: str, cmd: list[str]
    ) -> None:
        """Frontend npm/npx single-step pattern: skip if package.json
        absent, else run `cmd` and mark `metric_key` PASS on success.
        Encapsulates the gate→run→record triple shared by Oxlint, Biome,
        and Prettier Docs. Backend (cargo) steps need cwd/env overrides
        and have their own shape — do not retrofit them here."""
        if not self._maybe_skip_for_stack(
            metric_key, name, self.package_json, SKIP_FRONTEND_ABSENT
        ):
            if self.run_step(name, cmd):
                self._set_metric(metric_key, STATUS_PASS)

    def print_header(self, title: str):
        self._vprint(f"\n{INFO}🚀 {title}{RESET}")
        self._vprint(
            f"{INFO}═══════════════════════════════════════════════════════════{RESET}"
        )

    def run_step(
        self,
        name: str,
        cmd: list[str],
        cwd: Path | None = None,
        env_update: dict | None = None,
    ) -> bool:
        self._safe_print(f"  {name}...", flush=True)
        self._vprint(f"\n{INFO}▶ Running {name}...{RESET}")

        current_env = os.environ.copy()
        if env_update:
            current_env.update(env_update)

        # 10-minute cap on any single step. Cargo builds rarely take more
        # than 5 minutes on a warm cache; vitest the same. A genuine hang
        # (broken workspace, missing binary in a weird state) needs to
        # surface as a clear failure, not consume the CI budget silently.
        try:
            if self.verbose:
                result = subprocess.run(
                    cmd,
                    cwd=cwd or self.repo_root,
                    env=current_env,
                    timeout=600,
                )
                output = ""
            else:
                result = subprocess.run(
                    cmd,
                    cwd=cwd or self.repo_root,
                    env=current_env,
                    capture_output=True,
                    text=True,
                    timeout=600,
                )
                output = (result.stdout + result.stderr).strip()

            success = result.returncode == 0
            if success:
                self._vprint(f"{SUCCESS}✓ {name}: Passed{RESET}")
            else:
                self._vprint(
                    f"{FAILURE}✗ {name}: Failed (Exit {result.returncode}){RESET}"
                )
                self._record_failure(name, output)
            return success
        except FileNotFoundError:
            # Specific path for the most common cause of failure: tool
            # missing on PATH. Generic "[Errno 2] No such file or
            # directory: 'npm'" is unhelpful — point the user at the fix.
            missing = cmd[0] if cmd else "<unknown>"
            hint = {
                "npm": "install Node (https://nodejs.org) and re-run",
                "npx": "install Node (https://nodejs.org) and re-run",
                "cargo": "install Rust (https://rustup.rs) and re-run",
            }.get(missing, "install the tool or remove this step")
            msg = f"{missing} not found on PATH — {hint}"
            self._vprint(f"{FAILURE}✗ {name}: {msg}{RESET}")
            self._record_failure(name, msg)
            return False
        except (OSError, subprocess.SubprocessError) as e:
            # OSError catches PermissionError and similar. SubprocessError
            # covers TimeoutExpired (10-minute cap above) and
            # CalledProcessError. Bare `Exception` would swallow real
            # programming errors (TypeError on bad call shape, etc.) —
            # keep those surfaced as crashes so they get fixed.
            self._vprint(f"{FAILURE}✗ {name}: Exception: {e}{RESET}")
            self._record_failure(name, str(e))
            return False

    def check_sqlx(self) -> bool:
        if self._maybe_skip_for_stack(
            "sqlx",
            "SQLx Integrity",
            self.sqlx_dir,
            SKIP_SQLX_ABSENT,
            expected_when=self.migrations_dir,
        ):
            return True

        self._vprint(f"\n{INFO}▶ Checking SQLx Integrity...{RESET}")
        # check=False so a broken git invocation surfaces as a clean check
        # failure instead of propagating CalledProcessError out of the
        # executor and crashing the suite mid-report.
        result = subprocess.run(
            ["git", "diff", "--name-only", str(self.sqlx_dir)],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            self._set_metric("sqlx", STATUS_STALE)
            self._record_failure("SQLx", f"git diff failed: {result.stderr.strip()}")
            return False
        status = result.stdout
        if status.strip():
            self._vprint(
                f"{FAILURE}✗ SQLx: Unstaged changes in .sqlx/. Run 'just prepare-sqlx' and stage the result.{RESET}"
            )
            self._set_metric("sqlx", STATUS_UNCOMMITTED)
            self._record_failure(
                "SQLx",
                "Unstaged changes in .sqlx/. Run 'just prepare-sqlx' and stage the result.",
            )
            return False

        success = self.run_step(
            "SQLx Prepare Check",
            ["cargo", "sqlx", "prepare", "--check"],
            cwd=self.repo_root / BACKEND_DIR,
        )
        self._set_metric("sqlx", STATUS_PASS if success else STATUS_STALE)
        return success

    def run_all(self):
        self.print_header("Quality Check Suite")

        if self.fast_mode:
            self._vprint(f"{INFO}⏩ Fast mode: skipping tests and build.{RESET}")
        elif self.skip_tests:
            self._vprint(
                f"{INFO}⏩ --skip-tests: skipping test execution; build/lint/format still run.{RESET}"
            )

        if self._throttle_notes:
            print(
                f"{INFO}🧠 Low-memory throttle: {'; '.join(self._throttle_notes)} "
                f"(override: KIT_CHECK_JOBS / KIT_CHECK_SEQUENTIAL).{RESET}"
            )

        # Mark groups that won't run as SKIPPED so the final report doesn't
        # render their "Pending" defaults as failures. These writes run on
        # the main thread before the executor starts, so they're race-free
        # by construction — but go through `_set_metric` anyway to honour
        # the helper-only-writes contract documented above.
        if self.format_only:
            for key in (
                "react_tests",
                "rust_lib",
                "rust_beh",
                "build",
                "sqlx",
                "clippy",
                "tsc",
            ):
                self._set_metric(key, STATUS_SKIPPED)
        elif self.frontend_only:
            for key in ("rust_lib", "rust_beh", "sqlx", "clippy", "rust_fmt"):
                self._set_metric(key, STATUS_SKIPPED)
        elif self.backend_only:
            for key in (
                "react_tests",
                "build",
                "lint",
                "biome",
                "prettier_docs",
                "tsc",
            ):
                self._set_metric(key, STATUS_SKIPPED)

        if self.format_only:
            self._run_format_only()
        elif self.frontend_only:
            self._run_frontend_group()
        elif self.backend_only:
            self._run_backend_group()
        elif self.sequential:
            self._run_frontend_group()
            self._run_backend_group()
        else:
            # Frontend (no cargo) and backend (cargo) groups don't share any
            # build artifacts or env state — running them on separate threads
            # roughly halves wall time on warm cache. Cargo serialises itself
            # via target/.cargo-lock so two cargo invocations in the same
            # process can't conflict; threads only race on shared Python state
            # (metrics / failures), which the lock guards.
            with ThreadPoolExecutor(max_workers=2) as ex:
                fe = ex.submit(self._run_frontend_group)
                be = ex.submit(self._run_backend_group)
                fe.result()
                be.result()

        self._print_stack_summary()
        self.print_report()
        return not self.suite_failed

    def _run_format_only(self):
        """Format-only: oxlint + biome + cargo fmt --check. Sub-second.
        Useful as a super-fast pre-flight before committing."""
        self._frontend_npm_check_step("lint", "Oxlint", ["npm", "run", "lint"])
        self._frontend_npm_check_step("biome", "Biome Check", ["npm", "run", "format"])
        self._frontend_npm_check_step(
            "prettier_docs", "Prettier Docs", _PRETTIER_DOCS_CMD
        )

        if not self._maybe_skip_for_stack(
            "rust_fmt", "Rust Fmt", self.cargo_toml, SKIP_BACKEND_ABSENT
        ):
            if self.run_step(
                "Rust Fmt",
                ["cargo", "fmt", "--check"],
                cwd=self.repo_root / BACKEND_DIR,
            ):
                self._set_metric("rust_fmt", STATUS_PASS)

    def _run_frontend_group(self):
        """Frontend steps: vitest, build, oxlint, biome, tsc."""
        run_tests = not self.fast_mode and not self.skip_tests

        if run_tests:
            if not self._maybe_skip_for_stack(
                "react_tests", "React Tests", self.package_json, SKIP_FRONTEND_ABSENT
            ):
                # --passWithNoTests: a scaffolded React stack with no test files
                # yet (mid-bootstrap, fresh feature start) would otherwise exit 1
                # from vitest's "No test files found" path and fail the suite.
                # The flag short-circuits to exit 0 in that case; harmless once
                # tests exist. See gh#27.
                if self.run_step(
                    "React Tests", ["npm", "test", "--", "--run", "--passWithNoTests"]
                ):
                    self._set_metric("react_tests", STATUS_PASS)

        if not self.fast_mode:
            if not self._maybe_skip_for_stack(
                "build", "Application Build", self.package_json, SKIP_FRONTEND_ABSENT
            ):
                if self.run_step("Application Build", ["npm", "run", "build"]):
                    self._set_metric("build", STATUS_PASS)

        self._frontend_npm_check_step("lint", "Oxlint", ["npm", "run", "lint"])
        self._frontend_npm_check_step("biome", "Biome Check", ["npm", "run", "format"])
        self._frontend_npm_check_step(
            "prettier_docs", "Prettier Docs", _PRETTIER_DOCS_CMD
        )

        if not self._maybe_skip_for_stack(
            "tsc", "TSC", self.package_json, SKIP_FRONTEND_ABSENT
        ):
            self._safe_print("  TSC...", flush=True)
            self._vprint(f"\n{INFO}▶ Running TypeScript Check (TSC)...{RESET}")
            tsc_res = subprocess.run(
                ["npx", "tsc", "--noEmit"],
                cwd=self.repo_root,
                capture_output=True,
                text=True,
                timeout=600,
            )
            if tsc_res.returncode == 0:
                self._vprint(f"{SUCCESS}✓ TSC: Pass{RESET}")
                self._set_metric("tsc", STATUS_PASS)
            else:
                err_count = len(re.findall(r"error TS", tsc_res.stdout))
                self._vprint(f"{WARNING}⚠️ TSC: {err_count} errors found{RESET}")
                self._set_metric("tsc", f"{err_count} errors")
                err_output = tsc_res.stdout.strip() if not self.verbose else ""
                self._record_failure("TSC", err_output)

    def _compute_cargo_jobs_env(self) -> dict[str, str]:
        """CARGO_BUILD_JOBS cap for cargo subprocesses, sized to available RAM
        so a low-memory machine doesn't spawn one rustc/linker per core and
        swap. Returns {} (cargo's default, unchanged) when RAM is ample,
        unreadable, or the cap would equal the CPU count — so CI and beefy dev
        boxes are unaffected.

        Env contract: KIT_CHECK_JOBS=N forces the cap to N on any machine (a
        value < 1 or non-integer is ignored, falling through to auto-detect);
        when the auto-cap is active it overrides any ambient CARGO_BUILD_JOBS,
        but leaves it untouched when it returns {} (gh#81)."""
        override = os.environ.get("KIT_CHECK_JOBS")
        if override:
            try:
                n = int(override)
            except ValueError:
                n = 0
            if n >= 1:
                return {"CARGO_BUILD_JOBS": str(n)}
        # Auto-cap only under memory pressure — the SAME gate as serialisation,
        # so a high-RAM machine (even a many-core one whose RAM/core ratio is
        # low) is never capped and never sees the throttle banner. CI, reading
        # the host's large MemAvailable, stays here too.
        if self._ram_gb is None or self._ram_gb > _MEMORY_PRESSURE_GB:
            return {}
        ncpu = os.cpu_count() or 1
        # One codegen/link job needs roughly 1–2 GiB at peak; budget ~2 GiB
        # each and never drop below 1.
        cap = min(ncpu, max(1, int(self._ram_gb // 2)))
        if cap >= ncpu:
            return {}  # no effective cap — leave cargo's default untouched
        return {"CARGO_BUILD_JOBS": str(cap)}

    def _cargo_env(self, **extra: str) -> dict[str, str]:
        """Env overrides for a cargo subprocess: the RAM-based job cap (if any)
        merged with step-specific vars like SQLX_OFFLINE."""
        return {**self.cargo_jobs_env, **extra}

    def _run_backend_group(self):
        """Backend steps: rust tests, sqlx, clippy, fmt."""
        run_tests = not self.fast_mode and not self.skip_tests

        if run_tests:
            if not self._maybe_skip_for_stack(
                "rust_lib",
                "Rust Lib Tests",
                self.cargo_toml,
                SKIP_BACKEND_ABSENT,
            ):
                if self.run_step(
                    "Rust Lib Tests",
                    ["cargo", "test", "--lib"],
                    cwd=self.repo_root / BACKEND_DIR,
                    env_update=self._cargo_env(SQLX_OFFLINE="true"),
                ):
                    self._set_metric("rust_lib", STATUS_PASS)

            if not self._maybe_skip_for_stack(
                "rust_beh",
                "Rust Behavior Tests",
                self.cargo_toml,
                SKIP_BACKEND_ABSENT,
            ):
                if self.run_step(
                    "Rust Behavior Tests",
                    ["cargo", "test", "--tests"],
                    cwd=self.repo_root / BACKEND_DIR,
                    env_update=self._cargo_env(SQLX_OFFLINE="true"),
                ):
                    self._set_metric("rust_beh", STATUS_PASS)

        self.check_sqlx()

        if not self._maybe_skip_for_stack(
            "clippy", "Clippy", self.cargo_toml, SKIP_BACKEND_ABSENT
        ):
            if self.run_step(
                "Clippy",
                ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
                cwd=self.repo_root / BACKEND_DIR,
                env_update=self._cargo_env(SQLX_OFFLINE="true"),
            ):
                self._set_metric("clippy", STATUS_PASS)

        if not self._maybe_skip_for_stack(
            "rust_fmt", "Rust Fmt", self.cargo_toml, SKIP_BACKEND_ABSENT
        ):
            if self.run_step(
                "Rust Fmt",
                ["cargo", "fmt", "--check"],
                cwd=self.repo_root / BACKEND_DIR,
            ):
                self._set_metric("rust_fmt", STATUS_PASS)

    def _print_stack_summary(self) -> None:
        """Group skipped-for-stack checks by reason and print a consolidated notice.
        Makes "the stack is partial — these checks didn't run" highly visible
        rather than buried in inline output."""
        if not self._skipped_for_stack:
            return

        by_reason: defaultdict[str, list[str]] = defaultdict(list)
        for reason, check in self._skipped_for_stack:
            by_reason[reason].append(check)

        total = len(self._skipped_for_stack)
        print(
            f"\n{INFO}ℹ Stack components not detected — {total} check{'s' if total != 1 else ''} skipped:{RESET}"
        )
        # Sort outer reasons and inner check names so the summary is stable
        # across parallel runs (workers append in non-deterministic order).
        for reason in sorted(by_reason):
            checks = sorted(by_reason[reason])
            joined = ", ".join(checks)
            print(f"{INFO}  • {reason} → {joined} ({len(checks)}){RESET}")
        print(
            f"{INFO}\n  Once you scaffold the stack, these checks will activate automatically.{RESET}"
        )

    def _format_status(self, value: str) -> str:
        """Render a metric value into a coloured status cell.
        Variable-detail warnings (e.g. "3 errors") are recognised by suffix,
        not substring — `endswith` won't false-positive on a step *name* that
        happens to contain the word."""
        if value == STATUS_PASS:
            return f"{SUCCESS}✅ Pass{RESET}"
        if value == STATUS_SKIPPED:
            return f"{INFO}⏩ Skipped{RESET}"
        if value == STATUS_PENDING:
            return f"{FAILURE}❌ Fail{RESET}"
        if (
            value in (STATUS_STALE, STATUS_UNCOMMITTED)
            or value.endswith(" errors")
            or value.endswith(" warnings")
        ):
            return f"{WARNING}⚠️ {value}{RESET}"
        return f"{FAILURE}❌ {value}{RESET}"

    def print_report(self):
        print(f"\n{INFO}🚀 Quality Report{RESET}")
        print(f"| {'Check':<20} | {'Status':<30} |")
        print(f"|{'-' * 22}|{'-' * 32}|")

        for key, value in self.metrics.items():
            name = key.replace("_", " ").capitalize()
            # Pad the status cell to 30 *visible* columns so the table
            # aligns whether ANSI codes are present or stripped (NO_COLOR).
            # `f"{s:<30}"` pads by string length, which under NO_COLOR
            # padded to 30 but with color codes inflated the cell to ~40.
            status_str = _pad_visible(self._format_status(value), 30)
            print(f"| {name:<20} | {status_str} |")

        if self.suite_failed:
            # Failures go to stderr so the report can be redirected /
            # piped without dragging the error block into downstream
            # consumers. Compare merge.py's stderr discipline.
            print(f"\n{FAILURE}❌ SUITE FAILED{RESET}", file=sys.stderr)
            if self.failures:
                print(f"\n{INFO}— Failure details —{RESET}", file=sys.stderr)
                for step, output in self.failures.items():
                    print(f"\n{FAILURE}▶ {step}{RESET}", file=sys.stderr)
                    print(output, file=sys.stderr)
        else:
            print(f"\n{SUCCESS}✨ ALL CHECKS PASSED{RESET}\n")


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Kit quality check — runs lint, format, tests, and build.",
    )
    parser.add_argument(
        "--fast",
        action="store_true",
        help="Lint + format only (skip tests and build); used by pre-commit hook",
    )
    parser.add_argument(
        "--skip-tests",
        dest="skip_tests",
        action="store_true",
        help="Skip only test execution; build, lint, and format still run",
    )
    parser.add_argument(
        "--sequential",
        action="store_true",
        help="Run frontend and backend groups serially (default: parallel)",
    )
    parser.add_argument(
        "--format",
        dest="format_only",
        action="store_true",
        help="Sub-second pre-flight: oxlint + biome + cargo fmt --check only",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Stream subprocess output instead of capturing it",
    )
    parser.add_argument(
        "--strict",
        dest="strict_mode",
        action="store_true",
        help=(
            "Promote stack-marker skips to failures. "
            "Used by release.py to catch accidentally-deleted markers."
        ),
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--frontend",
        action="store_true",
        help="Run frontend group only (vitest, build, oxlint, biome, tsc)",
    )
    group.add_argument(
        "--backend",
        action="store_true",
        help="Run backend group only (cargo test, sqlx, clippy, fmt)",
    )
    return parser.parse_args(argv)


if __name__ == "__main__":
    args = _parse_args()
    checker = QualityChecker(
        fast_mode=args.fast,
        verbose=args.verbose,
        sequential=args.sequential,
        frontend_only=args.frontend,
        backend_only=args.backend,
        format_only=args.format_only,
        skip_tests=args.skip_tests,
        strict_mode=args.strict_mode,
    )
    if not checker.run_all():
        sys.exit(1)
