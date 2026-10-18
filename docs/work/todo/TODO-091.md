# TODO-091 — (fullstack) — Each account carries its tax wrapper, from a list per country

From gh282 (owner, 2026-10-17), which this entry closes when it ships. An account is a name, a bank and a currency; the wrapper a user thinks in (PEA, assurance vie, ISA, Depot) lives in the name they typed. A wrapper is a label: no tax is computed, no rule enforced. The informative deposit ceilings of gh312 come later, on top of it.

**User value:** A user sees their accounts by the wrappers they actually hold, in their own country's words, instead of encoding them in account names.
**Done when:** An account can carry a wrapper from a generic list (brokerage, savings, pension, life insurance, crypto wallet, other) or from its country's list; existing accounts have none and nothing changes for them; the lists exist for the generic case, France (PEA, PEA-PME, assurance vie, PER) and Germany (Depot, Riester/Rürup, bAV), in English, French and German, and a further country is a data change; the accounts list can group or filter by wrapper, and the allocation view once TODO-026 ships; an agent reads and sets it (TODO-088 takes it on creation).
**Size:** L — a mock-up, the wrappers per country with France fixed until TODO-092, the agent's read and write, the accounts list by wrapper.
**Design:** validated
**Open questions:**

- [x] Country on the portfolio, or on each account? — The portfolio's country of residence, set in the first run (TODO-092), overridable per account. (owner, 2026-10-17)
- [x] A label, or informative limits too? — A label only for the MVP; ceilings in a phase-2 issue (gh312). (owner, 2026-10-17)
- [x] The portfolio's country before TODO-092 asks it? — France, fixed in the code as EUR is today; each account can still choose its country; TODO-092 makes it a choice. (owner, 2026-10-18)
- [x] The term? — "Wrapper" / "Enveloppe"; "tax wrapper" / "enveloppe fiscale" in the glossary's definition and the form's help line. (owner, 2026-10-18)
- [x] How does an agent set it? — It reads it in `list_accounts`; it sets it only when it creates an account (TODO-088); an existing account's wrapper is the owner's, in the window. (owner, 2026-10-18)
- [ ] Grouping the accounts list by wrapper: the core groups (F32), so it must also say how groups combine with the table's column sort and its search — sort inside each group, groups in the lists' order, a group with no account left by a search hidden? (Recommended: yes to all three — the account summaries carry each row's group, in the lists' order, the accounts without a wrapper last.) Unknowable at the start: the reviewer found the grouping written in the window on the window part's branch (`feat/todo-091-wrapper-window`, kept, unpushed) (2026-10-18).
- [ ] An account whose country or wrapper the form does not offer — a wrapper another device's newer lists know, or a country set elsewhere: an edit of its name alone is refused today (the edit states both, ACC-044). Keep the value through such an edit, shown by its identifier; clear it with a warning; or refuse the edit? (Recommended: keep it — ACC-040 already keeps a wrapper the lists do not know.) Unknowable at the start: found by the spec review of the window part (2026-10-18).
