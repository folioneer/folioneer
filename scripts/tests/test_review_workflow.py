"""Tests on the review workflow's own shell (FLOW-002) — run with `just test-scripts`.

The step that posts a lane's report is shell inside YAML. Its usage-limit test is pulled
out of the file and run on the messages a stopped session really leaves.
"""

import re
import subprocess
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "review.yml"


def usage_limit_pattern() -> str:
    match = re.search(r'grep -qiE "([^"]+)" <<<"\$last"', WORKFLOW.read_text())
    assert match, "the usage-limit test is no longer in review.yml"
    return match.group(1)


def stopped_by_the_limit(message: str) -> bool:
    return (
        subprocess.run(
            ["grep", "-qiE", usage_limit_pattern()], input=message, text=True
        ).returncode
        == 0
    )


class UsageLimit(unittest.TestCase):
    # FLOW-002 — the message seen on pull request #91, and its siblings.
    def test_a_session_stopped_by_the_usage_limit_is_recognised(self):
        self.assertTrue(
            stopped_by_the_limit("You've hit your session limit · resets 9:10pm (UTC)")
        )
        self.assertTrue(
            stopped_by_the_limit("You've hit your weekly limit · resets Monday")
        )
        self.assertTrue(stopped_by_the_limit("Usage limit reached"))

    # A reviewer that ran and said something else is not excused.
    def test_another_last_message_is_not_a_usage_limit(self):
        self.assertFalse(
            stopped_by_the_limit("I could not find any changed file in my lane.")
        )
        self.assertFalse(
            stopped_by_the_limit("The diff adds a rate limit to the fetch.")
        )
        self.assertFalse(stopped_by_the_limit(""))


def granted_tools() -> list[str]:
    line = re.search(r"^\s*--allowedTools (.+)$", WORKFLOW.read_text(), re.M)
    assert line, "the reviewer step no longer grants its tools on one line"
    return re.findall(r'"([^"]+)"', line.group(1))


class NamedCommands(unittest.TestCase):
    """TD-082 — the reviewers run the commands granted by name, and no other."""

    REACHING_OUT = (
        "gh",
        "curl",
        "wget",
        "ssh",
        "scp",
        "nc",
        "git push",
        "git remote",
        "git fetch",
        "git pull",
        "git clone",
        "git ls-remote",
    )
    WRITING = ("rm", "mv", "cp", "chmod", "tee", "sed -i", "git commit", "git checkout", "git reset")

    def shell_grants(self) -> list[str]:
        return [tool[len("Bash(") : -1] for tool in granted_tools() if tool.startswith("Bash(")]

    def test_the_shell_is_never_granted_whole(self):
        tools = granted_tools()
        self.assertNotIn("Bash", tools)
        self.assertNotIn("Bash(*)", tools)
        self.assertTrue(self.shell_grants(), "the reviewers still need their helpers")

    def test_no_list_of_refused_commands_stands_in_for_the_grant(self):
        self.assertNotIn("--disallowedTools", WORKFLOW.read_text())

    def test_every_grant_names_one_command_before_its_wildcard(self):
        for grant in self.shell_grants():
            self.assertTrue(grant.endswith("*"), grant)
            command = grant[:-1]
            self.assertTrue(command.strip(), grant)
            self.assertNotIn("*", command, grant)
            self.assertFalse(command[0] in " *", grant)

    def test_nothing_granted_rewrites_the_checkout(self):
        for grant in self.shell_grants():
            for refused in self.WRITING:
                self.assertFalse(
                    grant == refused + " *" or grant.startswith(refused + " "),
                    f"{grant} grants {refused}",
                )

    def test_the_only_grant_that_reaches_out_reads_an_action_s_tags_on_github(self):
        reaching = [
            grant
            for grant in self.shell_grants()
            if any(
                grant == refused + " *" or grant.startswith(refused + " ")
                for refused in self.REACHING_OUT
            )
            or "://" in grant
        ]
        self.assertEqual(reaching, ["git ls-remote --tags https://github.com/*"])

    def test_the_helpers_and_the_reading_commands_of_git_are_granted(self):
        grants = self.shell_grants()
        for needed in (
            "bash scripts/branch.sh *",
            "bash scripts/review-path.sh *",
            "git diff *",
            "git log *",
            "git show *",
            "git cat-file *",
            "grep *",
            "shellcheck *",
        ):
            self.assertIn(needed, grants)

    def test_nothing_granted_runs_code_the_pull_request_brings(self):
        """An interpreter, a test runner or a build tool runs files of the checkout with
        everything the runner can do: once it starts, the list constrains nothing."""
        from_the_base_branch = {
            "bash scripts/branch.sh *",
            "bash scripts/review-path.sh *",
        }
        runners = ("bash", "sh", "python", "python3", "node", "npx", "npm", "cargo", "just", "make")
        for grant in self.shell_grants():
            if grant in from_the_base_branch:
                continue
            self.assertNotIn(grant.split()[0], runners, grant)

    def test_the_two_helpers_granted_are_the_ones_checked_out_from_the_base_branch(self):
        workflow = WORKFLOW.read_text()
        checked_out = re.search(r"git checkout \"origin/[^\n]*-- (.+)$", workflow, re.M)
        self.assertIsNotNone(checked_out)
        self.assertEqual(
            sorted(checked_out.group(1).split()),
            ["scripts/branch.sh", "scripts/review-path.sh"],
        )

    def test_the_web_tools_are_not_granted(self):
        for tool in granted_tools():
            self.assertFalse(tool.startswith("Web"), tool)


if __name__ == "__main__":
    unittest.main()
