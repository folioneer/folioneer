# TODO-028 — (fullstack) — Yearly income and costs: what the portfolio paid, and what holding it cost

Dividends and interest are recorded as transactions, and management fees as deductions, but the only way to know what a year brought in is to read the journal and add up. The application shows Dividends Received inside an account's figures; it shows no year, no total across accounts, and nothing that sets income beside the fees paid to hold the assets.

Proposal: a report by calendar year — dividends, interest, and management fees (valued at the price used when each deduction was recorded) — per account and per asset, with a total across accounts in EUR at the rates of each transaction's date, and the months of the current year so far. Amounts are what was recorded: gross or net is whatever the user entered, and the report computes no tax. The backend computes every figure; the view renders and lets a year, an account or an asset be opened.

**User value:** The user knows what their portfolio paid them each year, from which assets, and what holding it cost — the figures they look for at tax time and when judging a contract's fees.
**Done when:** The report shows, per year, dividends, interest and management fees per account and per asset, with a EUR total across accounts, in English and French; the current year is broken down by month; the figures are computed and tested in the backend, including a foreign-currency dividend and a fee taken in units; a year with nothing recorded shows as such rather than as zeroes that look like data; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [x] Are management fees part of this report, or a separate one? (Recommended: part of it — income without the cost of holding tells half the story, and the data is already there.) — Part of it, beside the income. (owner, 2026-10-17)
- [x] Where does it live — a tab of the global performance view, or its own navigation entry? — Its own navigation entry. (owner, 2026-10-17)
- [x] Do you want an export of the yearly figures (CSV), or is reading them on screen enough for now? — A CSV export. (owner, 2026-10-17)
