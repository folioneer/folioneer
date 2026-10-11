# FLOW-006 — An entry whose Done-when is an audit cannot close

- Kind: speed
- Observed: DEBT-065 ("every figure listed with where it is computed") took four pull
  requests and is still open: the audit kept finding work. TODO-055 promised a refusal that
  an existing rule (AST-009) forbids, found only when the owner read the result.
- Proposal: the challenge step, already run before starting a TD or an issue, also (a)
  splits an entry whose Done-when is a survey into "list them" and one entry per thing
  found, and (b) reads the entry against the spec rules of its domain before coding.
- Costs: minutes per entry. Protects: a pull request written to a promise the rules
  refuse.
- 0.7.0: TODO-059 held seven pieces of work and was split into seven entries when the
  owner read it, after the batch; the challenge step had not split it.
- 0.9.0: TODO-059's last clause — a session only the owner could record — kept the entry
  open after its code had merged, until the owner said he is not the QA. The rule is now
  in `docs/workflow.md` § 1 and in `/run-entry`: such a clause becomes a test or a debt
  entry, and the entry closes. What is left of this proposal is its first half, the
  entry whose Done-when is a survey.
