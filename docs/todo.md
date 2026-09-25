# TODO

<!-- Add new backlog items here. Format: ## #NNN — (domain) — Short title -->
<!-- #NNN is a permanent reference: never renumbered, never reused. A new entry takes the -->
<!-- next free number wherever it is placed. Next free: #043. -->
<!-- Every entry ends with four lines: **User value:**, **Done when:**, **Design:** and -->
<!-- **Open questions:**. Design is `none` until the agent proposes one (it does so before -->
<!-- touching anything the user sees), then `proposed (screenshots/design/NNN-*.png)`, then -->
<!-- `validated` once the human has looked. Open questions are `none` or a `- [ ]` list; an -->
<!-- entry with an open question is not ready. Either side may add a question; the human -->
<!-- answers by editing the entry. -->
<!-- Ordered by user value: entries that change what the user experiences first, -->
<!-- entries with no direct user value after the separator. -->

## Next

<!-- The human's queue: references (#NNN or TD-NNN) in the order to work them. The agent -->
<!-- takes the first ready one, never edits this list, and stops when it is empty. -->

Nothing queued.

## #009 — (fullstack) — A per-account analysis view: target price, horizon and reasoning on each holding

Now that prices and rates arrive on their own, what is missing is a place to think. A new view, opened from the account header, lists the account's active holdings with the figures the holdings table already computes — quantity, current value, YTD performance — and adds three fields that are the user's own judgement, edited inline per row: a target price (in the asset's currency), a horizon (short / medium / long term), and free text. This is not the holding note (HNO): the note carries an alarm the application acts on; this carries an opinion the application only stores. One derived figure belongs in the row: how far the current price stands from the target, computed by the backend.

Model: a `HoldingAnalysis` entity keyed by (account, asset) in the account bounded context, all three fields optional, removed when all are cleared. It is user data, so it takes the full sync ceremony — change capture, rank, apply path, tombstone, its own event — which is the cost driver: size it like the holding-note feature (backend, frontend and E2E PRs), not like a screen. Route through `/spec-writer` when scheduled; questions for the interview: whether closed positions appear (probably not — "assets owned"), and whether the row shows the note's alarm threshold read-only beside the target so the two are visibly different things.

**User value:** A working sheet per account where each holding carries the user's target, horizon and reasoning next to its live figures, kept in sync across devices like everything else.
**Done when:** The view lists active holdings with their computed figures and the three editable analysis fields plus distance to target; the entity syncs (captured, applied, tombstoned, announced) with CFR outcomes stated; the trigram is registered and every rule is covered by tests; screenshots and an E2E scenario are committed.
**Design:** none
**Open questions:** none

## #010 — (frontend) — Give multi-device sync its own view and rework its UI

Sync ships as one `SyncSection` inside the settings page (`src/features/settings/sync/`, ~12 KB of TSX). That section now carries the whole feature: the status block (enabled/paused, device name, folder, last sync), the roster of other computers, held-back counts, failures, conflict notices, inconsistent holdings, six actions (Sync now, Pause, Rename, Change folder, Leave, Start over), two modals, and a single-field prompt shared between rename and change-folder. It has outgrown a settings section.

Observed friction (2026-08-28, first real two-computer setup):

- The six actions render as one flat row of buttons with no grouping by consequence — "Sync now" sits beside "Start over", which discards every published file.
- The shared rename/change-folder prompt forced a conditional Browse button, since only one of the two takes a path. Splitting them into purpose-built dialogs removes the conditional.
- `InstallationHoldsUserData` states that joining requires a fresh installation but gives no route forward; the user has to be told out-of-band which directory to clear. The error should explain the remedy, and ideally offer it.
- Status is a bare `<dl>` — no sense of health at a glance, and nothing shows whether the other computer has published yet, which is the first thing a user checks when a join fails.

Proposal: promote sync to its own route (`/sync`, following the `/performance` precedent in `src/router.tsx`), leaving settings with a link and, at most, the enabled/paused summary. Group the actions by consequence (routine / device / destructive), split the shared prompt, and give the status block a health-oriented layout.

Carry-over risk: `e2e/sync/sync.test.ts` selects on `sync-*` stable ids throughout (`#sync-now`, `#sync-pause`, `#sync-leave`, `#sync-change-folder`, `#sync-enable-*`, `#sync-indicator`). Moving or regrouping those elements breaks the suite silently — `just check` does not type-check `e2e/`. Rewrite the specs in the same PR.

**User value:** Sync health is readable at a glance, destructive actions sit apart from routine ones, and a refused join states how to fix it.
**Done when:** Sync lives at `/sync` with a link from settings, actions are grouped by consequence, rename and change-folder have separate dialogs, `InstallationHoldsUserData` names the remedy, and `e2e/sync/sync.test.ts` passes on the new ids.
**Design:** none
**Open questions:** none

## #011 — (fullstack) — Monitored assets, price bars, and indicator primitives

Prerequisite work for the private advice module — design in [`advice-module-design.md`](advice-module-design.md) (draft, hook not yet ratified). Two public-side steps, both useful on their own: (1) a `monitored` asset flag plus an `asset_daily_bars` table (OHLCV, separate from `asset_prices` so the latest-write-wins price semantics stay untouched), fetched as one ranged request per monitored asset at the minimum window the enabled indicators need — one year of daily bars covers every requirement including SMA(200), and its month-end closes feed the monthly algorithms without a second call (25 KB / 256 bars measured); afterwards only the missing tail is topped up by the scheduled fetch. (2) Indicator primitives (SMA/EMA, MACD, ATR, RSI, Bollinger, Donchian, monthly closes, drawdown) as pure tested functions plus an indicator panel — readings only, no verdicts. Verdicts and levels stay in the private module. Route through /spec-writer when scheduled; the doc's open questions (target weights for 5/25 drift, SMA(200) inclusion) should be closed first.

**User value:** The user reads technical indicators (SMA, EMA, MACD, ATR, RSI, Bollinger, Donchian, drawdown) for the assets they mark as monitored.
**Done when:** The `monitored` flag and `asset_daily_bars` ship with the ranged fetch, the indicator functions are unit-tested, and the panel renders readings for a monitored asset.
**Design:** none
**Open questions:** none

## #012 — (fullstack) — Explain suppressed lifetime performance metrics instead of a bare "—"

When the since-inception % and annualized-yield columns are suppressed by the Dietz guard (denominator ≤ 0), the performance view shows "—" with no cause, which reads as a bug. Real-world trigger (CTO account, 2026-07-27): opening balances typed with unit price 0 (employee free shares) plus early withdrawals make the lifetime denominator negative forever, while the windowed Perf % column computes fine — the user cannot tell the data is fixable. Proposal: the response carries a degradation reason for suppressed lifetime metrics (e.g. zero-valued opening balance vs. genuinely undefined), and the view surfaces a persistent contextual hint (info icon/tooltip on the suppressed cells — not a snackbar, which is transient and re-fires) telling the user which transaction to correct. Needs a PRF spec rule + contract field + both layers; route through /spec-writer when scheduled. Companion guardrail at the entry side (user decision 2026-07-27: warn, don't block — a truly worthless position is legitimate): when an opening-balance form is submitted with Total Cost 0, show an inline warning that zero declares no starting capital and suppresses lifetime performance, suggesting the entry-date market value instead.

**User value:** When lifetime performance shows “—”, the user learns why and which transaction to correct.
**Done when:** The response carries a degradation reason, suppressed cells show a persistent hint naming the cause, and an opening balance submitted with Total Cost 0 warns inline.
**Design:** none
**Open questions:** none

## #025 — (fullstack) — Import transactions from a CSV file

Every transaction is typed by hand today. That is acceptable for one's own portfolio kept up to date week after week; it is what stops anyone with a few years of history — a new user, or the owner opening a second account — from starting at all. Brokers and insurers all export CSV; none of them agree on columns, separators, date or number formats (French exports use `;`, decimal commas, `dd/mm/yyyy`, often Windows-1252).

Proposal: an import, per account, in two steps. (1) The user picks a file and tells which column is what — date, operation, asset (ISIN or reference), quantity, unit price or total, fees, currency — with the separator, decimal mark, date format and encoding detected and correctable; a mapping can be saved under a name and reused, which is what makes "my broker's export" a one-click import without the application knowing any broker. (2) The backend parses, matches each row to an asset (by ISIN, then reference), validates every row through the same rules as manual entry, and returns a preview: rows ready, rows rejected with the reason, assets it does not know, rows that look already recorded. Nothing is written until the user confirms; the confirmed import is all-or-nothing, applied in chronological order, and announces itself once (one `TransactionUpdated`, not one per row — the lesson of #020). All parsing, matching and validation live in Rust; the frontend shows the preview. Route through `/spec-writer` (new spec, its own trigram) before any code; one PR per layer.

Constraint found while writing this entry: a purchase needs enough cash in the account (`InsufficientCash`, CSH-041), and most broker exports list trades without the transfers that funded them — an import of trades alone would be rejected row after row. The first open question settles it.

**User value:** A user brings years of history into an account in a few minutes instead of typing it, and re-imports the latest export without creating duplicates.
**Done when:** A CSV exported in French and in English conventions imports into an account through a saved column mapping; the preview lists ready, rejected (with reason), unknown-asset and already-recorded rows before anything is written; a confirmed import is atomic, ordered by date, passes the same validation as manual entry and raises one event; importing the same file twice records nothing new; the new rules are covered by tests and an E2E scenario imports a fixture file; screenshots of the mapping and preview steps are committed.
**Design:** none
**Open questions:**

- [ ] Cash: when the file holds trades but not the money that funded them, does the import (a) add the missing deposits itself, dated with each purchase, (b) refuse and list what is missing, or (c) let the user choose per import? (Recommended: (c), defaulting to (a) — otherwise most broker files cannot be imported.)
- [ ] Which operations does the first version cover — purchases and sales only, or also dividends, deposits and withdrawals? (Recommended: purchases, sales, deposits, withdrawals, dividends; fees in units, splits and free shares stay manual.)
- [ ] An asset the portfolio does not know: create it from the row (name, ISIN, currency — class and category left to fix afterwards), or stop and ask the user to create or pick it first? (Recommended: list them in the preview and let the user create or map each one there.)
- [ ] Which real export should the fixture and the first saved mapping be modelled on — which broker or insurer do you export from?

## #026 — (fullstack) — Asset allocation: how the portfolio splits, and how far it drifts from targets

The application answers "how did it perform?" in several views and "what do I own, and is it balanced?" in none. Everything needed is already recorded on each asset — class, category, currency, risk level — and the portfolio is already valued in EUR across accounts (the Global Value); what is missing is the read that groups that value and the view that shows it.

Proposal: an allocation view over the whole portfolio, filterable to one account, that splits the current value by asset class, by category, by currency and by risk level — amount and share of the total for each slice, cash included, with a drill-down to the holdings behind a slice. The backend computes every figure through the same valuation path as the Global Value (never a second one), including how an asset without a price is counted. Second step, same entry if it stays small: a target share per asset class, stored with the portfolio (and therefore synced), and the drift of each class from its target — which also closes the target-weights question left open in #011's design document. No advice, no "you should": shares, targets, drift.

**User value:** The user sees at a glance how their money is spread — by kind of asset, by currency, by risk — and how far that is from what they intended.
**Done when:** The view shows the four splits with amount and share, for the whole portfolio and for one account, in English and French; the totals equal the Global Value to the cent; a slice opens the holdings behind it; targets per asset class can be set and each class shows its drift; the figures are computed and tested in the backend, including an unpriced asset and a foreign-currency holding; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] Where does it live — a new entry in the navigation, or a tab beside the accounts list and the global performance? (Recommended: its own navigation entry; it is a third question about the portfolio, not a detail of the other two.)
- [ ] Targets in the first version, or splits only first? (Recommended: splits first if targets push the work past one PR per layer; the entry stays open for the second step.)
- [ ] Targets per asset class only, or also per category and per currency? And is a drift just shown, or flagged past a tolerance (for instance the 5/25 rule mentioned in #011)? (Recommended: per class, flagged past a tolerance you set once.)

## #032 — (fullstack) — Let the user choose the reference currency instead of a fixed EUR

Accounts and assets can be in any currency, but every figure that spans accounts — the portfolio total of the accounts list, the global performance, the price movement total, and tomorrow the allocation view (#026) — is reported in EUR, a constant in the code (`REFERENCE_CURRENCY`, GPF-011). For a user whose money is counted in CHF, GBP or USD the global views are in the wrong currency and nothing can change it; it is the one thing that ties the application to the euro area, and the cheapest way to make it usable outside it. The rest already travels: the interface exists in English, dates and numbers follow the locale, the price provider covers the world's markets, and the rate provider serves every pair through its euro base.

Proposal: the reference currency becomes a choice — EUR for every existing portfolio, so nothing changes for anyone who does not touch it. Every cross-account figure is computed in it by the backend, with the rates already stored; changing it recomputes the figures, fetches the history of the pairs now needed (the rate history backfill exists, FXR-110) and says so while it runs. The pair from each account's currency to the reference currency is followed automatically like an asset's pair, so a total never stays partial for lack of a pair the user did not know to declare — which is TD-026. The vocabulary gets two distinct terms, one for an account's own currency and one for the currency the portfolio is reported in, plus "portfolio total" — which is TD-025. The forms that preselect EUR for a new account or asset preselect the reference currency instead. Route through `/spec-writer` (GPF, FXR, ACC and PMV rules change); one PR per layer.

**User value:** A user who does not count in euros sees their whole portfolio — total, performance, movements — in their own currency, and no account is left out of a total for a missing rate.
**Done when:** The reference currency can be chosen and changed, in English and French; the portfolio total, the global performance and the price movement total are reported in it and recomputed when it changes, with the needed rate history fetched; an account in another currency has its pair to the reference currency followed without the user declaring it, and a USD-only account no longer leaves the total partial; an existing portfolio opens in EUR with every figure unchanged (the golden portfolio does not move); new-account and new-asset forms preselect the reference currency; the vocabulary carries the distinct terms; TD-025 and TD-026 are removed; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] Is the reference currency a property of the portfolio, the same on every computer and therefore synced, or a setting of each computer? (Recommended: of the portfolio — two computers showing the same portfolio in two currencies is a confusion with no benefit. It needs a synced place for portfolio-wide settings, which #026's targets need too; that is a new kind of synced record, hence the first bump of the sync data format version under the gate of #024.)
- [ ] On a new installation, is it asked when the first account is created, derived from the system's region, or simply EUR until changed? (Recommended: preselected from the first account's currency, shown once, changeable in Settings.)
- [ ] Do the terms suit you — "account currency" for an account's own, "reference currency" for the one the portfolio is reported in ("devise du compte" / "devise de référence")?

## #027 — (fullstack) — A benchmark beside the performance figures

A performance of +7 % says little until it stands beside what a plain index did over the same period. The performance views show the portfolio's and each account's return per period and over a window; nothing to compare them with.

Proposal: the user picks a benchmark — a ticker the price provider knows — and the performance views show its return for the same periods and window, in the same currency as the figures beside it (converted with the rates the application already keeps). The benchmark's daily closes are fetched as one ranged request, the way the price history backfill does, and stored as prices of an asset that is followed without being held — the same notion #011 introduces as a monitored asset, so whichever entry runs first creates it and the other reuses it. An index quoted by the provider is a price index: it leaves dividends out, which flatters the portfolio. An accumulating ETF on the same index does not, so the proposal is to suggest such ETFs rather than the index itself. The comparison is period return against period return; it does not simulate "what if every deposit had gone into the index", which is a different and heavier feature.

**User value:** The user knows whether their portfolio, or one account, did better or worse than simply holding the market over the same period.
**Done when:** A benchmark can be chosen and changed, in English and French; the account and global performance views show its return beside each period and over the window, in the right currency; its prices are fetched on selection and kept up to date by the existing fetches; a benchmark the provider does not know is refused with a clear message; the returns are computed and tested in the backend; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] One benchmark for everything, or one per account (a bond account against a world equity index is not a fair fight)? (Recommended: a global default, overridable per account.)
- [ ] Which benchmarks are offered ready-made — and do you agree to suggest accumulating ETFs (for instance one on MSCI World, one on the CAC 40 or Euro Stoxx 50) rather than the bare indices?
- [ ] Does this wait for #011's monitored assets, or may it introduce the "followed, not held" asset itself if it runs first? (Recommended: introduce it here in its smallest form; #011 extends it.)

## #028 — (fullstack) — Yearly income and costs: what the portfolio paid, and what holding it cost

Dividends and interest are recorded as transactions, and management fees as deductions, but the only way to know what a year brought in is to read the journal and add up. The application shows Dividends Received inside an account's figures; it shows no year, no total across accounts, and nothing that sets income beside the fees paid to hold the assets.

Proposal: a report by calendar year — dividends, interest, and management fees (valued at the price used when each deduction was recorded) — per account and per asset, with a total across accounts in EUR at the rates of each transaction's date, and the months of the current year so far. Amounts are what was recorded: gross or net is whatever the user entered, and the report computes no tax. The backend computes every figure; the view renders and lets a year, an account or an asset be opened.

**User value:** The user knows what their portfolio paid them each year, from which assets, and what holding it cost — the figures they look for at tax time and when judging a contract's fees.
**Done when:** The report shows, per year, dividends, interest and management fees per account and per asset, with a EUR total across accounts, in English and French; the current year is broken down by month; the figures are computed and tested in the backend, including a foreign-currency dividend and a fee taken in units; a year with nothing recorded shows as such rather than as zeroes that look like data; screenshots in light and dark mode are committed.
**Design:** none
**Open questions:**

- [ ] Are management fees part of this report, or a separate one? (Recommended: part of it — income without the cost of holding tells half the story, and the data is already there.)
- [ ] Where does it live — a tab of the global performance view, or its own navigation entry?
- [ ] Do you want an export of the yearly figures (CSV), or is reading them on screen enough for now?

## #038 — (release) — A private build channel for the owner, then Yahoo leaves the public repository

Its prerequisites have landed: a build without an External provider offers nothing it cannot run (#036), exchange rates refresh without one (#042), and the extension file it overlays exists (ADR-020). Most of the work lives in a private repository, `folioneer/folioneer-private`; this entry tracks it and owns the one public commit that ends it. The private repository holds its own `extensions.rs` and the Yahoo client, pins the public repository as a submodule at a release tag, copies its files over the submodule, builds with the same action as the public release and publishes to its own releases — nothing of it appears on the public releases page. Same identifier, so the same data folder: installing a public build over it loses automatic prices and nothing else. The build says what it is (`X.Y.Z+private` in About). Order, always: the owner releases publicly, then the private workflow builds that tag.

Updates: the private build reads an access token from a file in its configuration folder, never from the binary, and sends it through the headers of #037. Probe first, on a throwaway private release: private assets are probably served only through the API address, in which case the workflow writes its own `latest.json`. If the probe fails, the private build only notifies, and a recipe downloads and installs. The probe also says what a refusal looks like from that host: the application reports HTTP 401 and 403 as a refused access (UPD-028), and a host that answers 404 to a refused token needs that rule extended here.

The inventory of 2026-09-21 ([`external-dependencies.md`](external-dependencies.md)) sharpened why this is owed: Yahoo's terms forbid automated access itself (§2.4.9), not only commercial use, so the free application is outside them today, whatever is sold.

Last step, once the private build runs and updates itself on both of the owner's computers: the Yahoo client and its tests are deleted from the public repository, the public `extensions.rs` returns no provider, ADR-017 is superseded, the coverage floors are re-measured, and the next public release is the first without automatic prices.

**User value:** None for a standard user, who keeps one installer and one update channel and never sees the other; the owner keeps automatic prices.
**Done when:** The probe's result is recorded; the private workflow produces installers for Windows and Linux from a public tag; the owner's two computers run the private build, see their existing data and update from the private channel (or the fallback is in place); a refused token is reported in the application; then the public repository holds no Yahoo code, its harness is green with the re-measured floors, and ARCHITECTURE.md and the ADR index are current.
**Design:** none
**Open questions:**

- [ ] If the probe shows self-update from private releases is not workable, is "notify, then a recipe downloads and installs" acceptable on both computers? (Recommended: yes — one user, two computers.)

## #039 — (service) — A hosted price feed the application can subscribe to (deferred)

What is sold is what a server of the owner's provides: first a price feed under a licence that allows it, later bank feeds, perhaps advice. The application stays free and open, so nothing sold can live in it — a switch in an AGPL client is one fork away from being flipped. The client is ordinary public code: one more price provider behind the seam of #036 and #037, which calls the owner's service with the subscriber's token. The inventory found no licensed replacement — finding and pricing one is this entry's first task. Deferred until then, and until the application is worth showing (#030).

**User value:** A subscriber gets prices fetched for them, daily and on demand, without typing them and without the application standing on an endpoint it has no right to use.
**Done when:** To be written when the entry is scheduled — the service, its licensed source, the subscription and the client each deserve their own entry.
**Design:** none
**Open questions:**

- [ ] Which words name the paid feeds? "Sync" already means multi-device sync in the vocabulary. (Recommended: "price feed" and "bank feed".)
- [ ] Does multi-device sync through a folder the user owns stay free, a hosted sync being a separate paid offer? (Assumed yes when the paid offer was settled, 2026-09-20.)

## #013 — (frontend) — Merge TXL per-asset page into the account journal (deferred)

The per-asset transaction page (`transaction_list/TransactionListPage.tsx`, route `/accounts/$accountId/transactions/$assetId`, the holdings-row loupe target) predates the account journal and is now a strict subset of it — both already share `TransactionTable`, `EditTransactionModal`, delete flow, and `routeEditTransaction`. Consolidate: the loupe navigates to the journal with the asset filter prepopulated (`/accounts/$accountId/journal?asset=<assetId>`); delete the TXL page/hook/route. Decided 2026-07-06: cash-statement columns (Cash out / Cash in / Balance) render only in the unfiltered (global) journal view; with an asset filter active the table shows plain Total Amount — a running balance over a filtered subset is misleading.

Must carry over before deleting TXL: (1) add-transaction CTA + `AddTransactionModal` with prefill from the active filter; (2) the `pendingTransactionAssetId` deep-link round-trip — re-target its senders (`HoldingRow`, `ClosedHoldingRow`, `AssetManager` `returnPath` create-asset flow) to the journal route; (3) fold TXL-0xx spec rules into the journal spec. TXL's in-place account switcher is intentionally dropped. E2E: the suite uses `txl-*` stable ids throughout — rewrite those specs in the same PR (selector-removal trap).

**User value:** One transaction view instead of two near-identical ones; the holdings loupe opens the journal filtered to that asset.
**Done when:** The loupe navigates to `/accounts/$accountId/journal?asset=…`, the TXL page/hook/route are deleted, add-transaction prefill and the `pendingTransactionAssetId` deep link work from the journal, and the `txl-*` specs are rewritten.
**Design:** none
**Open questions:** none

---

<!-- Below: no direct user value — test infrastructure, conventions, dependency currency. -->

## #030 — (site) — A landing page, once the application is worth showing (deferred)

Nobody outside this repository knows the application exists, and the cheapest way to learn whether anyone else wants it is to show it and count. Deferred on purpose: showing it before it can import a history (#025) and answer "what do I own?" (#026) would spend a first impression on a tool strangers cannot start with.

Proposal: one static page, in French and English, in its own repository published with GitHub Pages — so that a wording change goes live in a minute instead of paying this project's harness — with a custom domain only if there is traction. Content: a one-sentence promise; three or four screenshots taken from the screens the E2E suite already captures in light and dark from seeded data (nothing from a real portfolio ever appears); what the alternatives cannot say — no account and no server, encrypted sync through a folder the user owns, management fees taken in units; a short "what happens to my data"; download buttons pointing at the latest release; and the two things a stranger will hit, said plainly: whether the Windows installer is signed (an unsigned one triggers a SmartScreen warning), and that there is no macOS build. Interest is measured without tracking anyone: release download counts, plus a cookie-free page counter at most.

**User value:** None for the current user — people who would want the application can find it, and the owner learns whether they exist before investing in anything paid.
**Done when:** The page is online in French and English with current screenshots and working download links; it states the licence, how data is handled, and the platform limits; download counts and page views can be read without any third-party tracking; the legal notices a site published from France needs are in place.
**Design:** none
**Open questions:**

- [ ] Ready when? (Recommended: after #025 and #026 have shipped.)
- [ ] A domain from the start, or the free GitHub Pages address first? (`folioneer.com` and `folioneer.app` are registered; they are the natural choice.)
- [ ] Is the Windows installer to be signed before strangers download it, and how? Today only the updater signature exists (it proves an update comes from the owner; SmartScreen ignores it) — no Windows code signing. The options, prices and eligibility to be checked again when the time comes: (a) SignPath Foundation — free, publisher shown as "SignPath Foundation", open-source licence and CI-built releases required; (b) a Certum open-source certificate — tens of euros a year, the owner's name shown, open-source project required, signing through their cloud service since keys must live on hardware; (c) Azure Trusted Signing — about ten dollars a month, eligibility of individuals depends on the country; (d) a standard certificate with cloud signing — a few hundred euros a year, no open-source condition; (e) no signing, and the page explains the warning. Even signed, the warning lasts until the certificate has earned reputation — which then carries over from one release to the next, whereas an unsigned installer starts from zero each time. (a) and (b) exist only while the licence is open source, which the AGPL-3.0-or-later is. (Recommended: (b) if the owner's name should show, (a) if free matters more.) Wiring any of them is one signing command in the Tauri configuration plus a credential in the release workflow.

## #041 — (ci) — A pull request that changes no code pays for the full E2E run

Every pull request runs the E2E suite: 13–15 minutes of Tauri build and WebDriver, measured over the runs of 2026-09-20. Entry #037 paid for four of them, about an hour, because each fix-up commit restarted it; the records-only pull requests #4 and #8 paid one each for changing nothing the suite can execute.

`.github/workflows/e2e.yml` explains the absence of a path filter, and the reason is sound: `paths-ignore` on a **required** check leaves it "Expected" for ever, so the pull request can never merge. The conclusion does not follow, though — the job can always start and decide inside itself. The required check then reports within a minute, and the twenty-minute build happens only when code moved. Pushes to `main` already carry the filter, so `main`'s screenshot artifact is refreshed by code pushes only and the visual baseline is unaffected.

Two traps for whoever takes this. The local classifier cannot be reused as it stands: `scripts/changed-scope.sh` answers `none` for `.github/*` and `scripts/*`, yet a change to `e2e.yml`, `wdio.conf.ts` or the capture helper is exactly when the suite must run — the skip condition has to be written for E2E rather than borrowed. And the job does more than run tests: it uploads the screenshot artifact, compares it against `main` and comments on the pull request, so a skipped run must leave those steps coherent rather than half-done.

**User value:** None — CI minutes, and a faster answer on a pull request that changes no code.
**Done when:** A pull request touching only Markdown, `docs/` or records reports the E2E check green in under a minute without building the application; a pull request touching `src/`, `src-tauri/`, `e2e/`, the workflow or the test tooling still runs the full suite; the screenshot comparison against `main` still runs on the pull requests that build; both paths are watched on a real pull request of each kind.
**Design:** none
**Open questions:** none

## #014 — (e2e) — Drive a second device in the E2E suite

The multi-device sync E2E covers the single-device critical path only (plan § Halt Artifact H1): `wdio.conf.ts` launches one binary with one `VAULT_COMPASS_E2E_DATA_DIR` and `maxInstances: 1`, so joining a folder another device created (SYN-014/036) is proven by the two-database integration test `src-tauri/tests/sync_two_devices.rs`, not through the UI. A real two-device E2E needs an `e2e/helpers/second_device.ts` that launches a second binary against its own data directory plus a wdio multi-remote configuration — a separate, pre-requisite task before any join scenario is written.

**User value:** None directly — test infrastructure.
**Done when:** A wdio multi-remote config and `e2e/helpers/second_device.ts` launch a second binary on its own data directory, and a join scenario (SYN-014/036) passes through the UI.
**Design:** none
**Open questions:** none

## #015 — (deps) — Upgrade specta / tauri-specta / specta-typescript past rc.22

Pinned at `specta 2.0.0-rc.22`, `tauri-specta 2.0.0-rc.21`, `specta-typescript 0.0.9`. The lockstep bump to rc.25 / rc.25 / 0.0.12 was attempted on 2026-09-12 and reverted: `specta-typescript 0.0.12` removed the global `Typescript::bigint(BigIntExportBehavior::Number)` switch this project relies on, and now refuses to export any unannotated `i64` / `u64` (`Error::bigint_forbidden`). The replacement is a per-field `#[specta(type = specta_typescript::Number)]` override (or a wrapper type) on every 64-bit integer that crosses the wire — which, under ADR-001, is every monetary amount, quantity, rate and percentage in every DTO and command signature. That is an annotation sweep across the whole wire surface, not a dependency bump, and a single missed field fails bindings generation.

**User value:** None — dependency currency.
**Done when:** every wire-visible 64-bit integer carries the `Number` override (or a shared newtype does), `just generate-types` produces a bindings diff that is cosmetic only, and the three crates sit on a current release together.
**Design:** none
**Open questions:** none

## #016 — (deps) — Accepted risk: WebdriverIO 9 transitive advisories (extract-zip)

`npm audit` reports 14 advisories (2 low, 12 high), all in `devDependencies`. The twelve highs share one root: `extract-zip`, affected in every published version and reached only through `@puppeteer/browsers 2.13.2`, which WebdriverIO 9.31 pins for downloading browser binaries the suite never uses (it drives the Tauri binary through tauri-driver). npm's only proposed fix is a downgrade to WebdriverIO 8.14.6. Checked at the 2026-09-19 release audit: `extract-zip` has no patched release and none is planned (2.0.1 is its last); `@puppeteer/browsers` 3.x replaced it with another extractor, but WebdriverIO 9.31.9 — the latest — still depends on `@puppeteer/browsers ^2.2.0`, so the only way out today is an npm override across a major version of a package the E2E runner calls. The two lows are mocha's bundled `diff`. The `deepmerge-ts` and `js-yaml` advisories were cleared by the in-range bump of 2026-09-12 (`ff986f1`). Nothing from these packages enters the application bundle or the Tauri binary, and CI's `npm audit --omit=dev` gate is green. Re-run `npm audit` at each release.

**User value:** None — devDependency advisories; nothing from them enters the shipped bundle.
**Done when:** WebdriverIO depends on `@puppeteer/browsers` 3.x or later (which no longer carries `extract-zip`), the bump is taken, `npm audit` is clean, and this entry is deleted.
**Design:** none
**Open questions:** none

## #017 — (deps) — Accepted risk: RUSTSEC-2023-0071 (rsa Marvin Attack)

`cargo audit` flags `rsa 0.9.10` (timing sidechannel, CVSS 5.9 medium) with no upstream fix. Pulled transitively via `sqlx-mysql 0.8.6` because the `sqlx` macro crate compiles all backends regardless of enabled features. We only enable `sqlite`, so the vulnerable RSA path is never reached at runtime. Re-evaluate when sqlx ships a fix or when we change DB backend.

**User value:** None — the vulnerable RSA path is unreachable in a SQLite-only build.
**Done when:** sqlx stops compiling `sqlx-mysql` for sqlite-only builds or `rsa` publishes a fix, `cargo audit` is clean, and this entry is deleted.
**Design:** none
**Open questions:** none
