# Business Rules — Price Movement (PMV)

## Context

A Global refresh currently ends in a snackbar carrying counts — how many prices were updated, how many assets were skipped (MKT-115, MKT-119). The user learns that the fetch ran, but not what the new prices did to the portfolio. Price Movement closes that gap: when the user runs a Global refresh, the application reports each account's value before and after the fetch, together with the movement attributable to the new prices alone.

The isolation matters. An account's value also moves when the user records a purchase, a sale or a deposit, and the same refresh rewrites currency rates (FXR-075), so a naive comparison of two stored totals would credit price movement with changes the prices did not cause. Every figure in this report is therefore computed over the same holdings, quantities and currency rates, so the only thing that differs between the two readings is the price of each asset.

This is a **feature spec** spanning the `account`, `asset` and `currency` bounded contexts. It reads the existing account valuation; it introduces no new stored record. All monetary values are micro-units per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md), and everything is recomputed rather than stored per [ADR-013](../adr/013-recompute-account-performance-on-read.md).

---

## Entity Definition

### PriceMovementReport

The outcome of one Global refresh, expressed as what the fetch did to the portfolio's value. It is produced when the fetch finishes, presented once, and discarded — nothing about it is persisted. It is a transient value object, like `AssetLookupResult`.

| Field                   | Business meaning                                                                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `rows`                  | One entry per account, in the report's own order (PMV-033).                                                                                                              |
| `total_before`          | The portfolio's value before the fetch, in the reference currency.                                                                                                       |
| `total_after`           | The portfolio's value after the fetch, in the reference currency.                                                                                                        |
| `total_currency`        | The cross-account reference currency the totals and the portfolio movement amount are expressed in.                                                                      |
| `total_movement_pct`    | The portfolio's movement as a proportion of `total_before`; absent when undefined or unmoved (PMV-044, PMV-045).                                                         |
| `total_movement_amount` | The portfolio's movement as an amount in the cross-account reference currency, `total_after` minus `total_before`; absent when unmoved (PMV-045, PMV-046).               |
| `observed_from`         | The observation date the portfolio carried before this fetch; absent when nothing was priced before (PMV-052).                                                           |
| `observed_to`           | The observation date this fetch produced; absent only when it produced none later than `observed_from` — carried even when `observed_from` is absent (PMV-051, PMV-052). |
| `incomplete`            | Whether any entry's reading is incomplete (PMV-043).                                                                                                                     |

> Asset counts are not carried here — the completion signal's own updated / skipped counts (MKT-119) already state them for the same scope.

### PriceMovementRow

One account's share of the report.

| Field             | Business meaning                                                                                                     |
| ----------------- | -------------------------------------------------------------------------------------------------------------------- |
| `account_id`      | Which account this entry describes.                                                                                  |
| `name`            | The account's display name.                                                                                          |
| `currency`        | The account's own currency — the currency this entry's values and amount are expressed in.                           |
| `before`          | The account's value before the fetch.                                                                                |
| `after`           | The account's value after the fetch.                                                                                 |
| `movement_pct`    | The movement as a proportion of `before`; absent when undefined or unmoved (PMV-025, PMV-031).                       |
| `movement_amount` | The movement as an amount in the account's currency, `after` minus `before`; absent when unmoved (PMV-027, PMV-031). |
| `incomplete`      | Whether at least one of the account's holdings could not be read at its current price (PMV-032).                     |

---

## Business Rules

> **Population.** Throughout PMV, _the account's holdings_ means every active holding of the account. _In scope_ means the subset the fetch actually attempts, which excludes system cash (MKT-116) and refresh-locked holdings (MKT-151). PMV-050 and PMV-052 turn on the latter; PMV-032 turns on the former, but treats both of those exclusions as expected rather than as incompleteness.

### Production (010–019)

**PMV-010 — Only the Global refresh reports movement (frontend + backend)**: A Global refresh produces a price-movement report. The Account refresh, the Auto-fetch on launch and the Scheduled fetch produce none; their existing feedback is unchanged. The Scheduled fetch runs with no window at all (SPF-020), so it has no surface on which a report could appear.

**PMV-011 — One report per refresh (backend)**: A refresh produces exactly one report, when the fetch has finished attempting every asset in its scope.

