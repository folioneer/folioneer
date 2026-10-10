# TODO

<!-- Add new backlog items here. Format: ## TODO-NNN — (domain) — Short title -->
<!-- TODO-NNN is a permanent reference: never renumbered, never reused. A new entry takes the -->
<!-- next free number wherever it is placed. Next free: TODO-067. -->
<!-- Every entry ends with four lines: **User value:**, **Done when:**, **Design:** and -->
<!-- **Open questions:**. Design is `none` until the agent proposes one (it does so before -->
<!-- touching anything the user sees), then `proposed (screenshots/design/NNN-*.png)`, then -->
<!-- `validated` once the human has looked. Open questions are `none` or a `- [ ]` list; an -->
<!-- entry with an open question is not ready. Either side may add a question; the human -->
<!-- answers by editing the entry. -->
<!-- Ordered by user value: entries that change what the user experiences first, -->
<!-- entries with no direct user value after the separator. -->

## Next

<!-- The human's queue: references (TODO-NNN or DEBT-NNN) in the order to work them. The agent -->
<!-- takes the first ready one, removes a reference when it closes the entry (order and -->
<!-- additions are the human's), and stops when the queue is empty. -->

1. DEBT-083
2. TODO-059
3. TODO-060
4. DEBT-096

## TODO-009 — (fullstack) — A per-account analysis view: target price, horizon and reasoning on each holding

Now that prices and rates arrive on their own, what is missing is a place to think. A new view, opened from the account header, lists the account's active holdings with the figures the holdings table already computes — quantity, current value, YTD performance — and adds three fields that are the user's own judgement, edited inline per row: a target price (in the asset's currency), a horizon (short / medium / long term), and free text. This is not the holding note (HNO): the note carries an alarm the application acts on; this carries an opinion the application only stores. One derived figure belongs in the row: how far the current price stands from the target, computed by the backend.

Model: a `HoldingAnalysis` entity keyed by (account, asset) in the account bounded context, all three fields optional, removed when all are cleared. It is user data, so it takes the full sync ceremony — change capture, rank, apply path, tombstone, its own event — which is the cost driver: size it like the holding-note feature (backend, frontend and E2E PRs), not like a screen. Write its spec when scheduled; questions to settle first: whether closed positions appear (probably not — "assets owned"), and whether the row shows the note's alarm threshold read-only beside the target so the two are visibly different things.

**User value:** A working sheet per account where each holding carries the user's target, horizon and reasoning next to its live figures, kept in sync across devices like everything else.
**Done when:** The view lists active holdings with their computed figures and the three editable analysis fields plus distance to target; the entity syncs (captured, applied, tombstoned, announced) with CFR outcomes stated; the trigram is registered and every rule is covered by tests; screenshots and an E2E scenario are committed.
**Design:** none
**Open questions:** none

## TODO-011 — (fullstack) — Monitored assets, price bars, and indicator primitives

Prerequisite work for the private advice module — design in [`advice-module-design.md`](advice-module-design.md) (draft, hook not yet ratified). Two public-side steps, both useful on their own: (1) a `monitored` asset flag plus an `asset_daily_bars` table (OHLCV, separate from `asset_prices` so the latest-write-wins price semantics stay untouched), fetched as one ranged request per monitored asset at the minimum window the enabled indicators need — one year of daily bars covers every requirement including SMA(200), and its month-end closes feed the monthly algorithms without a second call (25 KB / 256 bars measured); afterwards only the missing tail is topped up by the scheduled fetch. (2) Indicator primitives (SMA/EMA, MACD, ATR, RSI, Bollinger, Donchian, monthly closes, drawdown) as pure tested functions plus an indicator panel — readings only, no verdicts. Verdicts and levels stay in the private module. Write its spec when scheduled; the doc's open questions (target weights for 5/25 drift, SMA(200) inclusion) should be closed first.

**User value:** The user reads technical indicators (SMA, EMA, MACD, ATR, RSI, Bollinger, Donchian, drawdown) for the assets they mark as monitored.
**Done when:** The `monitored` flag and `asset_daily_bars` ship with the ranged fetch, the indicator functions are unit-tested, and the panel renders readings for a monitored asset.
**Design:** none
**Open questions:** none

## TODO-025 — (fullstack) — Import transactions from a CSV file

Every transaction is typed by hand today. That is acceptable for one's own portfolio kept up to date week after week; it is what stops anyone with a few years of history — a new user, or the owner opening a second account — from starting at all. Brokers and insurers all export CSV; none of them agree on columns, separators, date or number formats (French exports use `;`, decimal commas, `dd/mm/yyyy`, often Windows-1252).

Proposal: an import, per account, in two steps. (1) The user picks a file and tells which column is what — date, operation, asset (ISIN or reference), quantity, unit price or total, fees, currency — with the separator, decimal mark, date format and encoding detected and correctable; a mapping can be saved under a name and reused, which is what makes "my broker's export" a one-click import without the application knowing any broker. (2) The backend parses, matches each row to an asset (by ISIN, then reference), validates every row through the same rules as manual entry, and returns a preview: rows ready, rows rejected with the reason, assets it does not know, rows that look already recorded. Nothing is written until the user confirms; the confirmed import is all-or-nothing, applied in chronological order, and announces itself once (one `TransactionUpdated`, not one per row — the lesson of TODO-020). All parsing, matching and validation live in Rust; the frontend shows the preview. A new spec with its own trigram comes before any code; one PR per layer.

Constraint found while writing this entry: a purchase needs enough cash in the account (`InsufficientCash`, CSH-041), and most broker exports list trades without the transfers that funded them — an import of trades alone would be rejected row after row. The first open question settles it.

**User value:** A user brings years of history into an account in a few minutes instead of typing it, and re-imports the latest export without creating duplicates.
**Done when:** A CSV exported in French and in English conventions imports into an account through a saved column mapping; the preview lists ready, rejected (with reason), unknown-asset and already-recorded rows before anything is written; a confirmed import is atomic, ordered by date, passes the same validation as manual entry and raises one event; importing the same file twice records nothing new; the new rules are covered by tests and an E2E scenario imports a fixture file; screenshots of the mapping and preview steps are committed.
**Design:** none
**Open questions:**

- [ ] Cash: when the file holds trades but not the money that funded them, does the import (a) add the missing deposits itself, dated with each purchase, (b) refuse and list what is missing, or (c) let the user choose per import? (Recommended: (c), defaulting to (a) — otherwise most broker files cannot be imported.)
- [ ] Which operations does the first version cover — purchases and sales only, or also dividends, deposits and withdrawals? (Recommended: purchases, sales, deposits, withdrawals, dividends; fees in units, splits and free shares stay manual.)
- [ ] An asset the portfolio does not know: create it from the row (name, ISIN, currency — class and category left to fix afterwards), or stop and ask the user to create or pick it first? (Recommended: list them in the preview and let the user create or map each one there.)
- [ ] Which real export should the fixture and the first saved mapping be modelled on — which broker or insurer do you export from?

## TODO-026 — (fullstack) — Asset allocation: how the portfolio splits, and how far it drifts from targets

The application answers "how did it perform?" in several views and "what do I own, and is it balanced?" in none. Everything needed is already recorded on each asset — class, category, currency, risk level — and the portfolio is already valued in EUR across accounts (the Global Value); what is missing is the read that groups that value and the view that shows it.

Proposal: an allocation view over the whole portfolio, filterable to one account, that splits the current value by asset class, by category, by currency and by risk level — amount and share of the total for each slice, cash included, with a drill-down to the holdings behind a slice. The backend computes every figure through the same valuation path as the Global Value (never a second one), including how an asset without a price is counted. Second step, same entry if it stays small: a target share per asset class, stored with the portfolio (and therefore synced), and the drift of each class from its target — which also closes the target-weights question left open in TODO-011's design document. No advice, no "you should": shares, targets, drift.

**User value:** The user sees at a glance how their money is spread — by kind of asset, by currency, by risk — and how far that is from what they intended.
**Done when:** The view shows the four splits with amount and share, for the whole portfolio and for one account, in English and French; the totals equal the Global Value to the cent; a slice opens the holdings behind it; targets per asset class can be set and each class shows its drift; the figures are computed and tested in the backend, including an unpriced asset and a foreign-currency holding; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] Where does it live — a new entry in the navigation, or a tab beside the accounts list and the global performance? (Recommended: its own navigation entry; it is a third question about the portfolio, not a detail of the other two.)
- [ ] Targets in the first version, or splits only first? (Recommended: splits first if targets push the work past one PR per layer; the entry stays open for the second step.)
- [ ] Targets per asset class only, or also per category and per currency? And is a drift just shown, or flagged past a tolerance (for instance the 5/25 rule mentioned in TODO-011)? (Recommended: per class, flagged past a tolerance you set once.)

## TODO-032 — (fullstack) — Let the user choose the reference currency instead of a fixed EUR

Accounts and assets can be in any currency, but every figure that spans accounts — the portfolio total of the accounts list, the global performance, the price movement total, and tomorrow the allocation view (TODO-026) — is reported in EUR, a constant in the code (`REFERENCE_CURRENCY`, GPF-011). For a user whose money is counted in CHF, GBP or USD the global views are in the wrong currency and nothing can change it; it is the one thing that ties the application to the euro area, and the cheapest way to make it usable outside it. The rest already travels: the interface exists in English, dates and numbers follow the locale, the price provider covers the world's markets, and the rate provider serves every pair through its euro base.

Proposal: the reference currency becomes a choice — EUR for every existing portfolio, so nothing changes for anyone who does not touch it. Every cross-account figure is computed in it by the backend, with the rates already stored; changing it recomputes the figures, fetches the history of the pairs now needed (the rate history backfill exists, FXR-110) and says so while it runs. The pair from each account's currency to the reference currency is followed automatically like an asset's pair, so a total never stays partial for lack of a pair the user did not know to declare — which is DEBT-026. The vocabulary gets two distinct terms, one for an account's own currency and one for the currency the portfolio is reported in, plus "portfolio total" — which is DEBT-025. The forms that preselect EUR for a new account or asset preselect the reference currency instead. The GPF, FXR, ACC and PMV rules change in their specs first; one PR per layer.

**User value:** A user who does not count in euros sees their whole portfolio — total, performance, movements — in their own currency, and no account is left out of a total for a missing rate.
**Done when:** The reference currency can be chosen and changed, in English and French; the portfolio total, the global performance and the price movement total are reported in it and recomputed when it changes, with the needed rate history fetched; an account in another currency has its pair to the reference currency followed without the user declaring it, and a USD-only account no longer leaves the total partial; an existing portfolio opens in EUR with every figure unchanged (the golden portfolio does not move); new-account and new-asset forms preselect the reference currency; the vocabulary carries the distinct terms; DEBT-025 and DEBT-026 are removed; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] Is the reference currency a property of the portfolio, the same on every computer and therefore synced, or a setting of each computer? (Recommended: of the portfolio — two computers showing the same portfolio in two currencies is a confusion with no benefit. It needs a synced place for portfolio-wide settings, which TODO-026's targets need too; that is a new kind of synced record, hence the first bump of the sync data format version under the gate of TODO-024.)
- [ ] On a new installation, is it asked when the first account is created, derived from the system's region, or simply EUR until changed? (Recommended: preselected from the first account's currency, shown once, changeable in Settings.)
- [ ] Do the terms suit you — "account currency" for an account's own, "reference currency" for the one the portfolio is reported in ("devise du compte" / "devise de référence")?

