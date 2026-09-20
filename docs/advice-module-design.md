# Design — Monitored Assets, Indicators, and the Private Advice Module

> Status: **draft, not ratified**. Written 2026-07-31 from an exploration session.
> No implementation has started. Numbers marked _(measured)_ were probed live
> against the providers on 2026-07-31.

## Context

The app tracks what a portfolio _is worth_. This design adds what a position is
_doing_: technical measurements on a price series, and — separately — an opinion
derived from them.

The two halves are deliberately split across two repositories:

| Half                                                         | Lives in             | Ships to            |
| ------------------------------------------------------------ | -------------------- | ------------------- |
| Price bars, indicator mathematics, the indicator display     | this repository      | every build         |
| Verdicts, actionable levels, rationale text — the **advice** | a private repository | private builds only |

The seam is not arbitrary: an indicator is a **measurement** (ATR is 28.57), an
advice finding is a **judgement** (exit below 412.30 because trend has broken).
Measurements are factual and reusable; judgements are the part deliberately kept
out of a public artifact.

---

## Part 1 — Market data (public)

### Monitored assets

A new asset flag, **monitored**, marks a position the user wants analysed. It is
independent of ownership: a held asset may be unmonitored, and a watchlist asset
may be monitored without any holding. Only monitored assets accumulate bar
history — the whole point is to keep the download small.

Suggested ubiquitous-language entry: _Monitored asset — an asset the user has
flagged for price-series analysis; the app keeps a bar history for it._

### Bar storage

`asset_prices` holds one price per date with latest-write-wins semantics
(ADR-012) and mixes provider values with manual entries. A bar series is a
different animal: immutable, provider-owned, four values plus volume per day.
Proposal: a separate table rather than widening the existing one.

```
asset_daily_bars(asset_id, date, open, high, low, close, volume, source)
```

Valuation and performance keep reading `asset_prices` untouched; nothing in the
existing engine changes.

### Data availability — what the providers actually give

_(all measured 2026-07-31)_

- **Daily bars** carry `open, high, low, close, volume` plus `adjclose`. The
  current Yahoo client parses only `close`; the rest is present in the same
  payload and simply discarded today.
- **Fundamentals are closed.** `quoteSummary` answers `401 Invalid Crumb`
  without a cookie/crumb handshake. Since ADR-017 removed the key
  infrastructure, every valuation algorithm (Graham number, dividend-discount
  fair value, P/E screens) is **out of scope** — no data, no advice.
- **Mutual funds have no intraday range.** A Frankfurt fund listing reports
  `high == low == close` (NAV pricing); a stock on the same day swings several
  euros. This is a property of the instrument, not a gap history can fill:
  range-based algorithms are permanently inapplicable to the fund holdings.

Applicability therefore splits by what an algorithm consumes, not by asset:

| Input                            | Works on                    |
| -------------------------------- | --------------------------- |
| Closes only                      | everything, funds included  |
| High/low range (ATR and friends) | equities, ETFs, crypto only |
| Fundamentals                     | nothing — provider closed   |

### Minimum-data fetch policy

Each indicator declares the number of bars it needs; the window fetched for an
asset is the maximum over the **enabled** indicators, plus burn-in for the
recursive ones (Wilder smoothing converges slowly, so its formal minimum is not
its useful minimum).

| Indicator        | Formal minimum | Useful minimum |
| ---------------- | -------------- | -------------- |
| SMA / EMA(n)     | n              | n + 20%        |
| Bollinger(20, 2) | 20             | 25             |
| Donchian(20/10)  | 20             | 25             |
| RSI(14), Wilder  | 15             | ~100           |
| ATR(14 or 22)    | 15 / 23        | ~100           |
| MACD(12, 26, 9)  | 35             | ~120           |
| SMA(200)         | 200            | 240            |
| Faber 10-month   | 10 month-ends  | 10 month-ends  |
| Momentum 12-1    | 12 month-ends  | 13 month-ends  |

**One year of daily bars satisfies every line of that table**: 256 bars
_(measured)_ covers the 240-bar worst case, and its 12 month-end closes feed the
monthly algorithms without a second request. Monthly series are derived locally
from the daily bars — never fetched separately.

Cost per monitored asset _(measured, LVMH)_: **25 KB, 256 bars** for one year;
48 KB / 511 bars for two. Ten monitored assets is a quarter of a megabyte once,
then a few kilobytes a day.

Lifecycle:

1. Asset marked monitored → one ranged request for the computed window.
2. Thereafter → incremental top-up of the missing tail only, folded into the
   existing scheduled daily fetch.
3. Asset unmonitored → history retained (cheap) but no longer refreshed.

Two years should be preferred over one only if verdict history or a sanity
backtest is wanted; the default stays one year.

---

## Part 2 — Indicator primitives (public)

Pure functions over a bar slice, each independently testable, each returning a
number or `None` when the series is too short — no verdicts, no levels framed as
actions, no text.

- Trend: `sma`, `ema`, `macd` (Appel)
- Volatility: `true_range`, `atr` (Wilder), `bollinger` (Bollinger)
- Oscillator: `rsi` (Wilder)
- Channel: `donchian_high`, `donchian_low` (Donchian)
- Series shape: `monthly_closes`, `return_over`, `drawdown_from_peak`
- Portfolio: current weight vs target, relative drift

Explicitly **not** here: "stop", "entry", "exit", "signal", "buy", "sell". The
public app may render an indicator panel — RSI is 61.2, the 20-day high is
498.00 — because those are readings, not recommendations.

---

## Part 3 — The advice module (private)

The private crate composes public primitives into findings. It holds no
indicator mathematics of its own, which keeps it small and keeps the tested
arithmetic in one place.

