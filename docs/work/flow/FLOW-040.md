# FLOW-040 — Skills followed without being loaded, a recipe passed over, a contract amended by hand

- Kind: quality
- Size: S — one pull request: rule lines of the workflow and of two skills.
- Observed: in the 0.10.0 batch the steps of `/visual-proof` were followed four times
  from the text `/design-proposal` points to, and the skill was never loaded with the
  `Skill` tool, where `docs/workflow.md` § 3 says a skill is never followed from memory.
  Two contracts were amended by hand — one row, one type — and passed to
  `contract-reviewer`, where `CLAUDE.md` names `/contract`; the 0.9.0 batch did the same
  once. The script tests were run about twelve times with `python3 -m unittest`, where
  `just test-scripts` exists and the usage log never saw it; the 0.9.0 batch ran the
  native command three times. And `/run-entry` asks for one task per step, which a batch
  does not create, since `/run-queue` keeps one task per entry.
- Proposal: say what is done, or do what is said, one by one. `/design-proposal` and
  `/run-entry` load `/visual-proof` by name where they send to its steps. The workflow
  says a contract amended by one row or one type may be written by hand and goes to
  `contract-reviewer`, `/contract` being for a new command or a new contract — or the
  skill is loaded each time. `just test-scripts` takes a path, so that one file of the
  script tests runs through the recipe. `/run-entry` says its per-step tasks are for a
  run outside a batch.
- Costs: about an hour. Protects: the usage figures of an audit meaning what they say,
  and a rule that is passed over for three batches being either kept or changed.
- Accepted by the owner on 2026-10-12, when the audit was handed over: ready to queue.