**PMV-012 — A refresh that never ran reports nothing (backend)**: When the refresh is rejected before any asset is attempted — another fetch is already running (MKT-113), or the scope holds no fetchable asset (MKT-111) — the fetch never reaches the completion it would report on, so no report exists and the existing rejection feedback applies unchanged.

**PMV-013 — The report is presented as a surface of its own (frontend)**: The report appears on the accounts list — the surface carrying the Global refresh action — as a dialog the user reads and closes, presented over the account rows rather than docked among them. It reports on a refresh that has already finished, so it asks nothing of the user beyond closing it.

**PMV-014 — Reporting never changes what the fetch does (backend)**: Prices are recorded exactly as they would be without this feature. If the report cannot be produced, the fetch still succeeds and the user receives the pre-existing completion feedback.

**PMV-015 — A portfolio-wide fetch states which action started it (frontend + backend)**: The Global refresh and the Auto-fetch on launch are the same fetch over the same scope (MKT-130), so that fetch carries which of the two started it. Only the Global refresh reports movement (PMV-010), and without that distinction the fetch cannot tell which it is serving.

**PMV-016 — The report belongs to the surface that asked for it (frontend)**: The report is presented only if the user is still on the accounts list when the refresh completes. A Global refresh is acknowledged immediately and runs on under a progress indicator visible elsewhere (MKT-180), so the user may have navigated away; in that case no report is presented and none is kept for their return. Running another Global refresh produces a fresh one.

**PMV-017 — The report states that its figures are a frozen comparison (frontend)**: The report opens over account rows and, when shown, the portfolio total row (ACC-030) showing live values, and its own figures deliberately differ from them (PMV-020, PMV-023). The report therefore presents its two readings as a dated before-and-after of this refresh, never as the account's current value, so the two sets of numbers are not read as disagreeing.

**PMV-018 — The report waits for a refresh that still needs the user (frontend)**: The same refresh may leave assets unpriced, which opens the unupdated-prices modal (MKT-172) to ask the user for those prices. The report is presented only once that modal has closed. The modal asks the user for something and the report only tells them something, so the report never covers it, and the modal's own behaviour is unchanged. The report is not discarded while it waits — it is presented in full when the modal clears, subject to PMV-016.

### The comparison (020–029)

**PMV-020 — Movement is measured at constant holdings and constant rates (backend)**: Both readings are computed over the same holdings, the same quantities and the same currency rates — those in force when the refresh started, for every conversion the readings need (holding to account currency, and account to reference currency). Only the asset prices differ between the two readings. Because the same refresh also fetches currency rates (FXR-075), the later reading deliberately ignores any rate the refresh obtained; a currency move is not a price move.

**PMV-021 — "Before" uses the prices in force when the refresh started (backend)**: The earlier reading values each holding at the price the portfolio carried at the moment the refresh began.

**PMV-022 — "After" uses the prices in force when the refresh finished (backend)**: The later reading values each holding at the price **this refresh** left in place once every asset has been attempted — the new price where this refresh obtained one, the previous price where it did not. It is not a re-read of whatever the portfolio happens to carry at that moment (PMV-026).

**PMV-023 — Both readings use the application's own valuation (backend)**: Each reading computes an account's value the same way the rest of the application computes its Global Value, so the report never introduces a second valuation. It is not a restatement of what the accounts list shows at that moment: the list reflects live holdings and freshly fetched rates, while the readings deliberately hold both constant (PMV-020).

**PMV-024 — Movement is expressed as a proportion of the earlier value (frontend + backend)**: An entry's movement is the change from its earlier value to its later value, relative to the earlier value. The application decides whether a proportion exists; the presentation only renders it.

**PMV-025 — The proportion is undefined without a positive earlier value (backend)**: When an account's earlier value is zero or negative, no proportion is reported for it; its two values are still shown.

**PMV-026 — Only this refresh's prices are compared (backend)**: A price written while the refresh runs by anything other than this refresh — a Scheduled fetch, which may run concurrently (SPF-023), a manual entry, or another device's changes arriving through sync — is excluded from the later reading. The report describes what this refresh did, never what merely happened during it.

**PMV-027 — An entry's movement is also stated as an amount (frontend + backend)**: An entry's movement is also reported as the amount its value changed: the later value minus the earlier value, signed, in the account's own currency (PMV-034). The amount needs no positive earlier value, so it is reported even where PMV-025 withholds the proportion. The application computes the amount; the presentation only renders it.

