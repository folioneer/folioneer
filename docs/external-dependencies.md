# External services and embedded data

What the application calls and what it carries, with the terms each one is offered under. `just licence-check` reads the licence of every dependency the application **ships**; it cannot see the terms of a **service** the application calls, nor of reference data copied into the source. This document is the other half, and the release audit re-reads it (`/dep-audit`).

Terms are quoted as read on the date beside each one. They change; re-read before anything is sold.

> This is a record of what the providers write, made by the owner's agent, not legal advice. Anything marked ⚠️ deserves a qualified reading before money depends on it.

---

## Services called at runtime

| Service                                                         | Used for                                 | Commercial use  | Without it                                       |
| --------------------------------------------------------------- | ---------------------------------------- | --------------- | ------------------------------------------------ |
| [Yahoo Finance](#yahoo-finance) `query1.finance.yahoo.com`      | every asset price and daily close        | ⚠️ forbidden    | no automatic prices; manual entry still works    |
| [Frankfurter](#frankfurter) `api.frankfurter.dev`               | exchange rates, latest and historical    | ✅ allowed      | the ECB fallback covers the latest rates         |
| [European Central Bank](#european-central-bank) `ecb.europa.eu` | exchange rates, fallback (ADR-009)       | ✅ with credit  | Frankfurter alone; no fallback                   |
| [OpenFIGI](#openfigi) `api.openfigi.com`                        | asset lookup by ISIN or name             | ✅ data; ⚠️ API | the asset form is filled by hand                 |
| [GitHub Releases](#github-releases) `github.com`                | the in-app updater's manifest and assets | ✅ allowed      | no in-app update; a release is installed by hand |

### Yahoo Finance

- **Provider**: Yahoo. Endpoint `/v8/finance/chart/`, undocumented and unofficial — there is no published API contract, so the general [Terms of Service](https://legal.yahoo.com/us/en/yahoo/terms/otos/index.html) govern it (read 2026-09-21).
- **Commercial use**: forbidden. §2.5 — _"Unless otherwise expressly stated, you may not access or reuse the Services, or any portion thereof, for any commercial purpose."_
- **Automated access**: forbidden **irrespective of commerce**. §2.4.9 forbids to _"access or collect data … using any automated means, devices, programs, algorithms or methodologies, including but not limited to robots, spiders, scrapers"_. §2.4.10 forbids building _"any database, archive, mobile application, data feed, widget or any other aggregated data source that competes with or constitutes a material substitute"_.
- **Redistribution**: §2.8 forbids reproducing or distributing any portion of the Services for commercial purposes.
- **What this means here**: the usual reading — "fine while it is a free personal tool" — does not survive §2.4.9. An application that fetches prices on a schedule is automated access whether or not anyone is paid. So this is not only a question of what may be sold; the free application is already outside these terms today.
- **Decision** (2026-09-21): it leaves the public build. #036 makes the application whole without a price provider, #037 provided the seam, #038 moves the client into the owner's private build and deletes it here. The private build remains one person fetching their own holdings, which is what the endpoint is tolerated for in practice, but it is not what §2.4.9 permits, and this document does not pretend otherwise.
- **Also**: the endpoint can change shape or disappear without notice (ADR-017), which is a second, independent reason not to build anything sold on it.

### Frankfurter

- **Provider**: the Frankfurter project, open source. [frankfurter.dev](https://frankfurter.dev/) (read 2026-09-21).
- **Commercial use**: allowed. The documentation answers the question directly — _"Yes, absolutely. See each provider's terms for details on the underlying data."_
- **Quotas**: none. _"There are no quotas. Requests are rate-limited to prevent abuse, but there are no monthly or daily caps."_
- **Self-hosting**: supported, via the project's own deployment guide — so a paid service is not hostage to the public instance.
- **Underlying data**: rates from central banks, the ECB among them. The provider's answer defers to those sources, so the ECB's condition below travels with the rates Frankfurter serves.
- **Decision**: keep. No action.

### European Central Bank

- **Provider**: the ECB, `eurofxref-daily.xml`. [Copyright and reuse](https://www.ecb.europa.eu/services/disclaimer/html/index.en.html) and the [reference-rates page](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html) (read 2026-09-21).
- **Commercial use**: allowed, with conditions. Reuse is free, but _"When such information is distributed or reproduced, it must appear accurately and the ECB must be cited as the source."_ Where the information appears in something sold for profit, the buyer must be told — before payment and on access — that _"the information may be obtained free of charge through this website."_ Modifications (the ECB's example: seasonal adjustment, growth rates) must be stated.
- **Attribution today**: met. The rate rows name their source (`source_ecb`, `source_frankfurter` in the translations), so the ECB is cited where its rates are shown.
- **Intended use**: _"The reference rates are published for information purposes only"_ and _"Using the rates for transaction purposes is strongly discouraged."_ The application values a portfolio — information — and executes nothing, so this fits. It would not fit a feature that settled or quoted a real exchange.
- **Decision**: keep. If a paid feed ever serves these rates onward, the free-of-charge notice becomes an obligation on the seller, not just a courtesy.

### OpenFIGI

- **Provider**: Bloomberg.
- **The data** — FIGI identifiers and their metadata are released under the MIT licence and dedicated to the public domain: free to _"use, display, reproduce, distribute and create derivative works … including redistribution of the FIGI Identifiers to your customers"_, for _"any purpose, commercial or non-commercial"_ (Bloomberg's FIGI dedication, via its [Terms of Service](https://www.openfigi.com/docs/terms-of-service) and the [launch announcement](https://www.openfigi.com/insights/newsletters/2016/1/20/open-figi-com-and-open-figi-api-press-release), 2026-09-21). The data is not the problem.
- **The API** — ⚠️ **not verified**. `openfigi.com` answered HTTP 503 to every automated read of its terms and documentation on 2026-09-21, so the service's own conditions — rate limits, permitted use of the endpoint, termination — have **not** been read. What is known from secondary sources: the API is free and open, unauthenticated requests are allowed but rate-limited more tightly than keyed ones, and a key is free.
- **Decision**: keep for now; **the terms must be read in a browser before anything is sold**, since a public-domain dataset says nothing about the conditions on the service that serves it. A paid product would in any case want a key, which is where those conditions attach.

### GitHub Releases

- **Provider**: GitHub. The updater reads `releases/latest/download/latest.json` from the public repository and downloads the installer beside it.
- **Commercial use**: allowed — distributing releases of one's own software is what the service is for, and nothing in it restricts the software to non-commercial terms.
- **Note**: this is the one service the **private** build will not use as it stands; #038 points its updater at a private repository, which is the same service under the same terms plus authentication.
- **Decision**: keep. No action.

---

## Embedded reference data

| Data                    | Where                              | Terms                             | Decision    |
| ----------------------- | ---------------------------------- | --------------------------------- | ----------- |
| MIC exchange codes (14) | `context/asset/domain/exchange.rs` | free of charge; ⚠️ terms unstated | keep, watch |
| ISO 4217 currency codes | the `iso_currency` crate           | a dependency — `licence-check`    | keep        |
| Inter, Manrope fonts    | `@fontsource-variable/*`           | OFL-1.1, allowed by name          | keep        |
| Lucide icons            | `lucide-react`                     | ISC, allowed                      | keep        |

**MIC exchange codes.** ISO 10383 codes with their venue names, fourteen of them, hardcoded as the canonical set (AST-001). ISO appointed SWIFT the registration authority, and the register is published free of charge at [iso20022.org](https://www.iso20022.org/market-identifier-codes) (read 2026-09-21). ⚠️ No licence or redistribution terms are published alongside it, so the permission to copy from it is not written down anywhere the owner can point to. What the application embeds is fourteen four-letter codes and their plain names — facts, not expression, and a small fraction of a register of thousands. That is a thin risk rather than none. Re-read this if the set ever grows towards the whole register, or if the venue names start coming from the register rather than being written here.

**The other three** are dependencies the application ships, so `just licence-check` already gates them on every pull request, and `licence-allowlist.json` records the two fonts by name under OFL-1.1 — which permits a font to be bundled and sold with software, not sold alone.

---

## What this changes

- Of five services called, **one** forbids what the application does: Yahoo Finance, and it forbids it today rather than only when something is sold. That entry's decision is already queued as #036 → #037 → #038.
- The exchange rates are clean on both paths, and the ECB's one condition — cite the source — the application already meets.
- **One item is unverified**: the OpenFIGI API's own terms. No replacement entry is filed for it, because nothing yet says one is needed; the obligation is to read them, and it is recorded here and in the release audit rather than as a backlog entry that might sit unread.
- No replacement entry is filed beyond those that already exist: #039 holds the hosted price feed that replaces Yahoo for whoever subscribes.
