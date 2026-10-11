# FLOW-003 — Every merge sends the other open pull requests round again

- Kind: speed
- Observed: in 0.5.0 about 10 of the rounds re-tested a pull request whose own files had
  not changed, because another had merged first. In 0.6.0, 24 rounds of 61 came after the
  first (39 %; was 18 of 46, 39 %), and what caused them was not counted. `just merge`
  already waives the round when the rebase changes only record files.
  In 0.7.0, 24 rounds of 48 (50 %), the causes not counted again: FLOW-020 was not queued.
  Two of them are known, #153 and #155 on the release day, each sent round by the other's
  merge.
  In 0.7.1, 1 round of 18 (6 %): the entries were stacked (`docs/workflow.md` § 11) and
  no pull request was sent round by another's merge.
- Verdict: **keep, and count the causes (FLOW-020).** A round costs about 5 min where it
  cost 25, and the median pull request merges in 7 min. A merge queue would cost a
  rewrite of `just merge` for a saving that is small. The share rose on the reviewers'
  failures, not on the merges.
  In 0.9.0, 4 rounds of 22 (18 %; 0.8.0: 11 of 30, 37 %): one counted, a fix after a
  review; none known to be a pull request sent round by another's merge — the entries
  were stacked again. The verdict stands.