## TODO-027 — (fullstack) — A benchmark beside the performance figures

A performance of +7 % says little until it stands beside what a plain index did over the same period. The performance views show the portfolio's and each account's return per period and over a window; nothing to compare them with.

Proposal: the user picks a benchmark — a ticker the price provider knows — and the performance views show its return for the same periods and window, in the same currency as the figures beside it (converted with the rates the application already keeps). The benchmark's daily closes are fetched as one ranged request, the way the price history backfill does, and stored as prices of an asset that is followed without being held — the same notion TODO-011 introduces as a monitored asset, so whichever entry runs first creates it and the other reuses it. An index quoted by the provider is a price index: it leaves dividends out, which flatters the portfolio. An accumulating ETF on the same index does not, so the proposal is to suggest such ETFs rather than the index itself. The comparison is period return against period return; it does not simulate "what if every deposit had gone into the index", which is a different and heavier feature.

**User value:** The user knows whether their portfolio, or one account, did better or worse than simply holding the market over the same period.
**Done when:** A benchmark can be chosen and changed, in English and French; the account and global performance views show its return beside each period and over the window, in the right currency; its prices are fetched on selection and kept up to date by the existing fetches; a benchmark the provider does not know is refused with a clear message; the returns are computed and tested in the backend; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] One benchmark for everything, or one per account (a bond account against a world equity index is not a fair fight)? (Recommended: a global default, overridable per account.)
- [ ] Which benchmarks are offered ready-made — and do you agree to suggest accumulating ETFs (for instance one on MSCI World, one on the CAC 40 or Euro Stoxx 50) rather than the bare indices?
- [ ] Does this wait for TODO-011's monitored assets, or may it introduce the "followed, not held" asset itself if it runs first? (Recommended: introduce it here in its smallest form; TODO-011 extends it.)

