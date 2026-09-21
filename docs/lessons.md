# Lessons

Empirical failures the codebase has already paid for. Citable as `L-NNN` from commit messages, CLAUDE.md, reviewer findings.

Append-only; supersede in place if the underlying ecosystem changes.

---

## L-001 — Tauri NSIS bundler walks `src-tauri/src/bin/`

**First observed**: 2026-05-08 (a Windows release build)
**Recurrences**: 2026-05-13 (a sibling Tauri project), 2026-05-20

**Symptom** — Windows release bundle fails with:

> `failed to bundle project when getting size of …/release/{name}.exe: The system cannot find the file specified.`

**Trigger** — A `.rs` file in `src-tauri/src/bin/` whose binary won't be in `target/release/` at bundle time. `[[bin]]` + `required-features` is the common way to trip it: cargo skips the build, bundler still expects the artifact.

**Root cause** — The NSIS bundler enumerates `src-tauri/src/bin/` on disk and expects every entry to produce a bundled `.exe`. It does not consult `Cargo.toml`'s `required-features` flag.

**Mitigation** — Dev-only binaries live in `src-tauri/dev/`, declared as `[[bin]] path = "dev/{name}.rs"`. Feature gating is orthogonal and does not solve the bundler problem.

**Guardrail** (optional CI lint):

```bash
[ -z "$(ls src-tauri/src/bin/ 2>/dev/null)" ] \
  || { echo "src-tauri/src/bin/ must be empty — see L-001"; exit 1; }
```

---

## L-002 — `taiki-e/install-action` tool versions float when unpinned

**First observed**: 2026-05-22 (the CI backend job)
**Resolved by**: this commit (pin `sqlx-cli@0.8.6` in `.github/workflows/ci.yml`)

**Symptom** — Backend CI fails instantly with:

> `error: \`--database-url\` or \`DATABASE_URL\` must be set`
>
> at `cargo sqlx prepare --check`, despite `SQLX_OFFLINE=true` being set in `src-tauri/.cargo/config.toml [env]`.

**Trigger** — `taiki-e/install-action` with `tool: <name>` (no `@version` suffix). Each CI run resolves to whatever's latest on the tool's GitHub releases at run time. A point release of the tool flips behaviour silently between two otherwise-identical pipeline runs.

**Root cause** — `sqlx-cli` 0.9.0 (released between two CI runs) requires `DATABASE_URL` for `prepare --check` even when `SQLX_OFFLINE=true`. v0.8.6 honoured the offline flag. The action SHA is pinned for supply-chain safety; the tool name is not.

**Mitigation** — Always pin tool versions to match the runtime dependency: `tool: sqlx-cli@0.8.6` matches `sqlx = "0.8"` in `Cargo.toml`. Bump deliberately when the dependency moves. Same discipline applies to any other tool installed via `taiki-e/install-action` (`cargo-tarpaulin`, etc.) — currently uses default; consider pinning when next surprise lands.

---

## L-007 — Local E2E green is not CI green when the code branches on a host service

**First observed**: 2026-06-11 (suite passed twice locally, failed on CI minutes after merge)

**Symptom** — An E2E spec green in repeated local headless runs fails on CI, on an assertion right after an action whose behavior depends on an OS service — here the keychain: with no Secret Service on the runner, the save fell back to a lower storage tier whose UI flow is legitimately different, and the spec had asserted the dev-host variant only.

**Mitigation** — (1) When a code path branches on host-service availability, assert only what is identical across all environment-legal variants (or accept any of them explicitly). (2) Before trusting local runs for such a path, reproduce the CI host: `DBUS_SESSION_BUS_ADDRESS=disabled: just test-e2e-headless` makes anything Secret-Service-dependent see "unavailable", exactly like CI. Generalizes to any host-coupled dependency — locale, display server, network: find the env knob that recreates the CI condition and run the suite under it. Fixed in `2091460`.

## L-008 — An external API's "access denied" can be origin-gated, not credential-gated

