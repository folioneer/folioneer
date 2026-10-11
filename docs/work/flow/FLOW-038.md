# FLOW-038 — A queue is written without knowing how much it holds

- Kind: speed + quality
- Size: S — one pull request: the queue script, the entry check, two skills.
- Observed: on 2026-10-11 the next queue was written with eleven entries and no figure
  for what they hold; asked whether each was sized, the agent had only an impression
  (small, medium, large). Counted afterwards by the rule of the owner's other repository,
  they come to 14 to 17 pull requests, nearly all code, where the last two batches merged
  17 and 19 of which about half were records only: the queue is closer to two batches
  than one. No entry of this repository carries a size (103 files), and neither
  `just queue` nor `/plan-queue` says what a queue adds up to.
- Proposal: take the size rule of the owner's other repository, under the same names
  (FLOW-034). An entry carries a `Size` line — **S**: one pull request, one layer,
  nothing from the owner; **M**: any other single pull request — several lanes, a spec or
  a contract, an answer from the owner; **L**: two or three pull requests, or mock-ups or
  a spec interview with the owner; **XL**: a programme, which cannot be queued before it
  is sliced. Sized by facts, not by effort. The agent sets it on a debt or a flow entry;
  on a todo entry `/plan-queue` proposes it and the owner confirms it. `just queue` sums
  the sizes of the queued entries in pull requests (S and M count 1, L counts 2 to 3) and
  `/plan-queue` shows that sum beside what the last batches merged
  (`docs/work/audits.md`), stopping its proposal there. `/flow-audit` reports what each
  entry took beside its size.
- In two steps (owner, 2026-10-11). First the rule, the sum and the comparison. The
  existing entries are not sized in one pass: an entry gets its size when it is proposed
  for the queue or enters it, so that the queue is not overloaded; `just rule-homes`
  requires a size on a queued entry only. That is a transition: once every entry carries
  a size, the check requires it of all, as in the other repository.
- The entries of the queue written on 2026-10-11 carry their size already, set while the queue was refined with the owner; this entry was first called M, by its pieces, and is S by the rule — one pull request, one lane, nothing from the owner.
- Costs: about two hours, with the tests of the sum and of the check; a line per entry
  when it is queued. Protects: a queue larger than a batch being found out halfway, and
  a release that waits on it.
