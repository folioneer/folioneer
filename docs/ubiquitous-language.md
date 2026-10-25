# Ubiquitous Language

The authoritative dictionary of domain terms for this project.

> ⚠️ **Every addition or change to this document MUST be individually reviewed and validated
> by the user before it is considered confirmed.** Agents may propose additions (using status
> `confirmed`) but MUST NOT mark any term or entry as `confirmed` without explicit user approval.

**Rules:**

- All terms MUST be agreed with the user before use in code, tests, or docs.
- The agent MUST NOT invent or assume domain terms — propose and wait for confirmation.
- Terms marked `confirmed` are proposals awaiting user validation.
- Once confirmed, the term MUST be used consistently everywhere (code, comments, specs, UI).

---

## Account Context

### Account

The top-level aggregate root. Represents a financial account (e.g. brokerage, savings).
Owns all holdings and their transaction history.

> Status: confirmed

### Portfolio's country

The country of residence a portfolio is kept in, which an account without a country of its own takes (ACC-041): the account form suggests that country's wrappers first. France until the first run asks it (TODO-092). In French: "pays du portefeuille" (owner, 2026-10-17).

> Status: confirmed

### Wrapper

The tax wrapper an account sits in — a PEA, an assurance vie, a Depot: a label from the lists (ACC-040): a wrapper belongs to a country's list or to the generic one, and any account may hold any of them — holding one of another country is the user's choice (owner, 2026-10-18) (ACC-043). No tax is computed from it and no rule enforced. Code: `wrapper` (an identifier of the lists), with the account's `country`. French: "enveloppe" ("enveloppe fiscale" in full) (owner, 2026-10-18).

> Status: confirmed

### Holding

An internal entity of `Account`. Represents the current position in a given asset within
an account — quantity held, average price (VWAP), and realized P&L.

> Status: confirmed

### Transaction

An internal entity of `Account`. A single financial event — a purchase or a sale — with
date, quantity, price, fees, and exchange rate. Owned directly by `Account` alongside
`Holding`; a transaction affects its corresponding holding but is not nested inside it.

> Status: confirmed

### Account journal

Every transaction of an account in date order — same-day ones in the order they were
entered — each with the cash it took out or brought in and the account's cash balance
after it.

> Status: confirmed

### Stock Split

A corporate action that rescales a held position without changing its value: the share
count is multiplied by the split factor and the average price divides accordingly, the
cost basis staying intact. The position remains the same asset — a split (or an issuer
rename) never creates a new one. A reverse split uses a factor below one.

> Status: confirmed

### Write-off

A quantity that leaves a holding without proceeds — a network fee paid in the asset, dust,
coins sent away or lost — taken out at its weighted-average cost, moving no cash; its
realized result is the cost taken out, negated, and a write-off whose reason is Lost is
also counted apart (WOF). Its reason is one of `NetworkFee`, `Dust`, `Lost`, `Other`. In French: "Sortie sans contrepartie" (owner, 2026-10-19).

> Status: confirmed

### Transfer

A holding's quantity moved from one account to another of the same portfolio, at its cost: no cash moves and no result is realized; between currencies the cost is converted at the transfer's rate (TRF-020, TRF-022). It is stored as its parts — a transfer out, a transfer in and, when less arrives than left, a fee in kind — sharing one `transfer_id`, and is recorded, corrected and cancelled as one (TRF). In French: « Transfert » (owner, 2026-10-24).

> Status: confirmed

### Transfer out

The part of a transfer in the sending account: the quantity received leaving the sending holding at its cost (TRF-020). Code: `TransferOut`. In French: « Transfert sortant » (owner, 2026-10-24).

> Status: confirmed

### Transfer in

The part of a transfer in the receiving account: the quantity received entering at the cost the transfer out took, converted at the rate (TRF-020, TRF-022). Code: `TransferIn`. In French: « Transfert entrant » (owner, 2026-10-24).

> Status: confirmed

### Fee in kind

The difference between the quantity sent and the quantity received by a transfer: a write-off of reason `NetworkFee` in the sending account, part of the transfer (TRF-021). In French: « Frais en nature » (owner, 2026-10-24).

> Status: confirmed

### Reported split