### Algorithm catalogue — published only, nothing invented

| Algorithm                    | Source                                                                     | Consumes           | Produces a level                                        |
| ---------------------------- | -------------------------------------------------------------------------- | ------------------ | ------------------------------------------------------- |
| Donchian channel / Turtle S1 | Donchian; Dennis & Eckhardt, _Original Turtle Trading Rules_; Faith (2007) | high/low           | entry, exit, 2×ATR stop                                 |
| Chandelier Exit              | Chuck LeBeau                                                               | high/low           | trailing stop                                           |
| ATR, RSI, Parabolic SAR, ADX | Wilder, _New Concepts in Technical Trading Systems_ (1978)                 | range / closes     | SAR stop; RSI invertible to the price implying 30 or 70 |
| Bollinger Bands              | Bollinger, _Bollinger on Bollinger Bands_ (2001)                           | closes             | band prices                                             |
| MACD                         | Appel (1970s)                                                              | closes             | no                                                      |
| 10-month TAA                 | Faber, _A Quantitative Approach to Tactical Asset Allocation_, JWM (2007)  | 10 month-ends      | the SMA is the trigger price                            |
| Dual Momentum (GEM)          | Antonacci, _Dual Momentum Investing_ (2014)                                | 12 month-ends      | ranking only                                            |
| Time-series momentum         | Moskowitz, Ooi & Pedersen, JFE (2012)                                      | 12 month-ends      | direction only                                          |
| 5/25 rebalancing bands       | Swedroe                                                                    | holdings + targets | price at which a band breaks                            |

Faber's rule deserves first place in the build order: it needs ten monthly
closes — the lowest data requirement of anything here — and it is the only
family that applies to the fund-heavy assurance-vie account, where most of the
capital sits.

### Finding shape

Every finding carries the level, the arithmetic that produced it, the window it
used, and its published source. Sketch:

```
asset · algorithm · verdict · levels[] · inputs[] · window · source
```

Rendered:

> **MICROSOFT** · Chandelier Exit (LeBeau) — stop **412.30 €** = 22-day high
> 498.00 − 3 × ATR(22) 28.57. Last 465.10, 11.4% of headroom. _Trend intact._

### Not-computable is a first-class result

A missing verdict must state its cause and its remedy, never render as a blank:

> **DNCA INVEST CONVERTIBLES** · Chandelier — _not computable: mutual fund, no
> intraday range; ATR undefined. No amount of history changes this._

> **BARINGS GLB EM** · MACD — _not computable: 41 of ~120 bars. Monitored since
> 2026-07-14; available from about 2026-11._

This mirrors the lesson from the suppressed lifetime-performance metrics: a bare
"—" reads as a bug and costs an investigation.

---

## Part 4 — The hook

Requirement: the module is downloaded only from a private repository, and no
advice semantics live in the public one.

**Recommended — optional Cargo dependency on a private git crate.** The private
crate implements the analysis and registers its own commands; the public repo
retains only three neutral seams:

1. A feature name in `Cargo.toml` with an optional git dependency.
2. One `#[cfg(feature = ...)]` registration block in the Specta builder.
3. A Vite alias resolving the advice feature to a null stub when the private
   sources are absent.

Consequence to accept: generated bindings must split, with the private
commands' TypeScript emitted to a second, git-ignored file — otherwise the
committed `bindings.ts` churns between public and private builds.

The composition root is already shaped for this: `AppContainer::build` takes
optional providers and attaches them when present, exactly as the rate-history
provider is wired today. The "hook" is one more optional argument.

**Alternative — sidecar process.** The module ships as a separate binary
downloaded from private releases; the app runs it on demand and reads back
JSON. Fully decoupled, updatable without rebuilding, no compile-time
entanglement — at the cost of a second build pipeline and an IPC boundary.
Preferable only if algorithm iteration should not require an app rebuild.

**Rejected — public port with private implementation.** Cheapest to build, but
the advice concept, its types, and an empty panel would all be visible in the
public repository, which is precisely what this design avoids.

---

## Prerequisites, in order

1. **Public** — monitored flag, `asset_daily_bars`, ranged bar fetch with the
   minimum-window policy, incremental top-up in the scheduled fetch.
2. **Public** — indicator primitives with their unit tests, and an indicator
   panel for monitored assets.
3. **Private** — Faber 10-month and the 5/25 bands first: they apply to the
   funds, need the least data, and produce genuine levels.
4. **Private** — Turtle/Chandelier/RSI layer for the equity accounts once bar
   history is dense.

Step 1 and 2 are ordinary features of this app, useful with or without step 3.

## Caveats

Backtested rules read better on paper than in an account: trend systems buy
protection with whipsaws, and published edges decay after publication. Stop
levels also collide with assurance-vie mechanics — arbitrage delay, settlement
at an unknown NAV, occasional fee or holding-period constraints — so a level
that assumes instant execution is advisory at best there.

This argues for a module that **reports levels with their rationale** rather
than issuing imperatives, and that never places or schedules an order.

## Open questions

- [ ] Ratify the hook (recommended Cargo feature vs sidecar).
- [ ] Is SMA(200) wanted? It alone drives the window from ~120 bars to 240; the
      one-year default already covers it, but dropping it would allow a
      six-month window at half the payload.
- [ ] Target weights per asset or per class — the 5/25 rule needs a target to
      measure drift against, and none is recorded today.
- [ ] Does the indicator panel ship for every asset or only monitored ones?
- [ ] Fund NAV series: is the daily series worth storing at all when only
      month-ends are consumed, or is monthly granularity enough for funds?
