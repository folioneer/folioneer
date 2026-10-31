# Architecture Decision Records

An ADR keeps the _why_ of a **technical** choice costly to reverse — a framework, a storage
or transaction strategy, a test boundary. A functional or domain decision is never an ADR:
it is written in the spec or the glossary (owner, 2026-10-18): a developer-side choice
hard to remove — the language, the framework, a data representation. ADRs are rare.

**The gate** — all three hold, or no ADR: (1) genuinely complex, real trade-offs with no
obvious answer; (2) not obvious from the code or the spec; (3) costly to reverse,
significant rework across the codebase. The owner confirms before one is written.

**Writing one** — `docs/adr/{NNN}-{slug}.md`, the next number (never reused), one decision,
what was chosen and why, never how it is implemented; then its row below. An ADR is never
deleted: a new one supersedes it, and only the old one's `Status` line and index row
change; an ADR already superseded is not superseded again (the newer one is). The `Status`
is `Accepted`, `Accepted — supersedes ADR-{NNN}` or `Superseded by ADR-{NNN}`; a tentative
decision stays an open question in its spec.

```markdown
# ADR {NNN} — {Decision Title}

**Date**: {YYYY-MM-DD}
**Status**: Accepted

## Context

{The problem, constraint or forcing function — why a decision is needed now.}

## Decision

{What was chosen and why over the alternatives, which are named.}

## Consequences

- **Pros**: {what becomes easier or possible.}
- **Cons**: {what becomes harder or constrained.}
```

## Index

| ADR                                                                          | Title                                                                         | Status                                                   |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------- |
| [ADR-001](001-use-i64-for-monetary-amounts.md)                               | Use i64 for Monetary Amounts                                                  | Accepted                                                 |
| [ADR-002](002-replace-asset-account-with-holding.md)                         | Replace `AssetAccount` with `Holding`                                         | Accepted                                                 |
| [ADR-003](003-cross-context-use-case-orchestration.md)                       | Cross-Context Use Case Orchestration via Sequential Service Calls             | Superseded by ADR-022                                    |
| [ADR-004](004-use-cases-inject-services-not-repositories.md)                 | Use Cases Inject Services, Not Repositories                                   | Superseded by ADR-022                                    |
| [ADR-005](005-account-details-inject-transaction-service.md)                 | Inject TransactionService into account_details for Realized P&L               | Superseded by ADR-022                                    |
| [ADR-006](006-unit-of-work.md)                                               | Unit of Work Pattern for Cross-Aggregate Atomicity                            | Accepted                                                 |
| [ADR-007](007-e2e-combobox-boundary.md)                                      | E2E Test Boundary at HeadlessUI ComboboxField                                 | Superseded by ADR-022                                    |
| [ADR-009](009-fx-rate-provider-chain.md)                                     | FX Rate Provider Chain: Frankfurter + ECB XML + Manual                        | Accepted                                                 |
| [ADR-012](012-latest-write-wins-source-as-metadata.md)                       | Latest-Write-Wins; Source Field is Metadata                                   | Accepted                                                 |
| [ADR-013](013-recompute-account-performance-on-read.md)                      | Recompute Account Performance on Read                                         | Accepted                                                 |
| [ADR-014](014-price-refresh-lock-scope-exclusion.md)                         | Per-Asset Price-Refresh Lock via Fetch-Scope Exclusion                        | Accepted                                                 |
| [ADR-017](017-yahoo-finance-keyless-price-source.md)                         | Yahoo Finance is the Sole Keyless Price Source; No API Keys                   | Superseded by ADR-021                                    |
| [ADR-018](018-lazy-catch-up-management-fee-generation.md)                    | Lazy Catch-Up Generation for Recurring Management Fees                        | Accepted                                                 |
| [ADR-019](019-per-device-change-log-multi-device-sync.md)                    | Per-Device Change Log for Multi-Device Sync                                   | Accepted                                                 |
| [ADR-020](020-one-extension-file-per-build.md)                               | One Extension File Decides a Build's External Data Sources and Update Channel | Accepted                                                 |
| [ADR-021](021-withdraw-the-keyless-price-source.md)                          | Withdraw the Keyless Price Source Decision                                    | Superseded by ADR-024                                    |
| [ADR-022](022-withdraw-the-code-organisation-decisions.md)                   | Withdraw the Code-Organisation Decisions into the Rules                       | Accepted — supersedes ADR-003, ADR-004, ADR-005, ADR-007 |
| [ADR-023](023-agents-reach-the-portfolio-through-the-running-application.md) | An Agent Reaches the Portfolio Only Through the Running Application           | Accepted                                                 |
| [ADR-024](024-the-public-build-offers-a-list-of-price-providers.md)          | The Public Build Offers a List of Price Providers, One Active per Computer    | Accepted — supersedes ADR-021                            |