A split the price provider reports for an asset, with its date and its split factor (new :
old), kept and synced as an observation (MKT-230). It changes nothing by itself: only a
recorded Stock Split rescales a position. In French: "Division signalée" (owner,
2026-10-19).

> Status: confirmed

### Unrecorded split

A reported split that an account holding the asset on the day before it has not recorded
as a Stock Split within 5 days of that date (MKT-231); the holding row and an agent say so. In French:
"Division non enregistrée" (owner, 2026-10-19).

> Status: confirmed

### Holding Note

A free-text reminder the user pins to a position they hold, optionally carrying a price
alarm — a share-price threshold and direction. The alarm is live: it rings while the
current price is across the threshold and re-arms by itself when the price comes back.
Distinct from a transaction's note.

> Status: confirmed

---

## Asset Context

### Asset

The aggregate root of the asset context. Represents a financial instrument
(stock, ETF, bond, etc.) with a currency, class, category, and risk level.

> Status: confirmed

### Interest-Bearing

An asset attribute marking a line as a valid interest target — a cash-like
fund line ("fonds en euros") that capitalizes interest. Only interest-bearing
assets can receive an `Interest` credit; the Cash Asset is always
interest-eligible regardless of this attribute.

> Status: confirmed

### AssetPrice

An internal entity of `Asset`. A price observation for an asset on a given date, with a `source` field (see `AssetPriceSource`) qualifying its provenance.

> Status: confirmed

### Observation date

The date a price is _for_ — the trading day its value reflects — as distinct from when the price was fetched or recorded. An auto-fetched price uses the provider's reported date (e.g. Friday's close still seen on a Sunday); a manually entered price uses the date the user picks.

> Status: confirmed

### Price freshness

How recent the portfolio's prices are, told by two figures. The **newest price date** is the latest observation date among the prices of the assets currently held, whichever device recorded it. The **last fetch on this device** is the moment this device itself last fetched at least one price, in the app or by a scheduled download; it is kept per device and does not travel with the sync.

> Status: confirmed

### Outdated price

A held asset's price older than its kind and class allow (MKT-205): never for a custom asset, cash or real estate; otherwise more than 10 business days for a fund, 5 for any other listed or crypto asset. Judged the same way whatever recorded the price. The account and the portfolio count them. Code: `price_outdated`, `outdated_price_count`. French: "prix ancien" (owner, 2026-10-18).

> Status: confirmed

### Price typed by hand

An asset whose price the owner types and no fetch replaces: every fetch task leaves it (MKT-151, SPF-040) and the backfill refuses it (MKT-196), for an asset whose price cannot be fetched. Code: `price_refresh_blocked` (owner, 2026-10-18). Never called a "lock". French: "prix saisi à la main".

> Status: confirmed

### AssetPriceSource

