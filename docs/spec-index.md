# Trigram Registry

| Trigram | Spec Name                   | Description                                                                                                                    | Status   |
| ------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------- |
| TRX     | Financial Asset Transaction | recording purchases and managing holdings                                                                                      | active   |
| ACD     | Account Details             | Account holdings, performance and asset list                                                                                   | active   |
| ACC     | Account Management          | CRUD operations for financial accounts                                                                                         | active   |
| TXL     | Transaction List            | Browse, edit and delete transactions per (account, asset) pair                                                                 | active   |
| SEL     | Sell Transaction            | Recording asset sales, holding reduction, and realized P&L                                                                     | active   |
| MKT     | Market Price                | Per-asset price: manual entry, keyless auto-fetch (Yahoo Finance, ADR-017), per-asset refresh lock; unrealized P&L             | active   |
| AST     | Asset                       | Asset CRUD, archival, and listing                                                                                              | active   |
| CAT     | Category                    | User-defined asset category management                                                                                         | active   |
| UPD     | Application Update          | Auto-update detection, download, and install                                                                                   | active   |
| WEB     | Asset Web Lookup            | Search OpenFIGI to pre-fill the Add Asset form                                                                                 | active   |
| PFD     | Portfolio Dashboard         | Cross-account aggregate view — SUPERSEDED by GPF (global performance view) + the accounts list; improvements ride GPF/ACC      | retired  |
| CSH     | Cash Tracking               | Cash-as-Holding (one per currency); Deposit/Withdrawal; Buy/Sell re-linked to cash                                             | active   |
| PRF     | Account Performance         | Per-account value-over-time table: month/year rows, net-of-flows perf (MoM/YTD/all-time)                                       | planning |
| DIV     | Cash Dividend               | Cash dividend transaction: income credited to cash, attributed to the paying asset                                             | active   |
| FXR     | Foreign Exchange Rate       | Currency-pair rates (fetch + manual) for live multi-currency holding valuation                                                 | planning |
| KEY     | API Key Management          | BYOK provider API keys + Connections dialog — RETIRED by ADR-017 (price fetch moved to keyless Yahoo Finance)                  | retired  |
| FSD     | Free Share Distribution     | Bonus shares received at zero cost: quantity event, no cash, cost basis unchanged                                              | active   |
| FEE     | Management Fee Deduction    | Quantity-reducing fee paid in shares (recurring % schedule + one-off); cost basis unchanged, fees surfaced per holding         | active   |
| INT     | Interest Credit             | Capitalized interest: zero-cost quantity credit on a fund line or the cash line, entered as a rate or a direct amount          | active   |
| GPF     | Global Performance          | Portfolio-wide performance: all accounts (or one asset across accounts) aggregated in EUR; single-account scope = PRF          | active   |
| WNW     | What's New Dialog           | Once-per-upgrade dialog stacking the CHANGELOG sections between the last-seen and current app versions                         | active   |
| SPL     | Stock Split                 | Value-neutral corporate action rescaling a held position (quantity × factor, cost basis preserved); same asset, never a fork   | active   |
| HNO     | Holding Note                | Per-position free-text note with optional live share-price alarm (below/above threshold, in-app bell only)                     | active   |
| SPF     | Scheduled Price Fetch       | Daily OS-scheduled close-of-day price + FX download that runs even when the app is closed; in-app setup only                   | active   |
| SYN     | Multi-Device Sync           | Share one portfolio across several desktops: per-device encrypted change log exchanged through a cloud folder                  | active   |
| CFR     | Sync Conflict Resolution    | Outcome of every concurrent-change situation between devices (later-wins, parent/child, ledger invariants, fees, observations) | active   |
| PMV     | Price Movement              | Post-fetch report: per-account value before/after a global price fetch, isolating movement attributable to prices              | active   |

---

## Spec granularity rule

Two tiers — pick the one that fits:

**Domain spec** — for pure CRUD within a single bounded context (e.g. `asset.md`, `account.md`). Use when the feature touches one domain and does not extend another use case.

**Feature spec** — for cross-cutting capabilities that span ≥ 2 domains or extend an existing use case (e.g. `account-details.md`, `market-price.md`). One spec per user-visible capability; cross-references the domains it touches.

If two operations share a use case and entity, they belong in the same spec under the same TRIGRAM — even if they appear as separate UI flows (e.g. buy modal vs sell modal).

```
## Entity definition
## Operation A rules   (TRX-010 … TRX-040)
## Operation B rules   (TRX-041 … TRX-080)
```

> **Historical exception:** TRX and SEL are split because SEL was added as an extension spec after TRX was stable. Rule numbers are permanent — do not merge or renumber. The `record_transaction` contract references both. All future transaction-domain rules go into TRX.