**PMV-028 — A movement is presented as a gain or a loss by its sign (frontend)**: Every movement figure the report carries, an entry's or the total's, amount or proportion, is presented as a gain when positive and as a loss when negative. A figure the report does not carry, or that is zero, is presented as neither.

### Per-account entries (030–039)

**PMV-030 — Every account is listed (backend)**: The report carries one entry per account, including accounts whose value did not move and accounts holding no priced asset. The user can therefore tell "did not move" apart from "was not looked at".

**PMV-031 — An unmoved account carries no movement figure (frontend + backend)**: When an account's two values are equal, neither a proportion nor an amount is reported for it, and the entry is presented without movement figures rather than as a zero movement. Both values are still carried, so an unmoved entry stays distinguishable from one whose proportion is undefined (PMV-025).

**PMV-032 — An incomplete reading is marked (frontend + backend)**: An entry is marked incomplete when at least one of the account's holdings could not be read at its current price _although it was meant to be_. Two conditions do so, and whether the holding has ever had a price recorded never changes the answer: the fetch attempted the holding and could not price it (the full skip set of MKT-171, which includes a holding whose ticker cannot be resolved), or the holding contributes zero to both readings because no usable rate exists (FXR-034). Two conditions do not, because in each the holding was never going to move and the user knows it: a system cash holding, which the fetch never has in scope (MKT-116), and a holding whose automatic refresh the user locked, whose stale price is the point of the lock (MKT-151, MKT-158) — flagging it would raise a marker on every refresh for as long as the lock stands, teaching the user to ignore the one marker that matters. An incomplete entry's movement understates what the prices actually did.

**PMV-033 — Entries are ordered by account name (backend)**: The report lists accounts by name in ascending order — the accounts list's own default (ACC-007) — rather than following the user's current table sort, so the report reads the same way every time.

**PMV-034 — Each entry is expressed in its account's own currency (frontend + backend)**: An entry's values and its movement amount are stated in the currency of the account they describe, never converted.

### Portfolio total (040–049)

**PMV-040 — The report carries a portfolio total in the reference currency (backend)**: Both readings are also reported for the portfolio as a whole, converted to the cross-account reference currency the application already uses for aggregation (GPF-011). The report states that currency rather than leaving it to be assumed.

**PMV-041 — The total's movement is computed from the totals (backend)**: The portfolio movement is derived from the two converted totals, never by combining the per-account proportions.

**PMV-042 — An unconvertible account contributes nothing to the total (backend)**: An account whose currency cannot be converted as of the reading contributes zero to both totals, matching how the application already degrades a missing rate for account-level conversion (GPF, extending FXR-034). Its own entry is unaffected, and it marks the entry incomplete per PMV-032 so the understated total is never silent.

**PMV-043 — An incomplete total is marked (frontend + backend)**: When any entry's reading is incomplete (PMV-032), the portfolio total is marked incomplete too.

**PMV-044 — The portfolio proportion is undefined without a positive earlier total (backend)**: When the earlier total is zero or negative — an empty portfolio, or every account unconvertible — no portfolio proportion is reported; both totals are still shown. This mirrors PMV-025 at portfolio level.

**PMV-045 — An unmoved portfolio total carries no movement figure (frontend + backend)**: When the two totals are equal, neither a portfolio proportion nor a portfolio amount is reported, even though individual accounts may have moved in opposite directions that cancel out, and the total is presented without movement figures rather than as a zero movement. This mirrors PMV-031 at portfolio level; the per-account entries still carry their own movements.

**PMV-046 — The portfolio movement is also stated as an amount (frontend + backend)**: The portfolio total also carries its movement as an amount: the later total minus the earlier total, signed, in the cross-account reference currency, taken from the two totals like the proportion (PMV-041). It needs no positive earlier total, so it is reported even where PMV-044 withholds the portfolio proportion. The application computes the amount; the presentation only renders it.

### Observation dates (050–059)

**PMV-050 — The report states the two observation dates (frontend + backend)**: The report carries the observation date the portfolio carried before the refresh and the one the refresh produced, so the user reads the movement as being between two dated points. Holdings do not all carry the same observation date — a venue closed on Friday and one closed on Tuesday disagree — so each date is the most recent observation date across every holding in scope at that moment, read as "since prices last moved" rather than as a date every holding shares.

