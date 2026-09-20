# Architecture Decision Records

| ADR                                                          | Title                                                                         | Status                        |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------- | ----------------------------- |
| [ADR-001](001-use-i64-for-monetary-amounts.md)               | Use i64 for Monetary Amounts                                                  | Accepted                      |
| [ADR-002](002-replace-asset-account-with-holding.md)         | Replace `AssetAccount` with `Holding`                                         | Accepted                      |
| [ADR-003](003-cross-context-use-case-orchestration.md)       | Cross-Context Use Case Orchestration via Sequential Service Calls             | Accepted — amended by ADR-005 |
| [ADR-004](004-use-cases-inject-services-not-repositories.md) | Use Cases Inject Services, Not Repositories                                   | Accepted                      |
| [ADR-005](005-account-details-inject-transaction-service.md) | Inject TransactionService into account_details for Realized P&L               | Accepted — amends ADR-003     |
| [ADR-006](006-unit-of-work.md)                               | Unit of Work Pattern for Cross-Aggregate Atomicity                            | Accepted                      |
| [ADR-007](007-e2e-combobox-boundary.md)                      | E2E Test Boundary at HeadlessUI ComboboxField                                 | Accepted                      |
| [ADR-009](009-fx-rate-provider-chain.md)                     | FX Rate Provider Chain: Frankfurter + ECB XML + Manual                        | Accepted                      |
| [ADR-012](012-latest-write-wins-source-as-metadata.md)       | Latest-Write-Wins; Source Field is Metadata                                   | Accepted                      |
| [ADR-013](013-recompute-account-performance-on-read.md)      | Recompute Account Performance on Read                                         | Accepted                      |
| [ADR-014](014-price-refresh-lock-scope-exclusion.md)         | Per-Asset Price-Refresh Lock via Fetch-Scope Exclusion                        | Accepted                      |
| [ADR-017](017-yahoo-finance-keyless-price-source.md)         | Yahoo Finance is the Sole Keyless Price Source; No API Keys                   | Accepted                      |
| [ADR-018](018-lazy-catch-up-management-fee-generation.md)    | Lazy Catch-Up Generation for Recurring Management Fees                        | Accepted                      |
| [ADR-019](019-per-device-change-log-multi-device-sync.md)    | Per-Device Change Log for Multi-Device Sync                                   | Accepted                      |
| [ADR-020](020-one-extension-file-per-build.md)               | One Extension File Decides a Build's External Data Sources and Update Channel | Accepted                      |
