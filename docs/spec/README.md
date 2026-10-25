# Specs — numbering, trigrams and format

A spec is `docs/spec/{feature}.md`, in English: the business rules of one feature, written
or amended by the agent from the issue and the code and checked by `spec-reviewer` before anything is
built from it.

## Numbering

All business rules in feature specs use a consistent numbering scheme: **`TRIGRAM-NNN`**

- **TRIGRAM**: 3-letter code identifying the feature/context
- **NNN**: 3-digit index with gaps, grouped by topic; a new spec starts from 010–019 eligibility & initiation, 020–029 creation, 030–039 updates & status changes, 040–049 deletion, 050–059 extensions, and an existing spec keeps its own blocks
- **Letter suffix** (`TRIGRAM-NNNA`, `TRIGRAM-NNNB`, …): sub-rules of `TRIGRAM-NNN`, used when a rule needs surgical sub-parts or a full block runs out of indices

Once assigned, a rule number never changes; a removed rule leaves its number vacant. A new trigram (three letters, unique) is registered below before its spec uses it.

---

## Trigram registry

| Trigram | Spec Name                   | Description                                                                                                                                                                                      | Status  |
| ------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------- |
| TRX     | Financial Asset Transaction | recording purchases and managing holdings                                                                                                                                                        | active  |
| ACD     | Account Details             | Account holdings, performance and asset list                                                                                                                                                     | active  |
| ACC     | Account Management          | CRUD operations for financial accounts                                                                                                                                                           | active  |
| TXL     | Transaction List            | Browse, edit and delete transactions per (account, asset) pair, and the account journal                                                                                                          | active  |
| SEL     | Sell Transaction            | Recording asset sales, holding reduction, and realized P&L                                                                                                                                       | active  |
| MKT     | Market Price                | Per-asset price: auto-fetch when the build has an External provider (none publicly; Yahoo Finance in the owner's private build), a price typed by hand per asset; unrealized P&L                 | active  |
| AST     | Asset                       | Asset CRUD, archival, and listing                                                                                                                                                                | active  |
| CAT     | Category                    | User-defined asset category management                                                                                                                                                           | active  |
| CLI     | Command Line                | Record opening balances, purchases, sales, dividends, deposits, withdrawals and prices from a terminal, without a window                                                                         | active  |
| UPD     | Application Update          | Auto-update detection, download, and install                                                                                                                                                     | active  |
| WEB     | Asset Web Lookup            | Search OpenFIGI to pre-fill the Add Asset form                                                                                                                                                   | active  |
| PFD     | Portfolio Dashboard         | Cross-account aggregate view — SUPERSEDED by GPF (global performance view) + the accounts list; improvements ride GPF/ACC                                                                        | retired |
| CSH     | Cash Tracking               | Cash-as-Holding (one per currency); Deposit/Withdrawal; Buy/Sell re-linked to cash                                                                                                               | active  |
| PRF     | Account Performance         | Per-account value-over-time table: month/year rows, net-of-flows perf (MoM/YTD/all-time)                                                                                                         | active  |
| DIV     | Cash Dividend               | Cash dividend transaction: income credited to cash, attributed to the paying asset                                                                                                               | active  |
| FXR     | Foreign Exchange Rate       | Currency-pair rates (fetch + manual) for live multi-currency holding valuation, and the portfolio settings                                                                                       | active  |
| KEY     | API Key Management          | BYOK provider API keys + Connections dialog — RETIRED by ADR-017 (price fetch moved to keyless Yahoo Finance)                                                                                    | retired |
| FSD     | Free Share Distribution     | Bonus shares received at zero cost: quantity event, no cash, cost basis unchanged                                                                                                                | active  |
| FEE     | Management Fee Deduction    | Quantity-reducing fee paid in shares (recurring % schedule + one-off); cost basis unchanged, fees surfaced per holding                                                                           | active  |
| INT     | Interest Credit             | Capitalized interest: zero-cost quantity credit on a fund line or the cash line, entered as a rate or a direct amount                                                                            | active  |
| GPF     | Global Performance          | Portfolio-wide performance: all accounts (or one asset across accounts) aggregated in the reference currency; single-account scope = PRF                                                         | active  |
| WNW     | What's New Dialog           | Once-per-upgrade dialog stacking the CHANGELOG sections between the last-seen and current app versions                                                                                           | active  |
| SPL     | Stock Split                 | Value-neutral corporate action rescaling a held position (quantity × factor, cost basis preserved); same asset, never a fork                                                                     | active  |
| HNO     | Holding Note                | Per-position free-text note with optional live share-price alarm (below/above threshold, in-app bell only)                                                                                       | active  |
| SPF     | Scheduled Price Fetch       | Daily OS-scheduled close-of-day price + FX download that runs even when the app is closed, when the build has an External provider; in-app setup only                                            | active  |
| SYN     | Multi-Device Sync           | Share one portfolio across several desktops: per-device encrypted change log exchanged through a cloud folder                                                                                    | active  |
| CFR     | Sync Conflict Resolution    | Outcome of every concurrent-change situation between devices (later-wins, parent/child, ledger invariants, fees, observations)                                                                   | active  |
| PMV     | Price Movement              | Post-fetch report: per-account value before/after a global price fetch, isolating movement attributable to prices                                                                                | active  |
| NUM     | Numbers by language         | The decimal separator typed and shown follows the application's language; the command line keeps the dot                                                                                         | active  |
| AGT     | AI Agent Connection         | An AI agent reads the portfolio and records in it through the open application: a bridge started with `--mcp`, a local channel, the owner's consent in the window, read tools, marked recordings | active  |
| TDI     | Trade Dialog Insights       | Figures derived while recording a buy or a sell, shown in the trade dialog                                                                                                                       | active  |
| WOF     | Write-off                   | A quantity leaving a holding without proceeds — a network fee, dust, coins lost — at its cost, no cash moved, lost kept apart                                                                    | active  |
| TRF     | Transfer                    | A holding moved between two accounts at its cost — no cash, no realized result, a fee in kind, the rate between currencies recorded                                                              | active  |
| IMP     | Import                      | A broker's CSV file read through a saved column mapping, previewed row by row, then confirmed into one account                                                                                   | active  |

---

## Reference Format

When referencing a rule in code, documentation, or tests, use the full code:

✅ **Correct**: "ACC-010 ensures an account name is unique"
✅ **Correct**: `// Implements TRX-020: validate the trade date`
❌ **Avoid**: "R1 ensures eligibility" (ambiguous across specs)

---

## Format

```markdown
# Business Rules — {Feature Title} ({TRIGRAM})

## Context

{2-4 sentences: the business need, the feature's role, the main entities.}

---

## Entity Definition

> A feature that persists no entity replaces this section with `## Computed Concepts`: the
> views it computes, in business terms.

### {EntityName}

{One sentence describing what this entity represents in the business domain.}

| Field         | Business meaning                                                    |
| ------------- | ------------------------------------------------------------------- |
| `field_name`  | {What this field represents to the user, without technical detail.} |
| `other_field` | {Same.}                                                             |

> Names in English, Rust convention (`snake_case` fields, `PascalCase` entities); business
> meaning only — not the type, storage format or default value.

---

## Business Rules

### Eligibility and Initiation (010–019)

**{TRIGRAM}-010 — {Short Title} (frontend + backend)**: {Precise, testable description of the rule.}

**{TRIGRAM}-011 — {Short Title} (backend)**: {Description.}

### Creation (020–029)

**{TRIGRAM}-020 — {Short Title} (frontend)**: {Description.}

### Updates and Status Changes (030–039)

**{TRIGRAM}-030 — {Short Title} (frontend + backend)**: {Description.}

---

> Rules cover: creation, validation, update, deletion, state transitions,
> inter-entity dependencies, edge cases.

---

## Workflow

{ASCII diagram of the main user flow, if relevant}

---

## UX Draft

### Entry Point

{How the user accesses the feature: drawer entry, FAB button, contextual action...}

### Main Component

{Type: modal / page / panel / dialog. Notable sub-components.}

### States

- **Empty**: {what the user sees with no data}
- **Loading**: {loading state}
- **Error**: {error messages, validation}
- **Success**: {success feedback}

### User Flow

1. {Step 1}
2. {Step 2}
3. ...

---

## Open Questions

- [ ] {Point to clarify before or during implementation}
```

## Writing the rules

- Each rule atomic (one behaviour), testable on its own, with its scope: `(frontend)`,
  `(backend)` or `(frontend + backend)`. A notable edge case is a rule of its own.
- Every observable behaviour has a rule: one the Workflow or UX Draft shows without a rule
  gets one — form defaults, sort toggles, a modal staying open on error, empty state versus
  no search result. What the project's existing patterns settle (navigation, success
  feedback, network errors) is written as a rule without asking.
- An active ADR's choice is applied without asking; the owner is asked only when the
  feature must break one.
- Every assumption is an open question, asked before the work (`docs/workflow.md` § 3);
  a decision that passes the ADR gate (`docs/adr/README.md`) is an open question
  `ADR-SUGGESTED: {the decision and why}`, never mandated.
- **What and why, never how**: no SQL, file paths, function or component names, libraries
  or data structures. **Nor the contract's words**: no command names, error variant names
  (an error is a concept — "the action is rejected with a specific error"), return types,
  caller / request / DTO / payload framing, or runtime mechanisms ("background job",
  "frontend store", "thread", "worker"):

| ❌ Avoid                                                   | ✅ Prefer                                                                |
| ---------------------------------------------------------- | ------------------------------------------------------------------------ |
| "The command returns immediately to the caller"            | "The action is acknowledged synchronously"                               |
| "The backend rejects the request with `AccountNotFound`"   | "The action is rejected with a specific error if the account is unknown" |
| "Results are signaled via a background job posting events" | "Results are signaled asynchronously via the `{Entity}Updated` event"    |
| "The setting is stored in the frontend store"              | "The setting persists across sessions on the current device"             |

## Coverage

Before the spec is reviewed, grade each dimension it touches **Clear / Partial /
Missing**; a rule two competent developers could build two ways is at best Partial. Every
Partial or Missing cell is an open question, unless an existing pattern answers it;
business dimensions only — non-functional and data-format questions are the contract's.

| Dimension                 | Partial/Missing when…                                                          |
| ------------------------- | ------------------------------------------------------------------------------ |
| Entity fields             | a field's business meaning, optionality, or allowed values is unstated         |
| State transitions         | a lifecycle exists but some transitions, or who triggers them, are unspecified |
| Validation thresholds     | "valid"/"invalid" is used without the concrete boundary (min, max, format)     |
| Deletion semantics        | removal is mentioned but hard-vs-soft delete and cascade are unstated          |
| Inter-entity dependencies | a relationship's required/optional nature or cascade effect is unclear         |
| Edge cases                | empty, concurrent, conflicting, or duplicate-input behaviour is unspecified    |
| Permissions               | who may act is not pinned down when more than one role is plausible            |

---

## Spec granularity rule

Two tiers — pick the one that fits:

**Domain spec** — for pure CRUD within a single bounded context (e.g. `asset.md`, `account.md`). Use when the feature touches one domain and does not extend another use case.

**Feature spec** — for cross-cutting capabilities that span ≥ 2 domains or extend an existing use case (e.g. `account-details.md`, `market-price.md`). One spec per user-visible capability; cross-references the domains it touches.

If two operations share a use case and entity, they belong in the same spec under the same TRIGRAM — even if they appear as separate UI flows (e.g. buy modal vs sell modal). Each operation takes its rules inside the topic blocks (§ Numbering), its section saying which:

```
## Entity Definition
## Operation A rules   (TRX-010 eligibility, TRX-020 creation, …)
## Operation B rules   (TRX-011 eligibility, TRX-021 creation, …)
```

> **Historical exception:** TRX and SEL are split because SEL was added as an extension spec after TRX was stable. Rule numbers are permanent — do not merge or renumber. `account-contract.md` cites both. All future transaction-domain rules go into TRX.