## TODO-028 — (fullstack) — Yearly income and costs: what the portfolio paid, and what holding it cost

Dividends and interest are recorded as transactions, and management fees as deductions, but the only way to know what a year brought in is to read the journal and add up. The application shows Dividends Received inside an account's figures; it shows no year, no total across accounts, and nothing that sets income beside the fees paid to hold the assets.

Proposal: a report by calendar year — dividends, interest, and management fees (valued at the price used when each deduction was recorded) — per account and per asset, with a total across accounts in EUR at the rates of each transaction's date, and the months of the current year so far. Amounts are what was recorded: gross or net is whatever the user entered, and the report computes no tax. The backend computes every figure; the view renders and lets a year, an account or an asset be opened.

**User value:** The user knows what their portfolio paid them each year, from which assets, and what holding it cost — the figures they look for at tax time and when judging a contract's fees.
**Done when:** The report shows, per year, dividends, interest and management fees per account and per asset, with a EUR total across accounts, in English and French; the current year is broken down by month; the figures are computed and tested in the backend, including a foreign-currency dividend and a fee taken in units; a year with nothing recorded shows as such rather than as zeroes that look like data; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] Are management fees part of this report, or a separate one? (Recommended: part of it — income without the cost of holding tells half the story, and the data is already there.)
- [ ] Where does it live — a tab of the global performance view, or its own navigation entry?
- [ ] Do you want an export of the yearly figures (CSV), or is reading them on screen enough for now?

