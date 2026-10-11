# 2026-10-10 — DEBT-108 — Two error codes no code builds, and a comment that says too much

- Found by: reading the six contracts against the code (DEBT-096)
- Where: `FetchPriceTask::UnknownError` in `src-tauri/src/use_cases/asset_price_fetch/error.rs`; `PortfolioSyncTask::UnknownError` in `src-tauri/src/use_cases/portfolio_sync/error.rs`; `error.UnknownError` in `src/i18n/locales/{en,fr}/common.json`; `src/features/account_details/gateway.test.ts`; the comment of `Event::CurrencyPairUpdated` in `src-tauri/src/core/event_bus/event.rs`
- Severity: 🔵
- Observation: no code builds either `UnknownError`: each is named only by its own serialization test, and on the window's side by a string and a gateway test. The asset contract no longer promises it. The comment of `CurrencyPairUpdated`, which reaches the bindings, says the event is also raised when a pair is applied from another computer; only `declare_currency_pair` publishes it (SYN-064).
- User value: None directly — the error types say what can happen.
- Done when: the two variants, their string and the tests that only name them are gone, the comment says what the code does, the bindings regenerated.
