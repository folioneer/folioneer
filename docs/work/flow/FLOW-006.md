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