## TODO-059 — (backend) — An agent records the rest of an account's past: deposits, withdrawals, dividends, prices

First of seven entries split by the owner on 2026-10-10 from the work left out of 0.7.0 (the agent connection shipped with the read tools and three marked recordings: opening balance, purchase, sale).

The owner's first use. An old account is entered from the start of this year only; the owner holds one statement a year (PDF) for the years before. With an agent reading those statements, the agent enters the first year's position, then derives the movements from one year-end state to the next. The result is an approximation, better than no history, and may be redone when more documents turn up. With this entry and TODO-060 that use is possible end to end.

Four more recordings as tools, each datable years back, through the rules of the window, marked and shown in the journal like the three that exist: a deposit, a withdrawal, a dividend, and a price at a date. The command line gets the same four through the same recorder, so the two interfaces do not drift (TODO-064).

**User value:** An agent enters a whole year of an old account from its statement — the cash that came in and went out, the dividends, the year-end prices — not only the purchases and the sales.
**Done when:** the four recordings exist as tools and as commands of the command line, through one recorder; each is marked with its session and shows in the journal's "Recorded by"; a recorded Claude Code session against the open application lists the tools and calls a read tool and a recording (the clause of TODO-051 that could not be run by the agent).
**Design:** none
**Open questions:**

