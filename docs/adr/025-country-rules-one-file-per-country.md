# ADR 025 — Country Rules Live in One Rust File per Country

**Date**: 2026-11-07
**Status**: Accepted

## Context

Until now the only country-dependent code was data: the wrapper lists (ACC-040), labels for which "no tax is computed and no rule is enforced" (`docs/spec/account.md`). The batch of 2026-11-07 brings the first behaviour that depends on a country — a default cost basis method (#514), France's year-end figures (#517), then Germany's (#518) — and the countries the product targets differ in behaviour, not only in values: the UK's share pooling and 30-day matching and its tax year from 6 April, the US short and long term split, Spain's two-month loss rule, France's whole-portfolio formula for crypto. Without a home, each such rule would land in whichever use case first needs it, a `match` on the country growing in each, and adding a country would mean finding every one of them. These differences are the forcing function; each becomes a spec rule when it is planned.

## Decision

**Each country's rules live in one Rust file, behind one shared interface, and the list of countries is one line per file** (owner, 2026-11-07, on #764). The interface names what a country may differ in; each of its parts has a generic default, and a country's file states only what differs from it. The list starts with the seven countries the owner named — France, Germany, Spain, Italy, Poland, the United Kingdom and the United States — and stays open. A use case asks the interface for a country's answer and never names a country itself. Where the files live is in `ARCHITECTURE.md`.

Alternatives considered:

- **A data file per country (JSON or TOML)** — rejected: the differences are behaviour, not values. Pooling, matching and loss rules are algorithms a configuration file would either fail to express or turn into a small interpreter.
- **A `match` on the country in each use case** — rejected: every new country touches every use case that has a country rule, and nothing shows what one country differs in.
- **One file per region** — rejected for now: the countries above differ inside a region, and a region adds a level nothing needs yet.

## Consequences

- **Pros**: a country is one file and one line; what a country differs in reads in one place, and a reviewer checks a country's rules without reading the use cases.
- **Cons**: a rule two countries share is written twice or lifted into the interface's default; a behaviour that does not fit the interface means changing the interface and every file that overrides it; a country whose rules are mostly the defaults is still a file of its own. Reversing it once the cost basis, the year-end figures and the reports read the interface means rewriting each of them.

## Guard

- **Reversal looks like**: a use case or a service branching on a country code for a rule the interface could carry, or a country's rule living outside its file.
- **Guard**: tests `every_country_of_the_list_is_whole`, `each_country_implies_its_cost_basis` (`context/account/domain/country/mod.rs`) and `the_lists_are_those_published` (`context/account/domain/wrapper.rs`).