A value-object enum qualifying the provenance of an `AssetPrice` record. Variants: `Manual` (user-entered via manual entry or transaction auto-record), `YahooFinance` (written by a fetch through a Yahoo Finance provider, as in the owner's private build) and `Agent` (recorded by an agent the owner allowed — the Mark of a price; owner, 2026-10-10). Metadata for traceability per ADR-012 — does not influence read/write precedence (latest-write-wins).

> Status: confirmed

### Exchange

A canonical reference to a trading venue, independent of any market-data provider. Carries an ISO 10383 Market Identifier Code (MIC) as `code` (e.g. `XPAR`, `XNAS`) and a human-readable `label`. Optional field on `Asset`. Auto-filled by the OpenFIGI lookup path (WEB-049) or selected by the user via a curated picker on the Add/Edit Asset form (AST-021). Used by the auto-fetch task to resolve the Yahoo Finance provider symbol (MKT-110). Provider symbols (Yahoo venue suffixes, OpenFIGI exchange codes) are NOT stored on `Exchange` — they are resolved by per-provider mappers at the boundary.

> Status: confirmed

### Listing

> Status: confirmed

One of several listed assets that share an ISIN: the same instrument on another exchange, or in another currency. Each listing is an asset of its own, with its own reference, prices and positions; nothing of two listings is merged. The assets table says of each which other listings exist ("also held as"). In French: "Cotation".

### Kind

How an asset is identified and priced, chosen when it is created. The class says what the asset is economically; the kind says what makes it this asset and no other, and where its price comes from. In French: "Nature".

| Kind       | Meaning                                                                                         | In French    |
| ---------- | ----------------------------------------------------------------------------------------------- | ------------ |
| **Listed** | An instrument on an exchange, in a currency. It has an ISIN; its reference is its ticker there. | "Coté"       |
| **Crypto** | A crypto asset, known by its symbol. It is bought and sold like any asset; it is not money.     | "Crypto"     |
| **Custom** | What no market lists — real estate, a fund inside a contract. Its price is typed.               | "Non coté"   |
| **Cash**   | The application's own money line, one per currency. Never created or edited by the user.        | "Liquidités" |

Two assets are the same when they are of one kind and share what identifies it: ISIN, exchange and currency for listed assets; the symbol for crypto assets; the reference for custom ones. The same instrument on two exchanges is two assets.

> Status: confirmed

### Asset to settle

An asset that breaks a rule of its kind, or is the same as another, because it existed before kinds did or arrived from another device. The application leaves it as it is and lists it; the user settles it by editing it. In French: "Actif à régulariser".

> Status: confirmed

---

## Aggregate Root Methods (Account)

| Name                    | Intent                                                                                                  | Status    |
| ----------------------- | ------------------------------------------------------------------------------------------------------- | --------- |
| `buy_holding`           | Record a purchase of an asset into the account                                                          | confirmed |
| `sell_holding`          | Record a sale of an asset from the account                                                              | confirmed |
| `correct_transaction`   | Correct the fields of an existing transaction (cascades VWAP/P&L recalculation on the affected holding) | confirmed |
| `cancel_transaction`    | Delete an existing transaction (cascades VWAP/P&L recalculation or holding removal)                     | confirmed |
| `open_holding`          | Seed an existing position with a quantity and total cost at a given date, without full purchase history | confirmed |
| `place_transfer_parts`  | Put a transfer's parts in this account, replayed and checked (TRF-020, TRF-030)                         | confirmed |
| `remove_transfer_parts` | Take a transfer's parts out of this account (TRF-040)                                                   | confirmed |
| `restate_transfers_out` | Re-read the cost of this account's transfers out after an earlier change (TRF-025)                      | confirmed |
| `recost_transfer_in`    | Set a transfer in to the cost its transfer out now takes (TRF-025)                                      | confirmed |
| `transfer_parts`        | The parts of a transfer this account holds (TRF entity)                                                 | confirmed |

## Transaction Types

| Name             | Intent                                                                                                                   | Status    |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------ | --------- |
| `Purchase`       | A regular buy transaction — quantity, unit price, exchange rate, fees                                                    | confirmed |
| `Sell`           | A regular sell transaction — quantity, unit price, exchange rate, fees, realized P&L                                     | confirmed |
| `OpeningBalance` | A position seed entry — quantity and total cost paid directly, no fee breakdown                                          | confirmed |
| `Deposit`        | A cash inflow from outside the application's tracked world (CSH-022)                                                     | confirmed |
| `Withdrawal`     | A cash outflow to outside the application's tracked world (CSH-032)                                                      | confirmed |
| `Dividend`       | Cash income paid by a held asset; credits cash, attributed to the paying asset (DIV-023)                                 | confirmed |
| `FreeShares`     | Shares of a held asset received at no cost (bonus issue); quantity rises, cost basis unchanged (FSD-022)                 | confirmed |
| `ManagementFee`  | A fee paid by removing shares of a held asset; quantity falls, cost basis unchanged, no cash (FEE-022)                   | confirmed |
| `Interest`       | Capitalized interest credited at no cost to a fund line or the cash line; quantity rises, cost basis unchanged (INT-024) | confirmed |
| `WriteOff`       | A quantity of a held asset leaving without proceeds; quantity and cost basis fall at cost, no cash (WOF-020)             | confirmed |
| `TransferOut`    | The sending part of a transfer; quantity and cost basis fall at cost, no cash (TRF-020)                                  | confirmed |
| `TransferIn`     | The receiving part of a transfer; quantity enters at the cost moved, converted at the rate, no cash (TRF-020, TRF-022)   | confirmed |

### Total-Entry Mode

> Status: confirmed

A way of recording a Purchase or Sell from the all-in broker total (fees included, account currency) instead of the unit price. The typed total is stored verbatim as the transaction's `total_amount` and the unit price is derived from it (TRX-060, SEL-050).

### Trade

> Status: confirmed

A purchase or a sale of an asset. Cash is never traded: it moves by a Deposit or a Withdrawal.

### Transaction draft

> Status: confirmed

A transaction as the user is still entering it: a purchase or a sale with its account, asset, date, quantity and either a unit price or a total; an opening balance with its account, asset, date, quantity and total cost; a dividend being corrected, with its date, amount and exchange rate; a split with its account, asset, date and size. The application checks it before it can be recorded and shows its first problem, or what recording would store and what the user should know about it.

## Cash Domain Concepts (introduced by CSH spec)

### Cash Asset

> Status: confirmed

A system-seeded `Asset` of `class = AssetClass::Cash`, one per ISO currency, with deterministic id `system-cash-{ccy}`. Acts as the asset reference for cash positions. Not user-editable, not user-creatable; shown in the asset catalog as the application's, with no action.

### Cash Holding

> Status: confirmed

A `Holding` whose asset is a Cash Asset. Represents the cash balance held in the account in the account currency. Exactly one Cash Holding per account, created together with the account at a zero balance and kept for the account's lifetime — when the account holds no cash it stays at zero rather than disappearing.

### Global Value

> Status: confirmed

The full economic value of an account: cash balance + Σ (market value of non-cash active holdings). Surfaced as `AccountDetailsResponse.total_global_value` (CSH-094). Used as the canonical "what is this account worth right now?" metric across the Account Details header and (later) the portfolio dashboard.

## Dividend Domain Concepts (introduced by DIV spec)

### Current value

> Status: confirmed

What a holding is worth at its latest recorded price, in the asset's own currency: price × quantity, with no exchange rate involved. Shown in the Current Value column of an account's holdings; none when no price has been recorded.

### Capital brought in

> Status: confirmed

What the owner brought into an account, cash and positions together: deposits less withdrawals (the Net Cash Input), plus every opening balance at its recorded cost. An account opened with positions costing 50 000 and a deposit of 500 brings in 50 500, of which 500 is Net Cash Input (ACD-058; owner, 2026-10-18). Code: `total_capital_brought_in`. French: "Capital apporté".

### Weight

> Status: confirmed

The share of an account's Global Value that one holding represents, as a percentage — a holding worth 220 in an account worth 400 weighs 55 %. The cash line has a weight too; a holding without a market value has none.

### Dividends Received

> Status: confirmed

Cumulative cash dividend income attributed to a holding, in the account currency. Per-holding (`HoldingDetail.dividends_received`, Σ of `Dividend` total_amount for the `(account, asset)` pair, DIV-070) and per-account (`AccountDetailsResponse.total_dividends_received`, Σ across all dividends, DIV-073). Reported separately from realized P&L — a dividend is income, not a capital gain.

### Total Return

> Status: confirmed

A holding's combined return from price appreciation and dividend income, expressed as a percentage of cost basis: `(unrealized_pnl + dividends_received) / cost_basis`. Surfaced as `HoldingDetail.total_return_pct` (DIV-071); `null` under the same conditions as `performance_pct` (no recorded price, or zero cost basis — MKT-034/035).

---

## Currency Context (introduced by FXR spec)

### CurrencyPair

A directed currency pair the system follows, e.g. USD → EUR: to value a holding whose currency differs from its account's, and to bring an account into the reference currency (FXR-013, FXR-015). Created when first needed and kept thereafter (owner, 2026-10-18).

> Status: confirmed

### CurrencyRate

What one unit of a currency is worth in another on a given day. Used to convert a foreign holding's market value into the account's currency. Distinct from a transaction's `exchange rate`, which is fixed at trade time for cost basis — a currency rate is current and changes over time.

> Status: confirmed

### Rate refresh

Recording the latest published exchange rate of every followed currency pair. It happens by itself at every launch, whether or not the application fetches prices, and in the scheduled fetch; "Update rates" is the one the user starts.

> Status: confirmed

### Update rates

The action in the Currency Rates view that records one exchange rate per day, for every followed pair, from the first transaction up to today — so past values can be converted as well as current ones.

> Status: confirmed

### CurrencyRateSource

Where a currency rate came from: `Manual` (entered by the user), or `Frankfurter` / `Ecb` (fetched from a provider). Informational only — it does not change which rate applies.

> Status: confirmed

---

### Account currency

The currency an account is kept in, chosen when the account is created: its holdings, cash and figures are valued in it. Distinct from the portfolio's "reference currency". In French: "devise du compte" (owner, 2026-10-17).

> Status: confirmed

### Portfolio total

The total row of the accounts list: every account's global value and unrealized P&L converted to the reference currency and summed (ACC-027 to ACC-030), marked incomplete — shown as "partial" — when an account cannot be converted (ACC-028). In French: "total du portefeuille" (owner, 2026-10-18).

> Status: confirmed

### Reference currency

The currency every figure spanning accounts is reported in — the portfolio total, the global performance, the price movement total (GPF-011): a setting of the portfolio, the same on every device, EUR until one is chosen (FXR-120 to FXR-125). Distinct from an account's own currency, the "account currency". In French: "devise de référence" (owner, 2026-10-17).

> Status: confirmed

## Connection Context (introduced by KEY spec)

---

## Domain Events

The `…Updated` events are raised by changes made on this device. Changes applied from the user's other devices are announced together, by `SyncCompleted`.

| Name                       | Raised by              | Intent                                                                                                                                                       | Status    |
| -------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------- |
| `AccountUpdated`           | Account BC             | Any state change in the account or its holdings                                                                                                              | confirmed |
| `TransactionUpdated`       | Account BC             | A holding or transaction was created, updated, or cancelled                                                                                                  | confirmed |
| `AssetUpdated`             | Asset BC               | Any state change in an asset or category                                                                                                                     | confirmed |
| `CategoryUpdated`          | Asset BC               | Any state change in a category                                                                                                                               | confirmed |
| `AssetPriceUpdated`        | Asset BC               | An AssetPrice record was created, updated, or deleted                                                                                                        | confirmed |
| `AssetPriceFetchCompleted` | Asset price fetch task | A fetch task finished; carries `ok` / `skipped` counts plus the list of assets it could not price so the UI can summarize the outcome and offer manual entry | confirmed |
| `CurrencyRateUpdated`      | Currency BC            | A currency rate was recorded, updated, or deleted                                                                                                            | confirmed |
| `CurrencyPairUpdated`      | Currency BC            | A currency pair was declared on this device                                                                                                                  | confirmed |
| `FeeScheduleUpdated`       | Account BC             | A fee schedule was created, changed, paused, resumed or deleted                                                                                              | confirmed |
| `HoldingNoteUpdated`       | Account BC             | A holding note was written or deleted on this device                                                                                                         | confirmed |
| `SyncCompleted`            | Sync BC                | A sync applied changes from the user's other devices, or changed this device's sync failures or paused state; carries nothing                                | confirmed |
| `AgentConnectionChanged`   | Agent connection       | An agent asked to connect, was answered, connected, called a tool or left, or the owner switched the setting; carries nothing                                | confirmed |

---

## Asset Web Lookup

### OpenFIGI Lookup

The outbound HTTP search that, given a name, ticker, or ISIN, queries the OpenFIGI API and
returns up to 10 candidate `AssetLookupResult` values. 12-character alphanumeric inputs route
to the ISIN mapping endpoint; all others route to the keyword search endpoint.

> Status: confirmed

### AssetLookupResult

A transient value object returned by the OpenFIGI lookup. Never persisted. Carries the name,
reference (ISIN or ticker), currency, and asset class of a candidate instrument — used solely
to pre-fill the Add Asset form.

> Status: confirmed

## Asset Web Lookup Command

| Name           | Intent                                                                                                             | Status    |
| -------------- | ------------------------------------------------------------------------------------------------------------------ | --------- |
| `lookup_asset` | Query OpenFIGI with a name, ticker, or ISIN and return up to 10 `AssetLookupResult` values. Errors: `NetworkError` | confirmed |

---

## Management Fee Domain Concepts (introduced by FEE spec)

### Management Fee Deduction

> Status: confirmed

A fee charged by a fund/ETF (TER / ongoing charges) paid not in cash but by removing shares from the holding. Recorded as a `ManagementFee` transaction: quantity falls, cost basis is unchanged (so the average price rises), no cash moves, no realized P&L. Generated by a Fee Schedule, or entered as a one-off — either as a percentage of the holding or as its Resulting Quantity.

### Resulting Quantity

> Status: confirmed

The quantity a holding should hold once a one-off Management Fee Deduction has been applied, as of the fee's date. The deduction removes the difference between the quantity held on that date and the resulting quantity, exactly — the way to align a holding with the real position.

### Fee Schedule

> Status: confirmed

A recurring rule, one per `(account, asset)`, that generates Management Fee Deductions at a fixed cadence (monthly / quarterly / annually) from an annual percentage rate. Deductions are materialized lazily on app open, catching up every completed period since the last one applied.

### Management Fees

> Status: confirmed

The cumulative cost of Management Fee Deductions attributed to a holding, in the account currency — each deduction valued at the market price as of its own date. Per-holding and per-account figures shown on Account Details. Named to distinguish it from the brokerage-commission `fees` field on a `Transaction`.

---

## Asset Price Fetch Tasks

### Fetch task

A backend job that retrieves current prices from an External provider and upserts `AssetPrice` records. Umbrella term for the three named instances below.

> Status: confirmed

### Quote

A single price reading returned by a price provider during a fetch: the market price together with its observation date. Transient — it becomes an `AssetPrice` before being stored.

> Status: confirmed

### Auto-fetch

A fetch task triggered automatically at application launch when the user has enabled the auto-fetch setting. Scope: all active holdings across all accounts.

> Status: confirmed

### Global refresh

A fetch task triggered manually by the user from the global dashboard. Scope: all active holdings across all accounts. Shares the same backend entry point as auto-fetch.

> Status: confirmed

### Account refresh

A fetch task triggered manually by the user from an account detail page. Scope: active holdings of the specified account.

> Status: confirmed

### Scheduled fetch

The once-daily automatic download of closing prices and exchange rates, launched by the operating system at the configured trigger time — it runs even while the app is closed and records nothing but daily closes, never a live intraday quote.

> Status: confirmed

### Trigger time

The local wall-clock time of day the scheduled fetch runs (default 22:15). A trigger missed while the machine was off executes at the next start; the run settles the latest pending trigger day.

> Status: confirmed

### Price history backfill

The action the user starts on a holding to fill its price history from the account's earliest transaction on the asset through yesterday (active holding) or its latest transaction (closed holding), never including today, recording a daily close only on dates that carry no price. Unlike the scheduled fetch's Backfill, it is not limited to 30 days and never overwrites a recorded price.

> Status: confirmed

### Backfill

Recording every missed trading-day close since the last successful scheduled fetch, up to 30 days back. Older gaps are left untouched.

> Status: confirmed

### External provider

A third-party HTTP service that returns current asset prices and their daily-close history, with no credential asked of the user. A build may have none, and then holds only prices typed by hand — the public build has none.

> Status: confirmed

---

## Asset Price Service Operations

| Name                 | Intent                                                                                                                                                                                | Status    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| `record_asset_price` | Create or overwrite the price for an asset on a given date (upsert by `(asset_id, date)`). Errors: `AssetNotFound`, `NotPositive`, `NonFinite`, `DateInFuture`, `Unknown`             | confirmed |
| `get_asset_prices`   | Return all recorded prices for an asset, ordered by date descending. Errors: `AssetNotFound`, `Unknown`                                                                               | confirmed |
| `update_asset_price` | Change the date and/or price of an existing price record; atomic delete-old + upsert-new when date changes. Errors: `NotFound`, `NotPositive`, `NonFinite`, `DateInFuture`, `Unknown` | confirmed |
| `delete_asset_price` | Remove a specific price record by `(asset_id, date)`. Errors: `NotFound`, `Unknown`                                                                                                   | confirmed |

---

## Agent Connection Concepts (introduced by AGT spec)

### Agent

> Status: confirmed

A program working for the owner that reads the portfolio and records in it through the application's tools, such as Claude Code or Claude Desktop. It reaches the portfolio only through the open application, and only once the owner allowed it. In French: "Agent".

### Bridge

> Status: confirmed

The program an agent client starts to reach the application. It holds no data: it passes each call to the open application and returns the answer. Never shown on screen. In French: "Passerelle".

### Connection request

> Status: confirmed

An agent waiting for the owner's answer in the window. Nothing is served to it until the owner allows; refusing gives it nothing. In French: "Demande de connexion".

### Removal request

> Status: confirmed

An agent asking the owner, in the window, to remove one transaction the owner typed: the transaction is shown in full with what would become of it, refusing is the default, and the answer covers that one transaction only. The agent calls `request_removal`. In French: "Demande de suppression" (owner, 2026-10-14).

### Change request

> Status: confirmed

An agent asking the owner, in the window, to change one transaction the owner typed, of any kind: the transaction is shown as it is and as it would become, refusing is the default, and the answer covers that one change only. The agent calls `request_change`. In French: "Demande de modification" (owner, 2026-10-17).

### Replacement request

> Status: confirmed

An agent asking the owner, in the window, to replace four transactions the owner typed — a sale, a withdrawal, a deposit and a purchase, a transfer recorded the four-step way — with one transfer: the four are shown struck through above the transfer recorded in their place, refusing is the default, and the answer covers those four only (AGT-090 to AGT-092). The agent calls `request_transfer_replacement`. In French: "Demande de remplacement" (owner, 2026-10-24).

### Agent session

> Status: confirmed

What one "Allow for this session" opens: the agent's calls are served without another question until the owner disconnects it, it leaves, or the application closes. Nothing of it is remembered afterwards, except on what it recorded: every transaction an agent records is marked with its session. In French: "Session".

### Mark

> Status: confirmed

What a transaction or a price an agent recorded carries (widened to a price by the owner, 2026-10-10). A transaction carries the agent's name and its session: the account journal shows it in the column "Recorded by" ("Enregistré par"); a transaction the owner typed has none. A price carries the source "Agent", shown in the price history. In code: `AgentRecording`, `recorded_by`; `AssetPriceSource::Agent`. In French: "Marque".

## Multi-Device Sync Concepts (introduced by SYN spec)

### Device

> Status: confirmed

One installation of the application taking part in sharing a portfolio. Known to the other devices by a stable identity and a user-chosen name.

### Change

> Status: confirmed

One recorded modification of one record — a creation, an update, or a removal — made on one device. The unit exchanged between devices.

### Logical Timestamp

> Status: confirmed

The ordering value every change carries. It is always greater than every change the device had recorded or applied before, so "later" between two independent edits of the same record is well defined on every device regardless of clock differences.

### Segment

> Status: confirmed

A published file carrying a consecutive batch of one device's changes. Once published it is never modified.

### Area

> Status: confirmed

The part of the shared folder that one device publishes into. Only that device writes there, except when another device starts the portfolio over.

### Manifest

> Status: confirmed

A device's published identity card: its identity, its name, its data-format version, the application version it last synced with, and how far it has published.

### Roster

> Status: confirmed

The set of manifests in the folder — every device currently sharing the portfolio, as each device sees it.

### Sync Cursor

> Status: confirmed

How far this device has applied another device's changes. Distinct from a fee schedule's catch-up cursor (FEE-043), which tracks generated deduction periods.

### Sync Passphrase

> Status: confirmed

The secret every device sharing a portfolio holds, from which the key that encrypts every published file is derived. Never stored in readable form; there is no recovery if it is forgotten.

### Inconsistent Holding

> Status: confirmed

A holding whose merged transactions, each valid on the device that recorded it, together break an invariant (for example an oversold position). Kept visible with its reason until the user corrects or removes one of the transactions.

---

## Application Areas (named by the commit scopes, `docs/commit-rules.md`)

### Performance

> Status: confirmed

How much an account, or the whole portfolio, gained or lost over each month or year, the owner's own deposits and withdrawals left out. In French: "Performance".

### Import

> Status: confirmed

A file's rows brought into one account, previewed, then confirmed. In French: "Import".

### Command line

> Status: confirmed

The installed program run from a terminal or a script, recording without opening a window. In French: "Ligne de commande".

### Settings

> Status: confirmed

The page where the owner makes the application's choices: language, theme, reference currency, sync, AI agents. In French: "Paramètres".

### Sync

> Status: confirmed

Keeping one portfolio the same on the owner's computers, through a folder they share, each computer publishing its changes and applying the others'. In French: "Synchronisation".

### Update

> Status: confirmed

A newer version of Folioneer, found, proposed and installed from the application, the portfolio kept. In French: "Mise à jour".