- [x] The recorded Claude Code session of the Done when cannot be run by the agent: how does the entry close? — The owner records it once, from a script the agent leaves; the entry stays open until then (owner, 2026-10-10).
- [x] A price is not a transaction and never shows in the account journal: how is a price an agent recorded marked? — Through its source: it is stored with the source "Agent", shown where the price history shows a source, counts against the session's limit, never replaces a price already at that date, and "Remove everything it recorded" (TODO-060) removes it; the Mark is widened to a transaction or a price (owner, 2026-10-10).

## TODO-060 — (fullstack) — What an agent recorded can be undone, by it or by the owner

Split from TODO-059 on 2026-10-10. An agent may correct or cancel only transactions marked as its own, never one the owner typed: what the agent reads (a statement) is content it did not write, and must not be able to steer it into removing real history. The header opens a dialog on the session that lists what it read and recorded and offers the owner one action, "Remove everything it recorded".

The mock-up of the session dialog is `screenshots/design/060-*-session.png`, validated by the owner with TODO-051 on 2026-10-09.

**User value:** A reconstruction that went wrong is removed in one action, and an agent can fix its own mistakes without ever touching what the owner typed.
**Done when:** an agent corrects or cancels a transaction marked as its own and is refused on any other; the session dialog opens from the header, lists what the session read and recorded, and removes everything it recorded in one action after a confirmation that refuses by default.
**Design:** validated
**Open questions:** none

## TODO-061 — (fullstack) — An agent asks before touching a transaction the owner typed

Split from TODO-059 on 2026-10-10; after TODO-060. For a transaction the owner typed, the agent may only ask: the window shows the transaction in full (date, asset, quantity, amount) and what would become of it, refusing is the default action, the session's grant never covers it, and there is no "allow all". The position typed at the start of this year is the owner's to remove, in the window or by allowing the agent's request, once the past is rebuilt.

The mock-up of the ask is `screenshots/design/061-*-ask.png`, validated by the owner with TODO-051 on 2026-10-09.

**User value:** The owner lets an agent replace a typed position by the rebuilt history, one transaction at a time, seeing exactly what changes.
**Done when:** a change or a removal of a transaction the owner typed is asked in the window, shown in full with what would become of it, refused by default, one at a time; a refusal gives the agent nothing; nothing of an answer is remembered for the next ask.
**Design:** validated
**Open questions:** none

## TODO-062 — (backend) — "Recorded by" shows on every computer of the sync

Split from TODO-059 on 2026-10-10. The mark of an agent's recording is kept on the computer that recorded it (AGT-045): the owner's other computers show the transaction without its "Recorded by", and "Remove everything it recorded" (TODO-060) would work on one computer only. The mark becomes part of what sync carries, so the data format version moves (SYN-038).

**User value:** Whichever computer the owner opens, the journal says what an agent recorded, and removing a session's recordings removes them everywhere.
**Done when:** a mark written on one computer reaches the others with its transaction and goes with it when the transaction is removed; a computer on the previous format is told to update, as SYN-035 says; the two-device test covers a marked transaction.
**Design:** none
**Open questions:** none

## TODO-063 — (backend) — An agent adds an asset the portfolio does not have

Raised on 2026-10-10 while comparing the three interfaces: the command line adds an asset (CLI-026), an agent cannot. Rebuilding an old account stops at the first instrument the owner no longer holds and never entered.

