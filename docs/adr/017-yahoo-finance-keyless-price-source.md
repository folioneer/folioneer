# ADR 017 — Yahoo Finance is the sole keyless price source; no user-supplied API keys

**Date**: 2026-06-12
**Status**: Accepted

## Context

The application fetches asset prices automatically. The candidate sources fall in two groups, and both were tried in the field before this decision:

- **Stooq**, keyless or with a user-supplied key: the anonymous path is served to some IP ranges and denied to others, behind a proof-of-work gate; the keyed path needs a key whose signup is captcha-gated. A user can be stranded where neither works, with no automated price at all.
- **Keyed APIs** (Finnhub, Alpha Vantage, Twelve Data): each needs the user to obtain a key, and the application to carry a key-management surface (settings screen, OS-keychain storage with fallbacks, a refresh-time key gate).

Yahoo Finance's unofficial endpoints have a reputation for being blocked (URL rotation, CAPTCHA gates, IP throttling). That holds for the `/v7/quote` endpoints and for high-volume scrapers. An empirical probe (2026-06-12) shows a different picture for the endpoint that matters: `https://query1.finance.yahoo.com/v8/finance/chart/{symbol}` returns structured JSON, needs no API key and no cookie/"crumb" handshake, and served data from a VPN/datacenter exit IP that Stooq denied on the same day. Under Folioneer's usage profile (a cached on-launch burst of ~5–50 requests/day) it is accessible.

## Decision

**Yahoo Finance's `/v8/finance/chart/` JSON endpoint is the sole automated price source**, key-less. The application has **no user-supplied API key feature**.

- `AssetPriceSource` is `Manual | YahooFinance`.
- The price-fetch path threads no key and no fetch-mode flag; no bounded context, command, screen or storage exists for credentials.
- Yahoo symbols are derived per venue: bare ticker for US (`AAPL`), exchange suffix elsewhere (`VOD.L`, `BMW.DE`, `MC.PA`).
- An unknown symbol (Yahoo returns HTTP 200 with `chart.error.code = "Not Found"`) maps to a typed not-found outcome, not a hard error.

Alternatives considered:

- **Stooq, keyless and keyed** — rejected. Both modes fail from some networks and the key can be unobtainable; a proof-of-work solver plus a key surface for a provider that may not answer is cost with no benefit.
- **A keyed API (Finnhub / Alpha Vantage / Twelve Data)** — rejected. Each brings the key-acquisition friction and the machinery to manage it. Alpha Vantage's 25 requests/day free tier is also incompatible with multi-holding portfolios.
- **Literal HTML scraping** (Google/Investing.com) — rejected. Brittle against markup changes, trips anti-bot defenses, and raises terms-of-service concerns. The `/v8/chart/` JSON endpoint gives the same "no key" benefit with structured data.

## Consequences

- **Pros**: zero-friction onboarding — first launch shows live prices with no key, no captcha, no proof-of-work; no credential code to maintain; data arrives as structured JSON with current price, currency, and daily history in one response; broad global coverage including EU venues.
- **Cons**: the app depends on an **undocumented** Yahoo endpoint that can change shape, rotate, or rate-limit without notice — accepted deliberately and confined to one adapter behind the `AssetPriceSource` enum, so a provider swap stays local; **pence-quoted venues are a correctness hazard** — Yahoo reports LSE prices in `GBp` (and `ZAc`, `ILA` elsewhere), so the adapter must normalize to the major ISO unit (`÷100`) at the boundary or valuations against GBP holdings break; adding a keyed provider later means building a credential surface from nothing; and if Yahoo ever blocks the user's IP, only manual entry remains.
