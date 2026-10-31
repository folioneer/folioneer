# ADR 024 — The public build offers a list of price providers, one active per computer

**Date**: 2026-10-31
**Status**: Accepted — supersedes ADR-021

## Context

ADR-021 withdrew the keyless price source: the public build called Yahoo Finance no longer, because its terms forbid automated access ([`external-dependencies.md`](../external-dependencies.md)), and only the owner's private build kept a client. No replacement came, so whoever installs the public build types every price by hand. A private build is also a second pipeline to keep alive for one person. The application is free software run on each user's own computer: a request made from that computer, for that person's own holdings, because that person turned it on, is that person's use of Yahoo, not a service the project runs.

## Decision

**The public `extensions.rs` plugs in a list of External providers, in code; each computer chooses one or none, and none is the default.** This amends ADR-020: what the file returns for prices is the list and its default rather than one optional provider, and a client may live beside the file, in `extensions/`. The list starts with Yahoo Finance, through the client the private build used. Another provider is a line in that file; a provider is never a user's or a community's file.

The choice lives in a file of the computer's data folder, not in the database. It must not sync: fetching from Yahoo is what one person turned on, on one computer. The headless scheduled run must also read it before opening anything (SPF-072). A missing file means the build's default; content not on offer means off, so a mistake fails closed.

Every fetch goes through one provider that follows the choice and reaches none when off, so "no request leaves the computer" holds below every caller rather than in each of them. The commands also refuse first, with a typed code. The fetching use cases always exist, so choosing a provider needs no restart.

Alternatives considered:

- **A hosted price feed (the former TODO-039)** — not rejected, not decided: a future entry in the same list. It does not help anyone today.
- **Keep Yahoo private only** — rejected. The public build stays without prices, and the private build's pipeline stays for one user.
- **On by default** — rejected. The user must knowingly take on Yahoo's terms; a fresh install sends nothing to anyone.
- **The choice in the database, or in the interface's settings** — rejected. The first syncs to other devices; the second cannot be read by the headless run.

## Consequences

- **Pros**: anyone can have automated prices at no cost to the project; the private build can pause; a new provider costs one line.
- **Cons**: the public build carries a client of an unofficial endpoint that can change without notice; the project offers access its terms do not permit, so the notice shown before fetching (MKT-214) must stay honest about it. An overlay of `extensions.rs` written for the former shape no longer compiles, which the changelog's technical notes announce.

## Guard

- **Reversal looks like**: a price provider that fetches without a choice on this computer, or a client constructed outside `extensions.rs`.
- **Guard**: tests `the_public_build_offers_a_price_provider_off_by_default` (`src-tauri/src/extensions.rs`), `off_reaches_no_provider`, `only_the_chosen_provider_is_called` (`use_cases/price_provider_choice/chosen.rs`), `with_no_provider_chosen_every_fetching_command_is_refused` (`src-tauri/src/app.rs`).
