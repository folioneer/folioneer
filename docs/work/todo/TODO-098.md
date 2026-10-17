# TODO-098 — (fullstack) — A quantity leaves a holding without proceeds: a fee in kind, dust, lost coins

From gh273 (owner, 2026-10-17), which this entry closes when it ships. Network fees paid in the asset, dust left by a conversion, coins sent to an untracked wallet or lost have no proceeds, and `record_sale` refuses a zero total. The workaround — a sale at 0.01 and a withdrawal — invents cash and gains in a history that matters for tax.

**User value:** Fees in kind, dust and lost coins are recorded as what they are, the cash stays the real one, and realised gains are not padded with symbolic amounts.
**Done when:** A kind "removal without proceeds", with a reason (fee, dust, lost, other) and a note, reduces a holding at its weighted-average cost and moves no cash; its realised result is the cost removed, negated (nothing for a zero-cost holding), "lost" shown apart; the window and the agent share its rule and its refusals (more than held, unknown asset); tests hold both cases.
**Design:** none
**Open questions:**

- [x] A zero total on `record_sale`, or a kind of its own? — A kind of its own, "removal without proceeds", with a reason. (owner, 2026-10-17)
- [x] "Lost" a reason of its own? — Yes, shown apart. (owner, 2026-10-17)