**PMV-051 — A refresh that produced no later date shows one date (frontend + backend)**: When the refresh obtained no observation date later than the one the portfolio already carried, the report presents that single date instead of two identical ones. This says nothing about whether values moved: a refresh can overwrite a price at the same observation date (MKT-025), so prices may well have changed. Whether anything moved is decided by the values alone (PMV-060).

**PMV-052 — A portfolio with no prior price states no earlier date (frontend + backend)**: When no holding in scope carried any recorded price before the refresh, the report carries no earlier observation date. If that refresh then produced dated prices, the later date is still stated — the comparison reads as running from nothing priced to that date. Only when the refresh produced no dated price either does the report carry no date at all.

**PMV-053 — Each value column states its own date (frontend)**: When the report shows its table, each value column is headed by its own name and the date of its side, in the date format of the application language: the earlier date over the earlier value, the later date over the later value. A side for which the report carries no date (PMV-051, PMV-052) states its name and no date, never the other side's date.

### Outcomes with nothing to show (060–069)

**PMV-060 — A refresh that moved nothing reports that plainly (frontend + backend)**: When no account's value changed between the two readings, the report says so in plain words rather than presenting a table of identical figures. This is decided by the values, never by the dates. When such a refresh also left entries incomplete (PMV-032), the report still says that some holdings could not be read at their current price, so "nothing moved" is never mistaken for "nothing was tried". It states no count of its own: the number of assets the fetch could not price is the completion signal's own skipped count (MKT-119), a different population from PMV-032's incompleteness, and the two are never presented as if they were the same number.

**PMV-061 — The report is dismissed and not kept (frontend)**: The user dismisses the report; it is not retained, survives no restart, and the application offers no way to bring it back. A user who wants to see the movement again refreshes again.

---

## Workflow

```
[User runs a Global refresh from the accounts list]
        │
        ▼
[Portfolio valued — "before" reading]      ← holdings, quantities and rates frozen here
        │
        ▼
[Fetch attempts every asset in scope]      ← unchanged behaviour (MKT)
        │
        ▼
[Portfolio valued — "after" reading]       ← same holdings, same rates, new prices only
        │
        ▼
[Report produced: per-account entries + portfolio total + the observation dates]
        │
        ▼
[Report dialog opens on the accounts list — user reads it and closes it]
        │                              (only if the user is still there — PMV-016)
        │
        └── skipped assets? the unupdated-prices modal (MKT-172) comes first;
                            the report opens once it is closed (PMV-018)
```

---

## UX Draft

### Entry Point

None of its own. The report appears on the accounts list when a Global refresh the user started completes, provided the user is still on that surface (PMV-016).

### Main Component

A dialog on the accounts list, closed by the user and never shown again. A table of accounts — name, earlier value, later value, movement amount, proportion — closed by a portfolio total row. Each value column is headed by its name and its own observation date, with a dash where that side has no date (PMV-053): "Before (09/09/2026)", "After (15/09/2026)" in French, so the dates are not repeated on every entry.

### States

- **Nothing moved**: no table; a plain statement that no account's value changed, and — when any entry is incomplete — that some holdings could not be read at their current price (PMV-060).
- **Moved**: the table, with unmoved accounts present but showing no movement figure, and an account without a positive earlier value showing its movement amount but no proportion.
- **Partially complete**: the table, with the affected entries and the total marked as incomplete. No count of its own (PMV-060); the fetch's own skipped count reaches the user through the existing completion feedback (MKT-119/145).
- **One side dated**: the dated column shows its date, the other its name with a dash (PMV-051, PMV-052).
- **Undated**: both value columns show their name with a dash (PMV-052).
- **Error**: no state of its own — a failure to produce the report leaves the pre-existing completion feedback in place (PMV-014).

### User Flow

1. The user runs a Global refresh from the accounts list.
2. The refresh runs as it does today, with its existing progress feedback.
3. If the refresh skipped assets, the unupdated-prices modal opens first and behaves exactly as it does today.
4. The report opens once nothing else is asking for the user, listing every account with its two values and its movement.
5. The user reads the report and closes it.

---

## Open Questions

None — all questions have been resolved.