**User value:** An agent reading an old statement adds the instruments it names — a listed one by its ISIN, a custom one by its name — instead of stopping to ask the owner to create each.
**Done when:** a tool adds an asset of each kind through the rules of the window and of the command line (AST-031, AST-032), refusing the same asset twice; what an agent added is told to the owner in the window; the capabilities table says so (TODO-064).
**Design:** none
**Open questions:**

- [ ] An asset has no mark today. Should the assets table say which assets an agent added, as the journal does for transactions? (It adds a column or a badge: a design to validate.)

## TODO-064 — (fullstack) — One table says what the window, the command line and an agent can each do

Raised on 2026-10-10. The three interfaces share their rules — one recorder, one core — and not their capabilities: the window does everything; the command line records three transactions, adds an asset and lists accounts and assets; an agent reads the summary and the holdings, which the command line cannot, and cannot add an asset, which the command line can. Nothing says which gap is a choice and which an accident.

One document lists each capability and which interface offers it. A check fails when a tool or a command is added without its line. The same table ships with the application and is shown in it, as the page that says what this version can do through each interface (owner, 2026-10-10).

**User value:** The owner reads in the application what an agent and the command line can do in this version, and a new capability never appears in one interface by accident.
**Done when:** the table exists in the docs, one line per capability, one column per interface; a check fails on a tool or a command with no line, seen red; the application shows the table on a page of its own, in both languages, from the same source as the docs.
**Design:** none
**Open questions:**

- [ ] Where does the page live in the application: under Settings beside "Allow agents to connect", or in About?
- [ ] Should the command line read what an agent reads (the portfolio summary, the holdings of an account)? The table makes the gap visible; this decides whether to close it.

## TODO-065 — (fullstack) — The window says when an agent acts, and an agent client knows a read from a recording

Raised on 2026-10-10 by an audit of the agent connection against the protocol's practices (the part of DEBT-089 a user would notice). A recording made by an agent shows only if the journal is open. A call the agent gave up on leaves its connection dialog on screen until the agent's process ends. No tool says whether it only reads or changes something, so an agent client cannot ask its user before a recording and skip the question for a read.

**User value:** The owner sees each recording an agent makes as it happens, whatever view is open; a dialog never asks about a connection nobody waits for; Claude asks before recording and not before reading.
**Done when:** the window shows a notice naming the agent and what it recorded, on every view; a cancelled call (`notifications/cancelled`) withdraws its connection request; each tool carries a title and says whether it only reads (`readOnlyHint`), and a test holds that every recording says it does not.
**Design:** none
**Open questions:** none

## TODO-039 — (service) — A hosted price feed the application can subscribe to (deferred)

What is sold is what a server of the owner's provides: first a price feed under a licence that allows it, later bank feeds, perhaps advice. The application stays free and open, so nothing sold can live in it — a switch in an AGPL client is one fork away from being flipped. The client is ordinary public code: one more price provider behind the seam of TODO-036 and TODO-037, which calls the owner's service with the subscriber's token. The inventory found no licensed replacement — finding and pricing one is this entry's first task. Deferred until then, and until the application is worth showing (TODO-030).

**User value:** A subscriber gets prices fetched for them, daily and on demand, without typing them and without the application standing on an endpoint it has no right to use.
**Done when:** To be written when the entry is scheduled — the service, its licensed source, the subscription and the client each deserve their own entry.
**Design:** none
**Open questions:**

- [ ] Which words name the paid feeds? "Sync" already means multi-device sync in the vocabulary. (Recommended: "price feed" and "bank feed".)
- [ ] Does multi-device sync through a folder the user owns stay free, a hosted sync being a separate paid offer? (Assumed yes when the paid offer was settled, 2026-09-20.)

## TODO-013 — (frontend) — Merge TXL per-asset page into the account journal (deferred)

