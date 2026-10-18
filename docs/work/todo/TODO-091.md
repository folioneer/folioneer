# TODO-091 — (fullstack) — Each account carries its tax wrapper, from a list per country

From gh282 (owner, 2026-10-17), which this entry closes when it ships. An account is a name, a bank and a currency; the wrapper a user thinks in (PEA, assurance vie, ISA, Depot) lives in the name they typed. A wrapper is a label: no tax is computed, no rule enforced. The informative deposit ceilings of gh312 come later, on top of it.

**User value:** A user sees their accounts by the wrappers they actually hold, in their own country's words, instead of encoding them in account names.
**Done when:** An account can carry a wrapper from a generic list (brokerage, savings, pension, life insurance, crypto wallet, other) or from its country's list; existing accounts have none and nothing changes for them; the lists exist for the generic case, France (PEA, PEA-PME, assurance vie, PER) and Germany (Depot, Riester/Rürup, bAV), in English, French and German, and a further country is a data change; the accounts list can group or filter by wrapper, and the allocation view once TODO-026 ships; an agent reads and sets it (TODO-088 takes it on creation).
**Size:** L — a mock-up, the wrappers per country with France fixed until TODO-092, the agent's read and write, the accounts list by wrapper.
**Design:** none
**Open questions:**

- [x] Country on the portfolio, or on each account? — The portfolio's country of residence, set in the first run (TODO-092), overridable per account. (owner, 2026-10-17)
- [x] A label, or informative limits too? — A label only for the MVP; ceilings in a phase-2 issue (gh312). (owner, 2026-10-17)
- [x] The portfolio's country before TODO-092 asks it? — France, fixed in the code as EUR is today; each account can still choose its country; TODO-092 makes it a choice. (owner, 2026-10-18)
