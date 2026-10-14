# 2026-10-10 — DEBT-087 — The CI reviewers run out of turns on a wide pull request

- Found by: three reviewer lanes failing without a finding on pull requests 149 and 150 (runs 37993079482, 37995910120, 37997330569)
- Where: `.github/workflows/review.yml` — `--max-turns`, and the commands the reviewers try that their grant refuses; the reviewer prompts under `.claude/agents/`
- Severity: 🟡
- Observation: on a diff of many files a reviewer reached its 40 turns before or just after writing its report, and the job failed: once with no report (backend), twice with a clean one (infrastructure, contract). 7 to 12 turns of each run went into commands the grant refuses, which the reviewer then retried another way; one reviewer started helper sessions and ended while waiting for them. The limit is raised to 80, which hides the waste without removing it
- Seen again on 2026-10-14 (pull request 256, run 38607430557): reviewer-backend found nothing and failed all the same, its write of the report under `.review/` refused by its grant; it gave the report in its reply, which the job does not read.
- User value: None directly — a pull request is not held back by a review that found nothing.
- Done when: a run's refused commands are listed and each is either granted by name or removed from the prompts; a reviewer cannot start helper sessions in CI; a reviewer that writes its report within the limit never fails its job; the limit is set from the measured turns of a wide pull request, with a test of the workflow that holds it.
