#!/usr/bin/env python3
"""Auto-rebase + fast-forward merge the current feature branch into a target branch.

Atomic "task done" shortcut: pull the target, rebase the branch onto it,
FF-merge into the target, push, and delete the feature branch both locally
and on origin. Fails fast with a clear recovery hint at any step that can't
proceed cleanly (rebase conflict, divergent push, dirty tree, etc.).

The merge guard: the branch must be the head of an open pull request whose
every check run is green, and every check named in `required-checks.json`
must be among them. When the rebase moved the commits (the target advanced),
the rebased branch is pushed and the merge stops until CI has run on it, unless
the rebase changed record files only (todo, techdebt, lessons, plans, ADRs) or
nothing at all.

One entry lands as one commit: a fix pushed after the first push is a
`fixup!` commit (`git commit --fixup <sha>`), and the rebase folds it into the
commit it names. Folding rewrites commits and leaves the tree as CI tested it,
so the checks stand.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import NoReturn

if os.environ.get("NO_COLOR"):
    RED = GREEN = BLUE = NC = ""
else:
    RED = "\033[0;31m"
    GREEN = "\033[0;32m"
    BLUE = "\033[0;34m"
    NC = "\033[0m"


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], capture_output=True, text=True, check=check)


def fail(msg: str, *recovery_lines: str) -> NoReturn:
    print(f"{RED}❌ {msg}{NC}", file=sys.stderr)
    for line in recovery_lines:
        print(f"{BLUE}   {line}{NC}", file=sys.stderr)
    sys.exit(1)


def _git_dir() -> Path:
    return Path(git("rev-parse", "--git-dir").stdout.strip())


def _rebase_in_progress() -> bool:
    """True if a rebase was started but not yet finished or aborted."""
    gdir = _git_dir()
    return (gdir / "rebase-merge").is_dir() or (gdir / "rebase-apply").is_dir()


def gh(*args: str) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            ["gh", *args], capture_output=True, text=True, check=False, timeout=60
        )
    except FileNotFoundError:
        fail("gh is not installed — the merge guard cannot read the checks.")
    except subprocess.TimeoutExpired:
        fail(f"gh {args[0]} {args[1]} did not answer within 60 s — is GitHub reachable?")


PASSING = {"success", "skipped", "neutral"}


def _required_checks() -> list[str]:
    path = Path(git("rev-parse", "--show-toplevel").stdout.strip()) / "required-checks.json"
    try:
        return list(json.loads(path.read_text(encoding="utf-8"))["checks"])
    except (OSError, ValueError, KeyError) as exc:
        fail(f"Could not read {path.name}: {exc}")


def _open_pull_request(branch: str, target: str) -> tuple[int, str]:
    """Number and head sha of the open pull request from `branch` into `target`."""
    result = gh(
        "pr", "list", "--head", branch, "--base", target, "--state", "open",
        "--json", "number,headRefOid", "--limit", "1",
    )
    if result.returncode != 0:
        fail("Could not list pull requests.", (result.stderr or "").strip())
    prs = json.loads(result.stdout or "[]")
    if not prs:
        fail(
            f"No open pull request from {branch} into {target}.",
            "Every change goes through the harness — open one and let it run:",
            f"  git push -u origin {branch} && gh pr create --fill",
        )
    return int(prs[0]["number"]), str(prs[0]["headRefOid"])


def _check_runs(sha: str) -> dict[str, tuple[str, str]]:
    """Latest check run per name on `sha`: name -> (status, conclusion).

    A re-run creates a new run with the same name and a higher id; the
    highest id wins whatever order the API lists them in.
    """
    result = gh(
        "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/check-runs?filter=latest",
        "--paginate",
        "--jq", '.check_runs[] | [.id, .name, .status, (.conclusion // "")] | @tsv',
    )
    if result.returncode != 0:
        fail(f"Could not read the check runs of {sha[:7]}.", (result.stderr or "").strip())
    latest: dict[str, tuple[int, str, str]] = {}
    for line in result.stdout.splitlines():
        run_id, name, status, conclusion = (line.split("\t") + ["", "", ""])[:4]
        if name not in latest or int(run_id) > latest[name][0]:
            latest[name] = (int(run_id), status, conclusion)
    return {name: (status, conclusion) for name, (_, status, conclusion) in latest.items()}


# The record files: what closure commits and queue edits move. None is read by
# a check or a reviewer, so a move of `main` made only of them cannot change what
# the checks proved. Everything else — code, workflows, convention docs, prompts —
# re-runs the checks.
RECORD_FILES = {"docs/todo.md", "docs/techdebt.md", "docs/lessons.md"}
RECORD_DIRS = ("docs/plan/", "docs/adr/")


def _record_files_only(files: list[str]) -> bool:
    return all(f in RECORD_FILES or (f.endswith(".md") and f.startswith(RECORD_DIRS)) for f in files)


def _rebase_left_the_checks_standing(before: str, after: str) -> bool:
    """True when the tree the checks ran on and the tree about to land are identical, or differ by record files only.

    Comparing the two trees, not the target's history, means a branch stacked
    on commits that have since merged, or one whose `fixup!` commits were just
    folded, is not sent for a re-run: what CI tested is what lands.
    """
    diff = git("diff", "--name-only", before, after, check=False)
    if diff.returncode != 0:
        fail(f"Could not compare {before[:7]} with {after[:7]}.", (diff.stderr or "").strip())
    return _record_files_only([f for f in diff.stdout.splitlines() if f])


FOLD_PREFIXES = ("fixup! ", "squash! ", "amend! ")


def _fold_commits(target: str, branch: str) -> list[str]:
    """Titles of the commits of `branch` not in `target` that ask to be folded into another."""
    log = git("log", "--format=%s", f"{target}..{branch}", check=False)
    if log.returncode != 0:
        fail(f"Could not list the commits of {branch}.", (log.stderr or "").strip())
    return [title for title in log.stdout.splitlines() if title.startswith(FOLD_PREFIXES)]


def refuse_unreadable_folds(target: str, branch: str) -> None:
    """Stop before the rebase on a `squash!` or `amend!` commit: it would land a message no check has read."""
    unreadable = [t for t in _fold_commits(target, branch) if not t.startswith("fixup! ")]
    if unreadable:
        fail(
            f"`{branch}` carries squash!/amend! commits; only fixup! commits are folded.",
            *(f"  {title}" for title in unreadable),
            "Drop or reword each one (git rebase), carry its change as `git commit --fixup <sha>` instead,",
            "push, let CI run, then re-run.",
        )


def refuse_unfolded_fixups(target: str, branch: str, before: str) -> None:
    """Stop after the rebase on a `fixup!` commit that named no commit of the branch.

    It was not folded, and must not reach the target: its title would become a
    changelog line. The branch goes back to `before`, its head before the rebase.
    """
    unfolded = _fold_commits(target, branch)
    if unfolded:
        # The rebase has just completed, so the tree is clean: this only moves the branch back.
        restored = git("reset", "--hard", before, check=False).returncode == 0
        fail(
            f"`{branch}` carries fixup! commits that name no commit of the branch.",
            *(f"  {title}" for title in unfolded),
            "Branch was restored to its original state (no rewrite)."
            if restored
            else f"Branch is rebased; its original head was {before[:7]}.",
            "Give each the exact title of the commit it fixes (`git commit --fixup <sha>` does), then re-run.",
        )


def rebase_folding_fixups(target: str) -> subprocess.CompletedProcess[str]:
    """Rebase the current branch onto `target`, folding each `fixup!` commit into the commit it names."""
    return subprocess.run(
        ["git", "rebase", "--interactive", "--autosquash", target],
        capture_output=True,
        text=True,
        check=False,
        # The todo list git proposes is taken as it is: nobody edits it.
        env={**os.environ, "GIT_SEQUENCE_EDITOR": "true"},
    )


def ensure_checks_green(branch: str, target: str, before: str, after: str) -> None:
    """Refuse the merge unless CI is green on exactly the commits about to land."""
    number, head = _open_pull_request(branch, target)
    if after != head:
        push = git("push", "--force-with-lease", "origin", branch, check=False)
        if push.returncode != 0:
            fail(
                f"Local {branch} ({after[:7]}) is not the head of PR #{number} ({head[:7]}) and the push failed.",
                (push.stderr or "").strip(),
            )
        # The checks are then read on `head`, which the push above left without
        # a ref: GitHub keeps a commit's check runs regardless (verified: a
        # pre-rebase head answered with its 14 runs after the force push).
        if before == head and _rebase_left_the_checks_standing(before, after):
            print(
                f"{BLUE}ℹ The rebase left the tested tree as it was, record files aside; the checks of {head[:7]} stand.{NC}",
                file=sys.stderr,
            )
        else:
            why = "the rebase moved the commits" if before == head else "the local branch was ahead of the pull request"
            fail(
                f"{branch} pushed as {after[:7]} — {why}.",
                f"CI runs on it now: gh pr checks {number} --watch",
                "Re-run `just merge` when every check is green.",
            )
    runs = _check_runs(head)
    missing = [name for name in _required_checks() if name not in runs]
    not_green = sorted(
        f"{name}: {status if status != 'completed' else conclusion}"
        for name, (status, conclusion) in runs.items()
        if status != "completed" or conclusion not in PASSING
    )
    if missing or not_green:
        fail(
            f"PR #{number} is not green on {head[:7]}.",
            *(f"  missing: {name}" for name in missing),
            *(f"  {line}" for line in not_green),
            f"Watch: gh pr checks {number} --watch — then re-run `just merge`.",
        )
    landing = "" if after == head else f", merging {after[:7]}"
    print(f"{GREEN}✓ PR #{number}: every check green on {head[:7]}{landing}.{NC}", file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "-t",
        "--target",
        default="main",
        help="Target branch to merge into (default: main).",
    )
    args = parser.parse_args()
    target = args.target

    branch = git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()

    # Pre-flight 1 — not already on the target.
    if branch == target:
        fail(
            f"Already on {target} — nothing to merge.",
            f"Run from a feature branch (not {target}).",
        )

    # Pre-flight 2 — clean working tree (both staged and unstaged).
    if (
        git("diff", "--quiet", check=False).returncode != 0
        or git("diff", "--cached", "--quiet", check=False).returncode != 0
    ):
        fail(
            "Working tree has uncommitted changes.",
            "Commit or stash them, then re-run.",
        )

    # Pre-flight 3 — no rebase in progress (from a previous interrupted run).
    if _rebase_in_progress():
        fail(
            "A rebase is already in progress in this repo.",
            "Finish or abandon it first:",
            "  git rebase --continue   # after resolving conflicts",
            "  git rebase --abort      # to abandon",
            "Then re-run.",
        )

    # Pre-flight 4 — target branch exists locally.
    if git("rev-parse", "--verify", "--quiet", target, check=False).returncode != 0:
        fail(
            f"Target branch {target} does not exist locally.",
            f"Create or fetch it: git fetch origin {target}:{target}",
        )

    # Pre-flight 5 — feature branch's origin is not ahead of local.
    # merge.py deletes `origin/<branch>` at Step 5; if origin has commits we
    # don't have locally (someone else force-pushed, or we never pulled), those
    # commits vanish silently. Refuse and surface a clear recovery path. The
    # check is skipped when origin/<branch> doesn't exist (branch never
    # pushed) — there's nothing to lose in that case.
    has_origin_branch = (
        git(
            "rev-parse", "--verify", "--quiet", f"origin/{branch}", check=False
        ).returncode
        == 0
    )
    if has_origin_branch:
        # Fetch first so the ahead/behind count reflects the actual remote
        # state, not the cached refs from the last fetch. Quiet on success.
        fetch = git("fetch", "--quiet", "origin", branch, check=False)
        if fetch.returncode != 0:
            fail(
                f"Could not fetch origin/{branch}.",
                "Origin is unreachable or the branch was deleted server-side.",
                f"Investigate: git fetch origin {branch}",
            )
        # Count commits on origin that are NOT in local. Non-zero means
        # origin has unique commits we would discard. Fail closed if the
        # count itself fails — silently passing here defeats Pre-flight 5's
        # entire purpose (preventing silent data loss at Step 5).
        ahead = git("rev-list", "--count", f"{branch}..origin/{branch}", check=False)
        if ahead.returncode != 0:
            fail(
                f"Could not compare {branch} with origin/{branch}.",
                f"Investigate: git rev-list --count {branch}..origin/{branch}",
            )
        count = ahead.stdout.strip()
        if count != "0":
            fail(
                f"origin/{branch} has {count} commit(s) not in local {branch}.",
                f"merge would delete origin/{branch} at Step 5 and lose those commits.",
                f"  git pull --ff-only origin {branch}    # incorporate them",
                f"  git pull --rebase origin {branch}     # if local has also diverged",
                f"  git push --force-with-lease origin {branch}    # discard them (intentional)",
                "Then re-run merge.",
            )

    # Step 1 — sync local target with origin (skip if no GitHub remote).
    has_origin_target = (
        git(
            "rev-parse", "--verify", "--quiet", f"origin/{target}", check=False
        ).returncode
        == 0
    )
    result = git("checkout", target, check=False)
    if result.returncode != 0:
        detail = result.stderr.strip() or "(no stderr captured)"
        fail(
            f"Could not checkout `{target}`.",
            f"detail: {detail}",
            "Common causes: untracked files conflict, uncommitted changes. Inspect: git status",
        )
    if has_origin_target:
        result = git("pull", "--ff-only", "--quiet", "origin", target, check=False)
        if result.returncode != 0:
            fail(
                f"Could not fast-forward pull origin/{target}.",
                f"Local {target} has diverged from origin/{target}, or origin is unreachable.",
                f"Investigate: git fetch origin {target} && git log {target}..origin/{target}",
            )
    else:
        print(
            f"{BLUE}ℹ No origin/{target} remote — skipping pull.{NC}",
            file=sys.stderr,
        )

    # Step 2 — rebase the feature branch onto the (now-current) target.
    # On conflict, abort the rebase to restore the branch to its pre-rebase
    # state — `just merge` must stay "soft": never leave the user's branch in
    # a half-rewritten state. Conflict resolution is the user's job; we just
    # report cleanly and let them rebase manually.
    result = git("checkout", branch, check=False)
    if result.returncode != 0:
        detail = result.stderr.strip() or "(no stderr captured)"
        fail(
            f"Could not checkout `{branch}`.",
            f"detail: {detail}",
            "Inspect: git status",
        )
    before = git("rev-parse", branch).stdout.strip()
    refuse_unreadable_folds(target, branch)
    result = rebase_folding_fixups(target)
    if result.returncode != 0:
        abort_result = git("rebase", "--abort", check=False)
        if abort_result.returncode != 0:
            # Abort itself failed — branch is in a mid-rebase state, we can't
            # promise restoration. Override the standard recovery message.
            abort_detail = abort_result.stderr.strip() or "(no stderr captured)"
            fail(
                f"Cannot merge `{branch}` into `{target}`: rebase has conflicts AND `git rebase --abort` failed.",
                f"abort stderr: {abort_detail}",
                "Branch is mid-rebase. Inspect `git status`, then `git rebase --abort` or `--continue` manually.",
            )
        else:
            fail(
                f"Cannot merge `{branch}` into `{target}`: rebase has conflicts.",
                "Branch was restored to its original state (no rewrite).",
                "Resolve manually and re-run:",
                f"  git rebase {target}    # walk through the conflicts",
                "  # ...fix conflicting files, then git add + git rebase --continue",
                "  just merge              # finishes the merge",
            )

    refuse_unfolded_fixups(target, branch, before)

    # Step 2b — the merge guard: green checks on exactly these commits.
    ensure_checks_green(branch, target, before, git("rev-parse", branch).stdout.strip())

    # Step 3 — fast-forward merge target onto the rebased branch.
    # After step 2 this is guaranteed to FF; we still pass --ff-only for safety.
    result = git("checkout", target, check=False)
    if result.returncode != 0:
        detail = result.stderr.strip() or "(no stderr captured)"
        fail(
            f"Could not checkout `{target}` after rebase.",
            f"detail: {detail}",
            f"Rebase of `{branch}` onto `{target}` succeeded locally — only the checkout failed.",
            f"Manually: git checkout {target} && git merge --ff-only {branch}",
        )
    result = git("merge", "--ff-only", branch, check=False)
    if result.returncode != 0:
        # Defensive — should be unreachable after a successful rebase.
        fail(
            f"Unexpected: FF-merge of `{branch}` into `{target}` failed after rebase.",
            f"This should be unreachable after a successful rebase — `{target}` likely moved.",
            f"Inspect: git log {target}..{branch} && git status",
            "If target moved, re-run `just merge` — the second pass catches the new state.",
        )

    # Step 4 — push target to origin (skip if no GitHub remote).
    if has_origin_target:
        result = git("push", "origin", target, check=False)
        if result.returncode != 0:
            fail(
                f"Could not push {target} to origin/{target}.",
                f"Local `{target}` already contains the FF-merged feature branch — only the push failed.",
                "Origin probably moved while merging. Reconcile and retry:",
                f"  git fetch origin {target}",
                f"  git rebase origin/{target}",
                f"  git push origin {target}",
            )

    # Step 5 — delete the remote feature branch (if it exists).
    # Restores the "atomic shortcut" property: one command cleans up both
    # local and remote feature branches. Also removes the upstream reference
    # so Step 6's `git branch -d` falls back to the "merged-in-HEAD" check
    # (which passes after Step 3's FF-merge) instead of the stricter
    # "merged-in-upstream" check that would refuse if the feature branch was
    # ahead of its origin counterpart.
    #
    # Failure modes we distinguish:
    #   - Remote ref doesn't exist (race: someone else deleted it). Benign,
    #     fall through to local cleanup as if it had never been there.
    #   - Protected branch / pre-receive hook declined. The user needs to
    #     know — partial state on the remote, fix-forward required.
    #   - Network / auth failure. Same surface as the protected case.
    remote_state = "skipped"  # one of: skipped, deleted, gone, failed
    if has_origin_target:
        has_origin_branch = (
            git(
                "rev-parse", "--verify", "--quiet", f"origin/{branch}", check=False
            ).returncode
            == 0
        )
        if has_origin_branch:
            result = git("push", "--delete", "origin", branch, check=False)
            if result.returncode == 0:
                remote_state = "deleted"
            elif "remote ref does not exist" in (result.stderr or "").lower():
                remote_state = "gone"  # race — already removed elsewhere
            else:
                remote_state = "failed"
                stderr_excerpt = (result.stderr or "").strip().splitlines()
                detail = stderr_excerpt[-1] if stderr_excerpt else "no stderr"
                print(
                    f"{RED}❌ Failed to delete origin/{branch}: {detail}{NC}",
                    file=sys.stderr,
                )
                print(
                    f"{BLUE}   Retry manually once unblocked: "
                    f"git push --delete origin {branch}{NC}",
                    file=sys.stderr,
                )

    # Step 6 — delete the local branch.
    result = git("branch", "-d", branch, check=False)
    if result.returncode != 0:
        fail(
            f"Could not delete local branch {branch}.",
            "The branch is merged into target, but git refused the delete —",
            f"most likely a stale upstream config. Inspect: git branch -vv | grep {branch}",
            f"Confirm the merge: git log {target}..{branch}",
            f"Force-delete if confirmed: git branch -D {branch}",
        )

    # Compose an accurate success summary. Each segment reflects observed state,
    # not assumed state — important because Step 5's failure path leaves the
    # remote branch behind and the user must see that in the success line.
    parts = ["local"]
    if remote_state == "deleted":
        parts.append("remote")
    elif remote_state == "failed":
        parts.append("remote delete failed — see above")
    suffix = ", pushed" if has_origin_target else ""
    print(
        f"{GREEN}✅ {branch} rebased + merged into {target}{suffix}, "
        f"and deleted ({' + '.join(parts)}).{NC}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