The per-asset transaction page (`transaction_list/TransactionListPage.tsx`, route `/accounts/$accountId/transactions/$assetId`, the holdings-row loupe target) predates the account journal and is now a strict subset of it — both already share `TransactionTable`, `EditTransactionModal`, delete flow, and `routeEditTransaction`. Consolidate: the loupe navigates to the journal with the asset filter prepopulated (`/accounts/$accountId/journal?asset=<assetId>`); delete the TXL page/hook/route. Decided 2026-07-06: cash-statement columns (Cash out / Cash in / Balance) render only in the unfiltered (global) journal view; with an asset filter active the table shows plain Total Amount — a running balance over a filtered subset is misleading.

Must carry over before deleting TXL: (1) add-transaction CTA + `AddTransactionModal` with prefill from the active filter; (2) the `pendingTransactionAssetId` deep-link round-trip — re-target its senders (`HoldingRow`, `ClosedHoldingRow`, `AssetManager` `returnPath` create-asset flow) to the journal route; (3) fold TXL-0xx spec rules into the journal spec. TXL's in-place account switcher is intentionally dropped. E2E: the suite uses `txl-*` stable ids throughout — rewrite those specs in the same PR (selector-removal trap).

**User value:** One transaction view instead of two near-identical ones; the holdings loupe opens the journal filtered to that asset.
**Done when:** The loupe navigates to `/accounts/$accountId/journal?asset=…`, the TXL page/hook/route are deleted, add-transaction prefill and the `pendingTransactionAssetId` deep link work from the journal, and the `txl-*` specs are rewritten.
**Design:** none
**Open questions:** none

---

<!-- Below: no direct user value — test infrastructure, conventions, dependency currency. -->

## TODO-030 — (site) — A landing page, once the application is worth showing (deferred)

Nobody outside this repository knows the application exists, and the cheapest way to learn whether anyone else wants it is to show it and count. Deferred on purpose: showing it before it can import a history (TODO-025) and answer "what do I own?" (TODO-026) would spend a first impression on a tool strangers cannot start with.

Proposal: one static page, in French and English, in its own repository published with GitHub Pages — so that a wording change goes live in a minute instead of paying this project's harness — with a custom domain only if there is traction. Content: a one-sentence promise; three or four screenshots taken from the screens the E2E suite already captures in light and dark from seeded data (nothing from a real portfolio ever appears); what the alternatives cannot say — no account and no server, encrypted sync through a folder the user owns, management fees taken in units; a short "what happens to my data"; download buttons pointing at the latest release; and the two things a stranger will hit, said plainly: whether the Windows installer is signed (an unsigned one triggers a SmartScreen warning), and that there is no macOS build. Interest is measured without tracking anyone: release download counts, plus a cookie-free page counter at most.

**User value:** None for the current user — people who would want the application can find it, and the owner learns whether they exist before investing in anything paid.
**Done when:** The page is online in French and English with current screenshots and working download links; it states the licence, how data is handled, and the platform limits; download counts and page views can be read without any third-party tracking; the legal notices a site published from France needs are in place.
**Design:** none
**Open questions:**

- [ ] Ready when? (Recommended: after TODO-025 and TODO-026 have shipped.)
- [ ] A domain from the start, or the free GitHub Pages address first? (`folioneer.com` and `folioneer.app` are registered; they are the natural choice.)
- [ ] Is the Windows installer to be signed before strangers download it, and how? Today only the updater signature exists (it proves an update comes from the owner; SmartScreen ignores it) — no Windows code signing. The options, prices and eligibility to be checked again when the time comes: (a) SignPath Foundation — free, publisher shown as "SignPath Foundation", open-source licence and CI-built releases required; (b) a Certum open-source certificate — tens of euros a year, the owner's name shown, open-source project required, signing through their cloud service since keys must live on hardware; (c) Azure Trusted Signing — about ten dollars a month, eligibility of individuals depends on the country; (d) a standard certificate with cloud signing — a few hundred euros a year, no open-source condition; (e) no signing, and the page explains the warning. Even signed, the warning lasts until the certificate has earned reputation — which then carries over from one release to the next, whereas an unsigned installer starts from zero each time. (a) and (b) exist only while the licence is open source, which the AGPL-3.0-or-later is. (Recommended: (b) if the owner's name should show, (a) if free matters more.) Wiring any of them is one signing command in the Tauri configuration plus a credential in the release workflow.

