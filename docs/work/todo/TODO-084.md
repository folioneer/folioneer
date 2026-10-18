# TODO-084 — (backend) — A holding's return over a window it was not held through, and what an account started from opening balances was given

From gh245 (owner, 2026-10-12), which this entry closes when it ships. Two figures looked wrong on an account rebuilt from statements.

**A return below −100 %.** A holding showed about −19 800 % over five years and −39 600 % over ten. Read on 2026-10-13: the window's return is a Simple Dietz return (ACD-056), and a window that opens before the position existed starts from 0 (`window_position_pct`, "ACD-055 — a window opening before the position existed starts from 0"). The denominator is then the purchases weighted by the share of the window left after them: a position bought a few days before today has almost none of a five-year window left, so any gain or loss is divided by almost nothing — and by half as much again over ten years, which is the doubling seen. The old price made the loss large; the formula made it thousands of percent. It is not a wrong starting price, as the issue supposed: there is no starting price, the position was not held. ACD-057 lists when a window has no figure, and this case is not in it.

**Net cash input.** An account started with opening balances and a small deposit shows `total_net_cash_input` equal to the deposit. That is the rule as written: ACD-053 defines it as deposits less withdrawals, and an opening balance has no cash leg (PRF-086). The performance figures of the account do count an opening balance as money brought in (PRF-030, PRF-035). So nothing is computed against its rule; what is missing is a figure that says what the owner brought in, cash and positions together, for whoever reads `total_net_cash_input` as that — an agent did.

**User value:** The returns the owner reads, and those an agent repeats, are plausible: no figure for a window a holding was not held through, and a capital figure that counts what an account was opened with.
**Done when:** a window that opens before the position was first held shows what the owner decided below, with a test of a position bought days ago and priced low; the account says what was brought into it, opening balances included, as decided below, with a test of an account opened by balances; the rules (ACD-053, ACD-055 to ACD-057) say so.
**Size:** L — a mock-up of the capital brought in, then the rules and the backend, then the window.
**Design:** none
**Open questions:**

- [x] A window that opens before the holding was first held: no figure (the since-start return already covers the holding's whole life), or the return since the first purchase shown under that window's name? (Recommended: no figure.) — No figure. (owner, 2026-10-17)
- [x] What the owner brought in: `total_net_cash_input` keeps its literal meaning and a second figure counts opening balances at their cost, or the one figure changes its meaning? — A second figure: `total_net_cash_input` stays deposits less withdrawals, and a figure of the capital brought in adds the opening balances at their cost — an account opened with positions costing 50 000 and a deposit of 500 says 500 of cash and 50 500 brought in (owner, 2026-10-13).
- [x] Is the second figure shown in the window, or given to agents only? — Shown in the window and given to agents. (owner, 2026-10-17)