**First observed**: 2026-06-12 (a price provider that "needed a key" was actually blocking by IP)

**Symptom** — A read-only HTTP API returned access-denied for every request; the natural reading was "authentication required, get a key." Acquiring/sending a key changed nothing, because the gate keyed on the _request origin_ (IP/ASN allow-list), not on any credential. The same endpoint served data fine from a different network and rejected a valid key from a datacenter IP.

**Mitigation** — Before concluding an API needs auth, probe it from the _actual deployment network_ (a tool call's egress IP may differ from the user's), and test the keyed and keyless requests from the _same_ origin to isolate the variable. When the gate turns out to be origin-based and no key fixes it, the credential machinery is wasted complexity: prefer a provider whose documented JSON endpoint is permissive over scraping/auth gymnastics. This is the reasoning behind ADR-017.

## L-009 — Font metrics differ between local and CI, so borderline flex layouts fail only in CI

**First observed**: 2026-07-04 (account-details header overflow → "element click intercepted" in 3 E2E specs)

**Symptom** — Repeated local headless E2E runs green; the same suite red on CI with WebDriver "element click intercepted" on buttons in a dense flex row. Same app, same window size, same xvfb wrapper. The variable was the runner's installed fonts: wider fallback glyph metrics pushed a borderline `whitespace-nowrap` stats block into overlapping the sibling button group — locally the same row fit by a few pixels.

**Mitigation** — (1) Treat "element click intercepted" appearing across several unrelated specs as a layout-overflow signal, not per-spec flakiness — look for what the failing clicks share spatially (here: all targets lived in one header row). (2) Make dense rows wrap-tolerant (`flex-wrap` on the row and its groups) instead of relying on the current viewport fitting; nowrap text inside a `min-w-0` flex child overflows _over_ siblings rather than clipping. (3) Gate a release tag on the CI E2E run of the merge commit, not only on local E2E — text-metric-sensitive layouts are exactly the class of breakage only CI reveals.

## L-010 — A CI job timeout must budget the cold cache-miss build, not the warm one

**First observed**: 2026-07-13 (a backend coverage job cancelled at 30m with every test green)

**Symptom** — A coverage job that runs ~21 minutes on a warm dependency cache was cancelled at its 30-minute limit; the log showed tests passing steadily right up to the cutoff, plus orphaned tooling processes at cleanup — which reads like a hang but is just whatever was mid-flight when the axe fell. The trigger: the PR changed the lockfile (new dependency + a feature flag on an existing one), invalidating the dependency cache and forcing a cold instrumented rebuild.

**Mitigation** — (1) Before diagnosing a "hung" CI job, check whether steady progress was still being logged at cancellation — a timeout mid-progress is a budget problem, not a deadlock. (2) Size `timeout-minutes` for the cold-cache path (lockfile changes are routine), keeping headroom of roughly the warm duration's half. (3) A local run of the same tool over the suspect tests separates "genuinely hangs" from "ran out of time" in minutes.

## L-011 — Before bisecting a local-only E2E failure, run a known-green tag on the same machine

**First observed**: 2026-08-23 (multi-device sync PR-E: every E2E spec that writes through IPC timed out locally while CI on the same commit was green)

**Symptom** — Write commands invoked from E2E (`execute/async` seeds, a form submit) never resolved locally: WebDriver script timeouts, a modal stuck in its submitting state. Backend probes showed the command completing in milliseconds; read-only specs passed. It looked like a regression in the freshly merged feature, and an hour went into instrumenting it.

**Mitigation** — (1) When CI is green on the same commit, first run one E2E spec from a known-green release tag in a `git worktree` on the same machine; if it fails the same way, the environment is the variable (here: the local WebKitGTK/driver stack losing IPC responses under load) and the bisect is pointless. (2) Kill stale drivers with `pkill -x <name>`, never `pkill -f <pattern>` — the pattern matches the shell running the command and kills it (exit 144), silently skipping everything after it. (3) Gate the merge on the CI E2E run (the suite runs on the main push) and fix forward if it reddens.

