#!/usr/bin/env python3
"""
Release script.

Automates version bumping, changelog generation, and git tagging for the
current project.

Process:
  1. Run all quality checks via check.py (tests, lint, SQLx, build)
  2. Analyze git history since last tag
  3. Determine version bump using semver
  4. Update version in package.json, package-lock.json, Cargo.toml, and tauri.conf.json
  5. Create/update CHANGELOG.md
  6. Format files via just format
  7. Create commit and git tag

Usage:
  python3 release.py [--dry-run] [--preview] [--version X.Y.Z] [-y]

Options:
  --dry-run           Preview release without making changes (still updates files locally + creates local commit/tag)
  --preview           Read-only: print the suggested version and exit. Skips tests, file edits, commits, pushes.
  --version X.Y.Z     Force a specific version instead of auto-calculating from commits
  -y, --yes           Skip confirmation prompt (auto-confirm suggested version)
"""

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path

from check import release_validation

# ANSI colors (respect NO_COLOR=1)
if os.environ.get("NO_COLOR"):
    GREEN = YELLOW = RED = BLUE = NC = ""
else:
    GREEN = "\033[0;32m"
    YELLOW = "\033[0;33m"
    RED = "\033[0;31m"
    BLUE = "\033[0;34m"
    NC = "\033[0m"

# Changelog constants
CHANGELOG_INTRO = (
    "All notable changes to this project will be documented in this file.\n\n"
    "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),\n"
    "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)."
)

# Default branch this release script targets. Override in a downstream fork
# if the project uses `master` / `trunk` / etc.
MAIN_BRANCH = "main"
# What the release commit holds, and nothing else.
RELEASE_FILES = (
    "package.json",
    "package-lock.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
    "src-tauri/tauri.conf.json",
    "CHANGELOG.md",
)


class Mode(Enum):
    """Run mode for the release manager.

    REAL: full release (tests, edits, commit, tag, push).
    DRY_RUN: edits files locally + creates local commit/tag, skips push.
    PREVIEW: read-only; prints suggested version and exits.
    """

    REAL = "real"
    DRY_RUN = "dry_run"
    PREVIEW = "preview"


class _Resolution(Enum):
    """Outcome of the version-resolution phase, dictating run()'s next move."""

    READY = "ready"  # new_version is set; proceed to _apply_changes
    EARLY_OK = "early_ok"  # exit 0 (preview, or preview+no-commits)
    EARLY_FAIL = (
        "early_fail"  # exit 1 (tests failed, no commits in non-preview, user cancelled)
    )


def set_lockfile_version(lockfile: Path, version: str) -> None:
    """The npm lockfile states the version it locks: its own `version` and its root
    package's (DEBT-035). Written back the way npm writes it, so nothing else moves."""
    data = json.loads(lockfile.read_text(encoding="utf-8"))
    data["version"] = version
    root = data.get("packages", {}).get("")
    if root is not None:
        root["version"] = version
    lockfile.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


RUN_PASSED = {"success", "skipped", "neutral"}


def runs_in_the_way(runs: list[dict]) -> list[str]:
    """FLOW-031 — what stops a tag on a commit, from the workflow runs GitHub lists for
    it: no run at all, a run that has not ended, or one that did not pass. The newest run
    of each workflow counts. One line per obstacle, naming the run; empty when every
    workflow ended green."""
    newest: dict[str, dict] = {}
    for run in runs:
        name = run.get("name") or ""
        if name not in newest or run.get("id", 0) > newest[name].get("id", 0):
            newest[name] = run
    if not newest:
        return ["no workflow run is listed for this commit yet"]
    obstacles = []
    for name in sorted(newest):
        run = newest[name]
        where = run.get("html_url") or f"run {run.get('id')}"
        if run.get("status") != "completed":
            obstacles.append(f"{name}: not finished ({run.get('status')}) — {where}")
        elif run.get("conclusion") not in RUN_PASSED:
            obstacles.append(f"{name}: {run.get('conclusion')} — {where}")
    return obstacles


