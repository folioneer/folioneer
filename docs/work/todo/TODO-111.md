# TODO-111 — (fullstack) — A wrapper is a definition: its country, what it may hold, its ceiling, its interest

Asked by the owner (2026-10-18), answering TODO-091's open questions. Since TODO-091 a wrapper is a label from lists kept in the code (`src-tauri/src/context/account/domain/wrapper.rs`): generic, France, Germany. A wrapper is more than a name: a Livret A holds only cash, up to 23,000 €, at a yearly interest rate. The informative ceilings of gh312 belong here. Livret A, LDDS and LEP join France's list with their definitions, and the generic "Life insurance" — a second "Assurance vie" in French — leaves it.

**Database model (owner, 2026-10-18):** one table of wrappers; all of them live there, the French ones included — never some in the code and others in the database. A row holds:

- `id` — the identifier an account stores, the same as TODO-091's (`fr_pea`, `brokerage`, …), so no account moves;
- `country` — ISO 3166-1 alpha-2, or none for a generic wrapper (a wrapper belongs to a country; any account may hold it — TODO-091);
- its names — English, French, German for the seeded rows;
- `config` — a text field holding JSON: what it may hold, its ceiling, its rate, and whatever the definition gains later, with no new column. The backend reads it into a typed structure where every setting has a default, so a row written before a setting existed still reads.

Today's lists are seeded by the migration; a wrapper the user adds is synced like a category (a new record kind: a sync format version with its snapshot); the code lists disappear in the same change.

**User value:** A user sees what each wrapper allows — what it may hold, how much, at what rate — and adds the wrappers of their country without waiting for a release.
**Done when:** to be written by the owner — at least: the table above exists and is the only source of the lists; existing accounts keep their wrapper; the user can add and rename a wrapper; the Livret A, LDDS and LEP are listed with their definitions; the golden portfolio does not move.
**Size:** —
**Design:** none
**Open questions:**

- [ ] Which settings the first `config` carries (allowed holdings, ceiling, rate) and what the app does with each — informative only, as gh312 says, or a warning on a deposit past the ceiling?