## L-012 — Every gating workflow needs a manual trigger, because GitHub silently drops pull-request events

**First observed**: 2026-09-13 (a pull request opened during a GitHub 502 window got no runs at all; the next one lost the events for two consecutive pushes and a close/reopen while the status page read "all systems operational")

**Symptom** — A pull request whose head commit has zero check runs: no queued job, no failed job, nothing in the run list for that commit. The branch tip and the PR head agree, so nothing is wrong on the repository's side. With checks as the merge gate, the PR is stuck until something fires them.

**Mitigation** — (1) Give every workflow a required check depends on a `workflow_dispatch` entry, with an input for the pull request number when the workflow posts to the PR, and a base-branch fallback for expressions that read the pull-request context. (2) Fire the missing runs with `gh workflow run <name> --ref <branch>` and watch the branch tip's check runs through `gh api …/commits/<sha>/check-runs`; a closed-and-reopened PR is not a reliable re-trigger. (3) Remember the dispatch entry only becomes usable once the workflow file carrying it is on the default branch — a new gating workflow must land with it from its first version.

## L-013 — An in-place mutation run that dies leaves a mutated source file behind

**First observed**: 2026-09-13 (a local `cargo mutants --in-place` timing run was killed by the OS for memory on a 6 GB laptop; `git status` then showed one changed line in a service file nobody had edited)

**Symptom** — After a mutation-testing run that did not end on its own (killed, out of memory, Ctrl-C), a source file carries a one-line change that reads like a bug: a flipped comparison, a returned default, a deleted call. The tool restores files only when it finishes a mutant.

**Mitigation** — (1) Treat in-place mutation as a CI job, not a laptop task: it needs the memory of a full build plus the suite, repeatedly. (2) After any run that did not finish, `git status` before every other git command; `git checkout -- <tree>` restores the file. Never stash, commit or switch branches while such a run is alive. (3) A run that must stay local uses the copying mode, which leaves the working tree untouched.

## L-014 — An overlay scrollbar fades on its own schedule and breaks a screenshot comparison

**First observed**: 2026-09-21 (a Markdown-only pull request failed the visual gate twice on `sync-settings`, against a baseline built from its own parent commit)

**Symptom** — A visual-regression gate reports a difference on screens the change cannot reach — a documentation-only pull request, or a backend one. The diff is a thin vertical bar at the right edge of a scrollable pane, a few pixels wide over several hundred rows, present in one capture and absent in the other although both ran the same code. A second run reproduces the same percentage, so it reads as systematic rather than flaky, and the obvious suspects — a stale baseline, a changed dependency — all check out clean.

**Root cause** — WebKitGTK draws overlay scrollbars that fade in when a pane is touched and out again shortly after. Whether the capture lands before or after that fade is decided by the run's timing, not by the code. The pane only has to become scrollable for the screen to acquire the behaviour, so the regression appears in a change that never touched the screen. Measured here at 0.22 % of an 800×600 screen, against a 0.3 % threshold — on its own under the bar, and over it as soon as any other small difference joined, such as a temporary directory name rendered into the page.

**Mitigation** — (1) Hide scrollbars in the capture path, alongside the transitions and hover states a capture already neutralises: `scrollbar-width: none` and `::-webkit-scrollbar { display: none }` injected for the shot and removed afterwards. (2) When a gate accuses a change that cannot have caused it, compare the two artifacts pixel by pixel and locate the differing columns before believing either verdict — the column range names the culprit in one step. (3) Keep run-varying text (temporary paths, clock times) out of captured screens, so the threshold's headroom is spent on real regressions. (4) Once the fix to the capture path is on the default branch, every open pull request needs it before its comparison means anything: the baseline is rebuilt from the default branch, so a branch that predates the fix captures the old way and is failed by the very correction meant to clear it. Bring the fix into the branch first — and where the rules forbid rewriting a pushed branch, replay the commit onto a fresh one rather than re-running the old head and reading the result as a verdict.
