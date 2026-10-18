# TODO-110 — (fullstack) — An account's update frequency is removed, or does something real

Asked by the owner (2026-10-18), on TODO-091's mock-up. Every account carries an update frequency (Automatic, daily, weekly, monthly, yearly — ACC-004), asked on the account form and shown as a column of the accounts list. The spec calls it informational: nothing is automated from it. Read on 2026-10-18, it decides one thing: an account updated monthly or yearly is offered no month view of its performance (PRF-013), however many prices it has. TODO-088 lists it among the fields an agent may give on creation, and TODO-092's first run would ask it of a new user.

**User value:** The account form asks nothing that does nothing; the month view of performance is offered when the data supports it, not on a word chosen at creation.
**Done when:** the owner's choice below is built: either the frequency leaves the form, the accounts list, the agent's tools and the specs (ACC-004, ACC-015 to ACC-017, PRF-013), the month view following the account's data instead, and existing accounts losing nothing else; or it does what is decided below, said on the form; the golden portfolio does not move either way.
**Design:** none
**Open questions:**

- [ ] Remove the frequency, or give it a real effect (for instance a reminder when an account has not been updated for longer than its frequency, or the old-price threshold of TODO-085 for its holdings)? (Recommended: remove it — the outdated price of TODO-085 already says which account needs attention, from the prices themselves.)
- [ ] If removed: the month view of performance is offered for every account, or only when the account has prices in most months? (Recommended: for every account — a month without a price carries the last one forward, PRF-022.)
