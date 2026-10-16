# FLOW-041 — A spec rewritten after its local review goes to CI unreviewed

- Kind: speed
- Observed: in the 0.11.0 batch the CI spec lane found a 🔴 twice (#253, #260), each on
  a sentence written in answer to the local `spec-reviewer`, which was not run again on
  it: two rounds of four caused by a fix. `/run-entry` Step 6 says to re-run the
  reviewers until no 🔴 remains; it was read as "until the first run has none".
- Proposal: `/run-entry` Step 6 says that a file edited in answer to a review goes back
  to its lane before the push — the spec lane above all, whose text a fix rewrites.
- Costs: about a minute per edited spec. Protects: a CI round per rewrite, and a 🔴 that
  reaches the pull request.
