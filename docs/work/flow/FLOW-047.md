# FLOW-047 — The private build waits on GitHub's timer after a release

- Kind: speed
- Observed: after v0.13.0 was published (2026-10-18, about 18:17 UTC), the private repository's `follow.yml` — scheduled every half hour, its only link to the public release — had not run an hour later; that day its timer ran 4 times of the 48 scheduled. The owner asked why and started it by hand; the private release followed in half an hour.
- Proposal: the public release workflow starts the private one itself once the release is published (a dispatch of `follow.yml`, or of the private release workflow), the timer kept as a fallback. Needs a credential of the public repository allowed to start a workflow in the private one — the owner's decision, set outside the repository's text.
- Costs: one workflow step and one credential to hold. Protects: the owner's private build ready minutes after a release, not hours; no manual dispatch to remember.
- Needs the owner: yes — the credential.