def changelog_line(commit: dict) -> str:
    """A commit's changelog line: its scope, when it has one, before its title (#672)."""
    scope = (commit["scope"] or "").strip("()")
    return f"[{scope}] {commit['description']}" if scope else commit["description"]


class ReleaseManager:
    def __init__(
        self,
        mode: Mode = Mode.REAL,
        forced_version: str | None = None,
        yes: bool = False,
    ):
        self.repo_root = Path(__file__).parent.parent
        self.current_version = self.get_current_version()
        self.commits: list[dict] = []
        self.breaking_changes = 0
        self.features = 0
        self.fixes = 0
        self.new_version: str | None = None
        self.mode = mode
        self.forced_version = forced_version
        self.yes = yes

    def get_current_version(self) -> str:
        """Get current version from package.json."""
        package_json = self.repo_root / "package.json"
        with open(package_json, encoding="utf-8") as f:
            data = json.load(f)
        return data["version"]

    def get_latest_tag(self) -> str | None:
        """Get the latest git tag."""
        try:
            result = subprocess.run(
                ["git", "describe", "--tags", "--abbrev=0"],
                cwd=self.repo_root,
                capture_output=True,
                text=True,
                check=True,
            )
            return result.stdout.strip()
        except subprocess.CalledProcessError:
            return None

    def get_commits_since_tag(self, tag: str | None) -> list[dict]:
        """Get commits since the last tag (subject + body to capture BREAKING CHANGE footers)."""
        commit_range = f"{tag}..HEAD" if tag else "HEAD"

        result = subprocess.run(
            ["git", "log", commit_range, "--pretty=format:%s%n%b%x00"],
            cwd=self.repo_root,
            capture_output=True,
            text=True,
            check=True,
        )

        entries = [e.strip() for e in result.stdout.split("\x00") if e.strip()]
        return [self.parse_commit_message(entry) for entry in entries]

    def parse_commit_message(self, message: str) -> dict:
        """Parse conventional commit message format: type[(scope)][!]: description."""
        match = re.match(
            r"^(feat|fix|docs|chore|refactor|test|ci)(\(.+\))?(!)?: (.+)$",
            message,
            re.DOTALL,
        )

        if not match:
            return {"type": "other", "scope": None, "description": message}

        commit_type, scope, bang, description = match.groups()
        is_breaking = bang == "!" or "BREAKING CHANGE" in message
        # Changelog bullets are user-facing — some projects render CHANGELOG.md
        # in an in-app "What's new" dialog. Keep only the title line; the body
        # (rule IDs, layer notes, rationale) stays developer-facing in git
        # history via `original`, never leaking into the changelog (gh86).
        title = description.splitlines()[0].strip()

        return {
            "type": commit_type,
            "scope": scope,
            "description": title,
            "breaking": is_breaking,
            "original": message,
        }

    def analyze_commits(self, commits: list[dict]) -> None:
        """Count breaking changes, features, and fixes."""
        self.commits = commits

        for commit in commits:
            if commit.get("breaking"):
                self.breaking_changes += 1
            elif commit["type"] == "feat":
                self.features += 1
            elif commit["type"] == "fix":
                self.fixes += 1

    def calculate_new_version(self, current: str) -> str:
        """Calculate new version based on semver rules."""
        major, minor, patch = map(int, current.split("."))

        if self.breaking_changes > 0:
            major += 1
            minor = patch = 0
        elif self.features > 0:
            minor += 1
            patch = 0
        elif self.fixes > 0:
            patch += 1

        return f"{major}.{minor}.{patch}"

    def _format_mode_prefix(self) -> str:
        """Return dry-run prefix if applicable."""
        return f"{BLUE}[DRY-RUN]{NC} " if self.mode is Mode.DRY_RUN else ""

    def show_analysis(self) -> None:
        """Display release analysis."""
        print(f"\n{BLUE}=== Release Analysis ==={NC}")
        print(f"Current version: {BLUE}{self.current_version}{NC}")
        print(f"Latest tag: {BLUE}{self.get_latest_tag() or 'none'}{NC}")
        print("\nCommits since last release:")
        print(f"  {BLUE}Breaking changes: {self.breaking_changes}{NC}")
        print(f"  {GREEN}Features: {self.features}{NC}")
        print(f"  {BLUE}Fixes: {self.fixes}{NC}")
        print(f"\nSuggested version: {GREEN}{self.new_version}{NC}\n")

    def ask_confirmation(self) -> bool:
        """Ask user to confirm release. 'v' allows version override."""
        while True:
            response = (
                input(f"{BLUE}Confirm release v{self.new_version}? (y/n/v): {NC}")
                .lower()
                .strip()
            )

            if response == "y":
                return True
            elif response == "n":
                return False
            elif response == "v":
                self.ask_version_override()
                self.show_analysis()
            else:
                print("Invalid input. Use y (yes), n (no), or v (version override)")

    def ask_version_override(self) -> None:
        """Prompt user to manually set version."""
        while True:
            version = input(f"{BLUE}Enter version (e.g., 0.2.0): {NC}").strip()
            if re.match(r"^\d+\.\d+\.\d+$", version):
                self.new_version = version
                break
            print("Invalid version format. Use X.Y.Z")

    def _update_json_file(self, file_path: Path, key: str) -> None:
        """Update version key in JSON file."""
        with open(file_path, encoding="utf-8") as f:
            data = json.load(f)
        data[key] = self.new_version
        with open(file_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
            f.write("\n")

    def update_version_files(self) -> None:
        """Update version in package.json, package-lock.json, Cargo.toml, and tauri.conf.json.

        Raises:
            RuntimeError: if the Cargo.toml [package].version regex matches != 1 site
                (refuses to write a corrupted file). package.json is already mutated
                at this point; the user must `git checkout --` to revert.
        """
        prefix = self._format_mode_prefix()
        print(f"{BLUE}{prefix}Updating version files...{NC}")

        if self.mode is Mode.DRY_RUN:
            print("  → package.json")
            print("  → package-lock.json")
            print("  → src-tauri/Cargo.toml")
            print("  → src-tauri/tauri.conf.json")
            return

        self._update_json_file(self.repo_root / "package.json", "version")
        print("  ✓ package.json")
        set_lockfile_version(self.repo_root / "package-lock.json", self.new_version)
        print("  ✓ package-lock.json")

        cargo_toml = self.repo_root / "src-tauri" / "Cargo.toml"
        content = cargo_toml.read_text(encoding="utf-8")
        # Anchor replacement to the [package] section to avoid patching dependency
        # versions. Bail loud if the regex matches anything other than exactly one
        # site — silent multi-replace would corrupt downstream dependency pins.
        content, n_replaced = re.subn(
            r'(\[package\].*?version\s*=\s*")[^"]+(")',
            rf"\g<1>{self.new_version}\2",
            content,
            flags=re.DOTALL,
        )
        if n_replaced != 1:
            # package.json is already mutated; surface the recovery hint before
            # raising so the user sees it before the Python traceback.
            print(
                f"{RED}❌ Cargo.toml [package].version replace matched {n_replaced} "
                f"sites (expected exactly 1) — refusing to write a corrupted file.{NC}",
                file=sys.stderr,
            )
            print(
                f"{BLUE}   package.json already edited; "
                f"`git checkout -- package.json` to revert.{NC}",
                file=sys.stderr,
            )
            raise RuntimeError(
                f"Cargo.toml [package].version replace matched {n_replaced} sites"
            )
        cargo_toml.write_text(content, encoding="utf-8")
        print("  ✓ src-tauri/Cargo.toml")

        self._update_json_file(
            self.repo_root / "src-tauri" / "tauri.conf.json", "version"
        )
        print("  ✓ src-tauri/tauri.conf.json")

        print(f"{BLUE}  Updating src-tauri/Cargo.lock...{NC}")
        try:
            subprocess.run(
                ["cargo", "metadata", "--format-version", "1"],
                cwd=self.repo_root / "src-tauri",
                capture_output=True,
                check=True,
            )
        except subprocess.CalledProcessError as e:
            # JSON files are already mutated above; if cargo crashes here
            # the working tree is partial. Surface clearly so the user
            # knows to inspect Cargo.toml (most likely a syntax issue
            # introduced by the version edit) and revert if needed.
            print(
                f"{RED}❌ cargo metadata failed — Cargo.lock not updated.{NC}",
                file=sys.stderr,
            )
            detail = (e.stderr or e.stdout or "").strip()
            if detail:
                print(f"{RED}{detail}{NC}", file=sys.stderr)
            print(
                f"{BLUE}   Version files already edited; inspect Cargo.toml syntax or "
                f"`git checkout -- package.json package-lock.json src-tauri/Cargo.toml src-tauri/tauri.conf.json` to revert.{NC}",
                file=sys.stderr,
            )
            sys.exit(1)
        print("  ✓ src-tauri/Cargo.lock updated")

    def _build_changelog_entry(self) -> str:
        """Build new changelog entry from commits."""
        today = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        entry = f"## [{self.new_version}] - {today}\n"

        if self.breaking_changes > 0:
            entry += "\n### ⚠️ BREAKING CHANGES\n"
            entry += f"- {self.breaking_changes} breaking change(s)\n"

        if self.features > 0:
            entry += "\n### Added\n"
            for commit in self.commits:
                if commit["type"] == "feat":
                    entry += f"- {changelog_line(commit)}\n"

        if self.fixes > 0:
            entry += "\n### Fixed\n"
            for commit in self.commits:
                if commit["type"] == "fix":
                    entry += f"- {changelog_line(commit)}\n"

        return entry + "\n"

    def update_changelog(self) -> None:
        """Create or update CHANGELOG.md with new version entry."""
        prefix = self._format_mode_prefix()
        print(f"{BLUE}{prefix}Updating CHANGELOG.md...{NC}")

        if self.mode is Mode.DRY_RUN:
            print("  → CHANGELOG.md")
            return

        changelog = self.repo_root / "CHANGELOG.md"
        new_entry = self._build_changelog_entry()

        if changelog.exists():
            existing = changelog.read_text(encoding="utf-8")
            if existing.startswith("# Changelog"):
                lines = existing.split("\n")
                header_end = next(
                    (i for i, line in enumerate(lines) if line.startswith("## [")), 0
                )

                if header_end > 0:
                    header = "\n".join(lines[:header_end])
                    rest = "\n".join(lines[header_end:])
                    content = f"{header}\n{new_entry}{rest}"
                else:
                    content = existing + new_entry
            else:
                content = new_entry + existing
        else:
            content = f"# Changelog\n\n{CHANGELOG_INTRO}\n\n{new_entry}"

        changelog.write_text(content, encoding="utf-8")
        print("  ✓ CHANGELOG.md")

    def format_files(self) -> bool:
        """Run 'just format' to ensure CHANGELOG and code are clean."""
        prefix = self._format_mode_prefix()
        print(f"{BLUE}{prefix}Running formatters via just...{NC}")

        if self.mode is Mode.DRY_RUN:
            print("  → just format")
            return True

        try:
            subprocess.run(
                ["just", "format"],
                cwd=self.repo_root,
                check=True,
                capture_output=True,
                text=True,
            )
            print("  ✓ Files formatted")
            return True
        except subprocess.CalledProcessError as e:
            detail = (e.stderr or e.stdout or "").strip()
            print(
                f"{RED}❌ Error during format: {detail or e}{NC}",
                file=sys.stderr,
            )
            return False
        except FileNotFoundError:
            print(f'{YELLOW}⚠ "just" command not found. Skipping format.{NC}')
            return True

    def release_branch(self) -> str:
        return f"chore/release-v{self.new_version}"

    def _git(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(["git", *args], cwd=self.repo_root, capture_output=True, text=True, check=False)

    def on_fresh_main(self) -> bool:
        """A release starts from main as GitHub holds it: the release branch is cut from
        here, and the commits since the last tag are read here."""
        branch = self._git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
        if branch != MAIN_BRANCH:
            print(f'{RED}❌ Current branch is "{branch}", not "{MAIN_BRANCH}". Release must be run from {MAIN_BRANCH}.{NC}')
            return False
        fetched = self._git("fetch", "--quiet", "origin", MAIN_BRANCH).returncode == 0
        here = self._git("rev-parse", "HEAD").stdout.strip()
        there = self._git("rev-parse", f"origin/{MAIN_BRANCH}").stdout.strip()
        if not fetched or here != there:
            print(f"{RED}❌ Local {MAIN_BRANCH} is not origin/{MAIN_BRANCH}: git pull --ff-only, then release.{NC}")
            return False
        edited = self._git("status", "--porcelain", "--", *RELEASE_FILES).stdout.strip()
        if edited:
            print(f"{RED}❌ A file the release commit holds has uncommitted edits; commit or drop them first:{NC}")
            print(f"{RED}{edited}{NC}")
            return False
        return True

    def recovery(self, number: str | None = None) -> str:
        """What to do when the release stops between its push and its tag: the pull request
        may still merge, or have merged, so both ways out are named."""
        branch, tag = self.release_branch(), f"v{self.new_version}"
        state = f"gh pr view {number} --json state" if number else f"gh pr list --head {branch} --state all"
        return (
            f"Nothing is tagged. See where the pull request stands: {state}. "
            f"Merged (or merge it once green: gh pr merge <n> --rebase): git switch {MAIN_BRANCH} && git pull --ff-only && "
            f"git log -1 --format=%s should say 'chore: release {tag}', then "
            f"git tag -a {tag} -m 'Version {self.new_version}' {MAIN_BRANCH} && git push origin refs/tags/{tag}. "
            f"To drop it instead: gh pr close <n> --delete-branch (if one is open); "
            f"git switch {MAIN_BRANCH} && git branch -D {branch}."
        )

    def commit_release(self) -> bool:
        """Commit the version changes on a branch of their own: main takes them only
        through a pull request whose checks passed."""
        prefix = self._format_mode_prefix()
        print(f"{BLUE}{prefix}Creating the release commit...{NC}")

        if self.mode is Mode.DRY_RUN:
            print(f"  → Branch: {self.release_branch()}")
            print(f"  → Commit: chore: release v{self.new_version}")
            return True

        try:
            subprocess.run(
                ["git", "switch", "--quiet", "-c", self.release_branch()],
                cwd=self.repo_root,
                check=True,
            )
            subprocess.run(["git", "add", *RELEASE_FILES], cwd=self.repo_root, check=True)

            # --no-verify intentionally skips the commit-msg + pre-commit hooks.
            # Waiver: the commit message is fixed by this script (conventional
            # "chore: release vX.Y.Z") so the commit-msg lint is a no-op, and
            # run_tests() already passed the full quality suite so the pre-commit
            # lint pass would be redundant. The pull request's checks read it again.
            subprocess.run(
                [
                    "git",
                    "commit",
                    "--no-verify",
                    "-m",
                    f"chore: release v{self.new_version}",
                ],
                cwd=self.repo_root,
                check=True,
            )
            print(f"  ✓ Commit created on {self.release_branch()}")
            return True
        except subprocess.CalledProcessError as e:
            print(f"{RED}Error: {e}{NC}", file=sys.stderr)
            detail = (e.stderr or e.stdout or "").strip()
            if detail:
                print(f"{RED}{detail}{NC}", file=sys.stderr)
            print(
                f"{BLUE}   Nothing was pushed. To start again: git switch {MAIN_BRANCH}; "
                f"git restore --source=HEAD --staged --worktree -- {' '.join(RELEASE_FILES)}; git branch -D {self.release_branch()}{NC}",
                file=sys.stderr,
            )
            return False

    def head_runs_are_green(self) -> bool:
        """FLOW-031 — refuse to release a commit whose workflow runs have not all ended
        green: the checks of the last merge are part of what is released."""
        if self.mode is Mode.DRY_RUN:
            print(f"{BLUE}[DRY-RUN] Skipping the workflow runs of the head commit{NC}")
            return True
        try:
            sha = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=self.repo_root, capture_output=True, text=True, check=True,
            ).stdout.strip()
            listed = subprocess.run(
                ["gh", "api", f"repos/{{owner}}/{{repo}}/actions/runs?head_sha={sha}&per_page=100"],
                cwd=self.repo_root, capture_output=True, text=True, check=True,
            ).stdout
            runs = json.loads(listed).get("workflow_runs", [])
        except (subprocess.CalledProcessError, OSError, json.JSONDecodeError) as error:
            print(f"{RED}❌ The workflow runs of the head commit cannot be read: {error}{NC}")
            return False
        obstacles = runs_in_the_way(runs)
        if obstacles:
            print(f"{RED}❌ The workflow runs of {sha[:7]} have not all ended green:{NC}")
            for line in obstacles:
                print(f"{RED}   {line}{NC}")
            print(f"{BLUE}   Wait for them (`just watch-pr` knows how), or fix what failed, then release.{NC}")
            return False
        print(f"{GREEN}✅ Every workflow run of {sha[:7]} ended green.{NC}")
        return True

    def run_tests(self) -> bool:
        """Run the release's validation (`check.release_validation`): the full suite,
        strict, so any stack-marker skip (e.g. accidentally deleted .sqlx/ when
        migrations exist) blocks the release instead of being silently tolerated.
        `just release-check` runs the same at the hand-over of a batch."""
        print(f"{BLUE}🚀 Running full quality validation (--strict)...{NC}")

        if self.mode is Mode.DRY_RUN:
            print(f"{BLUE}[DRY-RUN] Simulating test suite (check.py){NC}")
            return True

        checker = release_validation()

        # run_all() streams output directly to the terminal
        success = checker.run_all()

        if not success:
            print(f"\n{RED}❌ Quality validation failed.{NC}")
            print(f"{RED}Fix the errors before attempting a new release.{NC}")
            return False

        print(f"\n{GREEN}✅ Quality validation passed. Moving to versioning...{NC}")
        return True

    def run(self) -> bool:
        """Execute the release workflow."""
        self._print_banner()

        outcome = self._resolve_version()
        if outcome is not _Resolution.READY:
            return outcome is _Resolution.EARLY_OK

        if not self._apply_changes():
            return False

        return self._finalize()

    def _print_banner(self) -> None:
        """Print the run-mode banner."""
        if self.mode is Mode.PREVIEW:
            banner = f" {BLUE}[PREVIEW MODE — read-only]{NC}"
        elif self.mode is Mode.DRY_RUN:
            banner = f" {BLUE}[DRY-RUN MODE]{NC}"
        else:
            banner = ""
        print(f"\n{BLUE}🚀 Release Manager{banner}{NC}\n")

    def _resolve_version(self) -> _Resolution:
        """Run tests, fetch + analyze commits, compute or force ``new_version``,
        show analysis, and confirm with the user. Returns the resolution that
        ``run()`` should act on (see :class:`_Resolution`)."""
        if self.mode is not Mode.PREVIEW and not self.on_fresh_main():
            return _Resolution.EARLY_FAIL
        if self.mode is not Mode.PREVIEW and not self.head_runs_are_green():
            return _Resolution.EARLY_FAIL
        if self.mode is not Mode.PREVIEW and not self.run_tests():
            return _Resolution.EARLY_FAIL

        latest_tag = self.get_latest_tag()
        commits = self.get_commits_since_tag(latest_tag)

        if not commits:
            print(f"{YELLOW}No commits since last tag. Nothing to release.{NC}")
            # Under --preview, "no work pending" is a successful analysis,
            # not a failure. Returning EARLY_FAIL here would exit 1 and
            # break any CI step doing `release.py --preview` to detect
            # pending releases. Exit 0 with the current version as the
            # answer.
            if self.mode is Mode.PREVIEW:
                print(
                    f"{GREEN}✨ Preview: no release pending — current version is v{self.current_version}.{NC}\n"
                )
                return _Resolution.EARLY_OK
            return _Resolution.EARLY_FAIL

        self.analyze_commits(commits)

        if self.forced_version:
            self.new_version = self.forced_version
            print(
                f"{YELLOW}⚠ Version forced to {self.new_version} via --version flag.{NC}"
            )
        else:
            self.new_version = self.calculate_new_version(self.current_version)
            if self.new_version == self.current_version:
                if self.yes and self.mode is not Mode.PREVIEW:
                    print(
                        f"{RED}❌ No releasable commits (no feat/fix/breaking change since last tag).{NC}"
                    )
                    print(
                        f"{BLUE}   Use --version X.Y.Z to force a version, or remove --yes to confirm interactively.{NC}"
                    )
                    return _Resolution.EARLY_FAIL
                print(
                    f"{YELLOW}⚠ No releasable commits found (no feat/fix/breaking change since last tag).{NC}"
                )
                if self.mode is not Mode.PREVIEW:
                    print(
                        f'{BLUE}  Use "v" at the confirmation prompt or --version X.Y.Z to override, or cancel.{NC}'
                    )

        self.show_analysis()

        if self.mode is Mode.PREVIEW:
            print(f"\n{GREEN}✨ Preview: next release would be v{self.new_version}{NC}")
            print(f"{BLUE}Run without --preview to actually release.{NC}\n")
            return _Resolution.EARLY_OK

        if self.mode is Mode.DRY_RUN and self.yes:
            print(
                f"{BLUE}Note: --yes is redundant with --dry-run (no changes are made regardless).{NC}"
            )

        if self.yes:
            print(f"{BLUE}--yes flag set: auto-confirming v{self.new_version}{NC}")
        elif not self.ask_confirmation():
            print(f"{BLUE}Release cancelled.{NC}")
            return _Resolution.EARLY_FAIL

        return _Resolution.READY

    def _apply_changes(self) -> bool:
        """Update version files + changelog, run formatters, create the release commit."""
        if self.mode is not Mode.DRY_RUN and self._git("rev-parse", "--verify", "--quiet", f"refs/heads/{self.release_branch()}").returncode == 0:
            print(f"{RED}❌ {self.release_branch()} is left from an earlier attempt; nothing was changed.{NC}")
            print(f"{BLUE}   {self.recovery()}{NC}")
            return False
        self.update_version_files()
        self.update_changelog()

        if not self.format_files():
            print(f"\n{RED}❌ Formatting failed. Release cancelled.{NC}\n")
            return False

        return self.commit_release()

    def _finalize(self) -> bool:
        """Push (real mode only) and print the outcome message."""
        if self.mode is Mode.DRY_RUN:
            print(
                f"\n{GREEN}✨ Dry-run completed! Release would be v{self.new_version}{NC}"
            )
            print(f"Run without {BLUE}--dry-run{NC} to apply changes\n")
            return True

        if not self.push_release():
            return False
        print(f"\n{GREEN}✨ Release v{self.new_version} published!{NC}\n")
        return True

    def push_release(self) -> bool:
        """Land the release commit through a pull request, then tag what landed.

        main accepts only a commit whose checks passed: the branch is pushed, its pull
        request waits for every required check (`scripts/watch-pr.py`), GitHub merges it,
        and the tag — which starts the release workflow — goes on the commit main holds.
        """
        branch = self.release_branch()
        tag = f"v{self.new_version}"
        print(f"{BLUE}Opening the release pull request...{NC}")

        def run(*command: str) -> subprocess.CompletedProcess[str]:
            return subprocess.run(command, cwd=self.repo_root, capture_output=True, text=True, check=False)

        def stop(message: str, *hints: str) -> bool:
            print(f"{RED}❌ {message}{NC}", file=sys.stderr)
            for hint in hints:
                print(f"{BLUE}   {hint}{NC}", file=sys.stderr)
            return False

        current = run("git", "rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
        if current != branch:
            return stop(f'Current branch is "{current}", not "{branch}".')
        commit = run("git", "rev-parse", "HEAD").stdout.strip()

        # --no-verify intentionally skips the pre-push hook here.
        # Waiver: run_tests() already executed the full quality suite (check.py,
        # tests, build) earlier in this release flow and would have aborted on
        # any failure. Re-running it in the hook would be redundant and slow.
        pushed = run("git", "push", "--quiet", "--no-verify", "-u", "origin", branch)
        if pushed.returncode != 0:
            return stop(f"Could not push {branch}.", pushed.stderr.strip(), self.recovery())
        opened = run(
            "gh", "pr", "create", "--base", MAIN_BRANCH, "--head", branch,
            "--title", f"chore: release {tag}", "--body", f"The version and the changelog of {tag}.",
        )
        if opened.returncode != 0:
            return stop("Could not open the release pull request.", opened.stderr.strip(), self.recovery())
        number = opened.stdout.strip().rstrip("/").rsplit("/", 1)[-1]
        print(f"  ✓ Pull request #{number} opened — waiting for its checks")

        undo = self.recovery(number)
        watched = subprocess.run(
            [sys.executable, str(self.repo_root / "scripts" / "watch-pr.py"), number, "--timeout", "3600"],
            cwd=self.repo_root, check=False,
        )
        if watched.returncode != 0:
            return stop(f"The watch of pull request #{number} did not end green (exit {watched.returncode}).", undo)

        merged = run("gh", "pr", "merge", number, "--rebase", "--match-head-commit", commit)
        if merged.returncode != 0:
            return stop(f"GitHub refused to merge pull request #{number}.", merged.stderr.strip(), undo)

        # GitHub rebased the commit: the tag goes on the one main holds, once it is
        # known to be the release commit (same tree, same title).
        synced = run("git", "switch", "--quiet", MAIN_BRANCH).returncode == 0 and (
            run("git", "pull", "--quiet", "--ff-only", "origin", MAIN_BRANCH).returncode == 0
        )
        landed = run("git", "log", "-1", "--format=%s", MAIN_BRANCH).stdout.strip()
        same_tree = run("git", "diff", "--quiet", commit, MAIN_BRANCH).returncode == 0
        if not synced or landed != f"chore: release {tag}" or not same_tree:
            return stop(f"Pull request #{number} is merged, but {MAIN_BRANCH}'s head is not the release commit.", undo)
        tagged = run("git", "tag", "-a", tag, "-m", f"Version {self.new_version}", MAIN_BRANCH)
        if tagged.returncode != 0:
            return stop(f"Could not tag {MAIN_BRANCH} as {tag}.", tagged.stderr.strip(), undo)
        pushed = run("git", "push", "--quiet", "--no-verify", "origin", f"refs/tags/{tag}")
        if pushed.returncode != 0:
            return stop(
                f"Could not push the tag {tag}.", pushed.stderr.strip(),
                f"Retry: git push origin refs/tags/{tag}",
            )
        run("git", "branch", "-D", branch)
        print(f"  ✓ {tag} tagged on {MAIN_BRANCH} and pushed → GitHub Action triggered")
        return True


def _semver_str(value: str) -> str:
    """argparse type for --version. Exits 2 via ArgumentTypeError on mismatch."""
    if not re.match(r"^\d+\.\d+\.\d+$", value):
        raise argparse.ArgumentTypeError(f"expected semver X.Y.Z, got '{value}'")
    return value


if __name__ == "__main__":
    import usage_log

    usage_log.start()
    parser = argparse.ArgumentParser(description="Release manager.")
    # --dry-run and --preview are mutually exclusive: they're two distinct
    # modes (DRY_RUN edits files locally but skips push; PREVIEW is fully
    # read-only). Argparse enforces "at most one" so users get a clear
    # error rather than confusing combined behavior.
    mode_group = parser.add_mutually_exclusive_group()
    mode_group.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate release: update files, create local commit + tag, but skip the push. For a fully read-only check use --preview instead.",
    )
    mode_group.add_argument(
        "--preview",
        action="store_true",
        help="Read-only: print the suggested version and exit. Skips tests, file edits, commits, and pushes.",
    )
    parser.add_argument(
        "--version",
        metavar="X.Y.Z",
        type=_semver_str,
        help="Force a specific version (e.g. 0.12.1)",
    )
    parser.add_argument(
        "-y",
        "--yes",
        action="store_true",
        help="Skip confirmation prompt (auto-confirm suggested version)",
    )
    args = parser.parse_args()

    if args.preview:
        mode = Mode.PREVIEW
    elif args.dry_run:
        mode = Mode.DRY_RUN
    else:
        mode = Mode.REAL

    manager = ReleaseManager(
        mode=mode,
        forced_version=args.version,
        yes=args.yes,
    )
    success = manager.run()
    sys.exit(0 if success else 1)
