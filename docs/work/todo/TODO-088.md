# TODO-088 — (backend) — An agent creates an account

From gh246 (owner, 2026-10-12), which this entry closes when it ships. An agent reads the accounts and records everything inside one, and cannot create one: bringing a new envelope in from statements starts with the owner creating the account by hand, then going back to the agent.

A tool `add_account`, through the rules of the window (ACC-001 to ACC-004, ACC-026): a name, a currency, and optionally the bank and the update frequency; answered with the account, usable at once by its name. Renaming or deleting an account stays the owner's: a deletion takes the account's holdings with it (ACC-005).

Read on 2026-10-13, against what 0.10.0 decided for an asset an agent adds (TODO-063): there the session's grant covers the addition, the connection dialog names it on a line of its own, it takes a place of the session's limit, and nothing marks the asset — the session dialog counts them (DEBT-119 is the mark, for later). The issue asks more of an account: an ask in the window for each creation, as TODO-061 will ask for a removal, and a mark. An account is as empty and as harmless as an asset until something is recorded in it; asking for each would make it wait for TODO-061's dialog and a mock-up of its own. And its last clause, a line of the capabilities table, belongs to TODO-064, as it did for TODO-063.

**User value:** The owner hands an agent the statements of an account Folioneer does not know and gets it created and filled in one go.
**Done when:** `add_account` exists, applies the window's rules and answers their refusals — the same name twice, a currency that is none, an empty name; the creation is consented to as decided below; the window says what the session created, as decided below; the agent's spec and the connection dialog say so.
**Design:** none
**Open questions:**

- [ ] Asked in the window for each account, refused by default (after TODO-061, with its mock-up), or covered by the session's grant and named in the connection dialog, as adding an asset is? (Recommended: covered by the grant — an empty account does no more than an unused asset.)
- [ ] What the window says of it: a count in the session dialog, as for assets, or a mark on the account (DEBT-119 asks the same of an asset)?
- [ ] Which optional fields on the first day: the bank and the update frequency, and the management fees' switch or not?