## TODO-015 — (deps) — Upgrade specta / tauri-specta / specta-typescript past rc.22

Pinned at `specta 2.0.0-rc.22`, `tauri-specta 2.0.0-rc.21`, `specta-typescript 0.0.9`. The lockstep bump to rc.25 / rc.25 / 0.0.12 was attempted on 2026-09-12 and reverted: `specta-typescript 0.0.12` removed the global `Typescript::bigint(BigIntExportBehavior::Number)` switch this project relies on, and now refuses to export any unannotated `i64` / `u64` (`Error::bigint_forbidden`). The replacement is a per-field `#[specta(type = specta_typescript::Number)]` override (or a wrapper type) on every 64-bit integer that crosses the wire — which, under ADR-001, is every monetary amount, quantity, rate and percentage in every DTO and command signature. That is an annotation sweep across the whole wire surface, not a dependency bump, and a single missed field fails bindings generation.

**User value:** None — dependency currency.
**Done when:** every wire-visible 64-bit integer carries the `Number` override (or a shared newtype does), `just generate-types` produces a bindings diff that is cosmetic only, and the three crates sit on a current release together.
**Design:** none
**Open questions:** none

## TODO-016 — (deps) — Accepted risk: WebdriverIO 9 transitive advisories (extract-zip)

`npm audit` reports 22 advisories (3 low, 1 moderate, 18 high) at the 2026-10-03 release audit — 14 (2 low, 12 high) on 2026-09-19 — all in `devDependencies`, all reached through the WebdriverIO packages; `npm audit --omit=dev` reports none. On 2026-09-19 the twelve highs shared one root: `extract-zip`, affected in every published version and reached only through `@puppeteer/browsers 2.13.2`, which WebdriverIO 9.31 pins for downloading browser binaries the suite never uses (it drives the Tauri binary through tauri-driver). npm's only proposed fix is a downgrade to WebdriverIO 8.14.6. Checked at the 2026-09-19 release audit: `extract-zip` has no patched release and none is planned (2.0.1 is its last); `@puppeteer/browsers` 3.x replaced it with another extractor, but WebdriverIO 9.31.9 — the latest — still depends on `@puppeteer/browsers ^2.2.0`, so the only way out today is an npm override across a major version of a package the E2E runner calls. The two lows are mocha's bundled `diff`. The `deepmerge-ts` and `js-yaml` advisories were cleared by the in-range bump of 2026-09-12 (`ff986f1`). Nothing from these packages enters the application bundle or the Tauri binary, and CI's `npm audit --omit=dev` gate is green. Re-run `npm audit` at each release.

**User value:** None — devDependency advisories; nothing from them enters the shipped bundle.
**Done when:** WebdriverIO depends on `@puppeteer/browsers` 3.x or later (which no longer carries `extract-zip`), the bump is taken, `npm audit` is clean, and this entry is deleted.
**Design:** none
**Open questions:** none

## TODO-017 — (deps) — Accepted risk: RUSTSEC-2023-0071 (rsa Marvin Attack)

`cargo audit` flags `rsa 0.9.10` (timing sidechannel, CVSS 5.9 medium) with no upstream fix. Pulled transitively via `sqlx-mysql 0.8.6` because the `sqlx` macro crate compiles all backends regardless of enabled features. We only enable `sqlite`, so the vulnerable RSA path is never reached at runtime. Re-evaluate when sqlx ships a fix or when we change DB backend.

**User value:** None — the vulnerable RSA path is unreachable in a SQLite-only build.
**Done when:** sqlx stops compiling `sqlx-mysql` for sqlite-only builds or `rsa` publishes a fix, `cargo audit` is clean, and this entry is deleted.
**Design:** none
**Open questions:** none
