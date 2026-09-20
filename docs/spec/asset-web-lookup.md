# Business Rules — Asset Web Lookup (WEB)

## Context

The Asset Web Lookup feature allows users to search for financial instrument metadata from the OpenFIGI API (maintained by Bloomberg) before creating a new asset. The user types a name, ticker, or ISIN into a search box, selects an instrument from the returned list, and the Add Asset form is pre-filled with the retrieved metadata (name, reference, currency, and asset class). All pre-filled fields remain editable; the user saves via the existing `add_asset` command.

This is a **feature spec** extending the asset creation flow. The new Tauri command issues an outbound HTTP request to the OpenFIGI API and lives in `use_cases/asset_web_lookup/` — consistent with the `update_checker` use case, which is also an external HTTP concern. No new persisted entity is introduced; lookup results are transient.

---

## Value Object Definition

### AssetLookupResult

A transient value object returned by the OpenFIGI API. Not persisted; used only to pre-fill the Add Asset form.

| Field         | Business meaning                                                                                                                                        |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `name`        | Full name of the financial instrument (e.g. "Apple Inc.").                                                                                              |
| `reference`   | Ticker symbol returned by OpenFIGI; pre-fills the Add Asset `reference` field. Absent when OpenFIGI returns no ticker for the result (WEB-046).         |
| `isin`        | ISIN code (12 chars, ISO 6166); pre-fills the Add Asset `isin` field. Populated only on the ISIN path (WEB-046); absent on the keyword path.            |
| `currency`    | ISO 4217 trading currency of the instrument (e.g. "USD"). Absent if OpenFIGI does not return one for the result.                                        |
| `asset_class` | Classification of the instrument mapped from the OpenFIGI `securityType`. Absent if the type is unrecognised (WEB-023).                                 |
| `exchange`    | Canonical `Exchange` value (per AST Entity Definition) resolved from the OpenFIGI response. Absent when OpenFIGI returns no recognized venue (WEB-049). |

---

## Business Rules

### Entry Point and Initiation (010–019)

**WEB-010 — Asset creation entry point (frontend)**: Initiating the creation of a new asset opens the web lookup step instead of going directly to the blank Add Asset form.

**WEB-011 — Minimum query length (frontend)**: Each lookup field's search action requires at least 1 character in its input. An empty field disables that field's search action; the other field's action is independent. The ISIN search button is enabled at ≥1 character (the strict 12-character format check is the backend's responsibility — WEB-016 — so the user receives a clear typed error if the entered value is malformed rather than a silently-disabled button that gives no hint why).

**WEB-012 — Two explicit lookup inputs (frontend)**: The lookup step exposes two separate input fields with distinct search actions, stacked vertically:

- **ISIN** — accepts a 12-character International Securities Identification Number.
- **Keyword** — accepts a free-text instrument name or ticker symbol.

Each field has its own search button. There is no auto-detection or shared input; the user explicitly chooses which path to invoke. Activating either button submits only that field's value to `lookup_asset` with the corresponding mode (WEB-014).

**WEB-013 — Fill manually bypass (frontend)**: A "Fill manually" action is always visible in the lookup step. Activating it skips the web lookup entirely and opens the blank Add Asset form, preserving the pre-existing creation path.

**WEB-014 — Explicit lookup mode (backend)**: The lookup action takes an explicit mode that selects either the ISIN path or the keyword path. The frontend chooses the path based on which input was used. The backend never infers the path from the query shape.

- **ISIN path** — the query is sent to the OpenFIGI ISIN mapping endpoint after passing format validation (WEB-016). If validation fails, no HTTP call is made.
- **Keyword path** — the query is sent to the OpenFIGI keyword search endpoint after diacritics normalization (WEB-015).

**WEB-015 — Diacritics normalization (backend)**: On the keyword path (WEB-014), the trimmed query is normalized by Unicode NFD decomposition followed by combining-mark removal. This maps `"Société Générale"` to `"Societe Generale"`, `"Münchener Rück"` to `"Munchener Ruck"`, and so on. OpenFIGI's name index is unaccented, so the unnormalized form returns zero hits for accented inputs. ASCII inputs are unaffected. The ISIN path does not apply this normalization (ISIN charset is restricted to `[A-Z0-9]` per WEB-016).

**WEB-016 — ISIN format validation (backend)**: On the ISIN path (WEB-014), the query is first trimmed and uppercased; this normalized form is the value used by the rest of the pipeline (sent to OpenFIGI, embedded in `AssetLookupResult.isin` per WEB-046, and ultimately persisted by `add_asset` into the `Asset.isin` field per AST-023). The normalized form MUST satisfy three checks before any HTTP call. If any check fails the action is rejected with a specific error and no HTTP request is made:

1. **Length**: exactly 12 characters.
2. **Charset**: characters 1–2 are ASCII letters (`[A-Z]`, an ISO 3166-1 alpha-2 country code), characters 3–11 are alphanumeric (`[A-Z0-9]`), character 12 is a digit (`[0-9]`).
3. **Check digit**: the trailing digit MUST match the value computed by the Luhn-mod-10 algorithm defined in ISO 6166 over the first 11 characters (letters expanded to their `A=10, B=11, …, Z=35` numeric values before the Luhn pass).

Country code (chars 1–2) is not validated against the ISO 3166-1 list — OpenFIGI accepts any well-formed ISIN and a strict country whitelist would reject newly-issued codes the project has not yet learned about.

### Lookup Command (020–029)

**WEB-020 — Backend command (backend)**: The lookup action takes the query and the path mode (WEB-014) and returns a (possibly empty) ordered list of `AssetLookupResult` items on success.

**WEB-021 — No API key required (backend)**: The OpenFIGI API is accessed without authentication. No credential is stored or transmitted.

**WEB-022 — Result limit (backend)**: The command returns at most 30 results. If the OpenFIGI response contains more, only the first 30 are forwarded.

**WEB-023 — Asset class mapping (backend)**: The OpenFIGI `securityType` field is mapped to `AssetClass` per the canonical OpenFIGI vocabulary (the values returned by `GET /v3/mapping/values/securityType`):

| OpenFIGI `securityType`                                                                                     | `AssetClass`                                                                                                                                                  |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `"Common Stock"`                                                                                            | `Stocks`                                                                                                                                                      |
| `"ETP"`                                                                                                     | `ETP` (umbrella for ETF/ETN/ETC — OpenFIGI does not expose the structural distinction at this level; users can edit the class manually for finer granularity) |
| `"Mutual Fund"`, `"Open-End Fund"`, `"Closed-End Fund"`, `"Fund of Funds"`, `"Pvt Eqty Fund"`               | `MutualFunds`                                                                                                                                                 |
| `"Bond"`, `"Conv Bond"`                                                                                     | `Bonds`                                                                                                                                                       |
| `"Crypto"`                                                                                                  | `DigitalAsset`                                                                                                                                                |
| `"REIT"`                                                                                                    | `RealEstate`                                                                                                                                                  |
| `"Warrant"`, `"Equity Option"`, `"Index Option"`, `"SINGLE STOCK FUTURE"`, `"SINGLE STOCK DIVIDEND FUTURE"` | `Derivatives`                                                                                                                                                 |

Any unrecognised `securityType` (including `"Structured Product"`, `"Certificate"`, `"ADR"`, MTNs, futures sub-types not listed above, and others) results in `asset_class` being absent from the result.

> The `Cash` asset class is reserved for the system Cash Asset (CSH-014). OpenFIGI does not return a meaningful `Cash` `securityType` for user-importable instruments, so the table does not map to `Cash`.

**WEB-024 — Currency passthrough (backend)**: The ISO 4217 currency code returned by OpenFIGI is forwarded unchanged. If OpenFIGI does not return a currency for a result, the `currency` field is absent.

**WEB-025 — Error handling (backend)**: The lookup action surfaces three behavioral failure modes, surfaced as distinct typed errors so the frontend can render per-mode copy (WEB-033):

- **Invalid ISIN format** — the ISIN path was invoked with a query that failed any of the three checks in WEB-016 (wrong length, invalid charset, or check-digit mismatch). No HTTP call is made.
- **Rate limited** — OpenFIGI signaled `HTTP 429 Too Many Requests`. Transient and recoverable: the user can wait briefly and retry.
- **Generic network failure** — any other reachability or HTTP failure (network unreachable, connection timeout, any other non-2xx response).

No partial result list is returned on any failure mode.

### Search UX (030–039)

**WEB-030 — Loading state (frontend)**: While `lookup_asset` is in progress, a loading indicator is shown and the search action is disabled to prevent duplicate requests.

**WEB-031 — Results display (frontend)**: Each `AssetLookupResult` is shown as a two-line selectable row. First line: reference code (if present, displayed as a muted prefix) followed by the instrument name. Second line: the asset class label if present, otherwise a localised "unknown type" fallback label; if `exchange` is also present, its `label` (e.g. "Euronext Paris") is shown alongside, separated by a visual separator. When `exchange` is absent the second line shows only the type label (or the fallback). When both `asset_class` and `exchange` are absent the second line shows only the fallback label. The currency field is not shown in the results list; it is pre-filled silently into the form on selection (WEB-041).

**WEB-032 — Empty results state (frontend)**: When the command returns an empty list, a message indicates no instruments were found. The user can modify the query and search again, or use the "Fill manually" bypass (WEB-013).

**WEB-033 — Error state (frontend)**: When the lookup action fails, an inline error message is shown beside the field that triggered the action. Copy is per failure mode (WEB-025):

- **Invalid ISIN format**: "Not a valid ISIN. Expected 12 characters with a valid check digit." — shown beside the ISIN field; no Retry affordance (the user must correct the input). The other field's button stays enabled.
- **Rate limited**: "Too many searches. Please wait a minute and try again." — Retry affordance; the user is expected to wait briefly before retrying.
- **Generic network failure**: "Could not reach the lookup service. Try again or fill manually." — Retry affordance.

In all cases the "Fill manually" bypass (WEB-013) remains accessible. No navigation away from the search step occurs on error.

### Selection and Pre-fill (040–049)

**WEB-040 — Result selection (frontend)**: Selecting a result from the list transitions to the Add Asset form with fields pre-filled from the selected `AssetLookupResult`.

**WEB-041 — Pre-filled fields (frontend)**: The following Add Asset form fields are pre-filled from the selected result: `name` ← `AssetLookupResult.name`; `reference` ← `AssetLookupResult.reference` (blank if absent); `isin` ← `AssetLookupResult.isin` (blank if absent); `currency` ← `AssetLookupResult.currency` (blank if absent); `asset_class` ← `AssetLookupResult.asset_class` (no selection if absent); `exchange` ← `AssetLookupResult.exchange` (no selection if absent). All pre-filled values remain user-editable per WEB-043.

**WEB-042 — Risk level default from asset class (frontend)**: When opening the Add Asset form from the web lookup path (creation only), if `asset_class` is pre-filled, `risk_level` is automatically set to the class default, consistent with the `AssetClass::default_risk()` behaviour defined in AST-010. When `asset_class` is absent, `risk_level` is left at its form default. This rule applies exclusively to the creation flow; it does not affect the edit form.

**WEB-043 — All pre-filled fields are editable (frontend)**: Every pre-filled field in the Add Asset form can be changed by the user before saving. The lookup result is a suggestion, not a locked value.

**WEB-044 — Category default (frontend)**: The `category` field is not provided by the OpenFIGI lookup and defaults to the system default category, consistent with the existing manual form behaviour.

**WEB-045 — Save via existing add_asset command (frontend + backend)**: Saving the pre-filled form uses the existing `add_asset` command. All existing Asset creation rules apply — reference uniqueness check, field validation, and `AssetUpdated` event publication — as defined in the AST spec. The web lookup path introduces no new save rules.

**WEB-046 — Reference and ISIN field sources (backend)**: `AssetLookupResult.reference` is the ticker symbol returned by OpenFIGI when available; absent when OpenFIGI does not return a ticker for the result. This is consistent across both lookup paths so `reference` always carries the value that market-data providers expect (ticker, not ISIN). `AssetLookupResult.isin` is populated only on the ISIN path with the normalized ISIN query (per WEB-016: trimmed + uppercased + format-validated); absent on the keyword path because OpenFIGI's `/v3/search` response does not expose ISIN.

**WEB-047 — Back navigation from form to search results (frontend)**: When the form is in the pre-filled state (WEB-040), a back action is available that returns the user to the search step. The previous query and results list are retained in memory; the user does not need to retype the query. Selecting a different result replaces all pre-filled values.

**WEB-048 — Result ordering (backend)**: Results are sorted by instrument type priority before the 30-item truncation (WEB-022). Priority is determined by the resolved `asset_class` value (WEB-023): Priority 1 (top) — `asset_class` ∈ {`Stocks`, `ETF`, `ETP`, `MutualFunds`, `Bonds`, `DigitalAsset`, `RealEstate`, `Cash`}; Priority 2 — `asset_class` = `Derivatives`; Priority 3 — `asset_class` absent (unrecognised `securityType`, including structured products, certificates, ADRs, MTNs, and futures sub-types not mapped in WEB-023). Within each priority group, the original OpenFIGI response order is preserved.

**WEB-049 — Exchange field (backend)**: The OpenFIGI response is normalized to a canonical `Exchange` value (per AST Entity Definition) via a per-provider mapper. The mapper consults OpenFIGI's exchange identifier fields (`micCode` when present, otherwise `exchCode`) and returns `Some(Exchange)` when the venue is in the canonical curated set, `None` otherwise (including when OpenFIGI returns no exchange information). The resolved `Exchange` is forwarded as `AssetLookupResult.exchange`. Provider key equality (e.g. OpenFIGI's `micCode` happening to match ISO 10383 MIC) is treated as accidental convergence; the mapper is the contract.

**WEB-050 — Primary listing surfacing (backend)**: A deduplication-and-enrichment pipeline transforms the OpenFIGI response into a clean shortlist of primary venue(s) per share class, headed (on the ISIN path) by the instrument's home venue. The pipeline has six clauses (WEB-050a … WEB-050f). The keyword path runs all six; the ISIN path skips 050a–050d (the `/v3/mapping` response is already canonical for the ISIN) and applies only 050e and 050f. The opinionated tables and pipeline live in a single `primary_listing_processor` module so they can be audited and tested in isolation.

**WEB-050a — No request-time securityType filter on keyword search (backend)**: On the keyword path, the initial `/v3/search` request is issued **without** a `securityType` filter. OpenFIGI's `/v3/search` accepts only a single `securityType` string (not an array), so request-time filtering cannot cover the broader set of asset classes WEB-023 supports. Instead, classification happens post-response: hits whose `securityType` is in the WEB-023 mapping table receive an `asset_class`; unrecognised ones fall through with `asset_class` absent and rank into Priority 3 of WEB-048's ordering. The downstream pipeline (WEB-050b onward) plus WEB-022's 30-item cap and WEB-048's priority sort keep the noise tractable while letting ETPs (Amundi MSCI World, iShares EuroStoxx UCITS ETF, …), mutual funds, bonds, REITs, and crypto surface in keyword results — not just stocks.

**WEB-050b — Drop trade-reporting noise (backend)**: Results whose `shareClassFIGI` is `null` are discarded — these are pure trade-reporting venue rows (e.g. `X1` "TradEcho APA EU") that carry no canonical share class.

**WEB-050c — Dedup by share class (backend)**: Remaining keyword-path results are grouped by `shareClassFIGI`. Multiple keyword-search hits for the same share class collapse into one group, preserving the order of first appearance for cross-share-class ranking.

**WEB-050d — Share-class enrichment (backend)**: For the unique `shareClassFIGI` values from WEB-050c, a single batched call is made to `/v3/mapping` with `idType: "ID_BB_GLOBAL_SHARE_CLASS_LEVEL"`. The response — all known listings for that share class globally — replaces the original keyword-search hits for the group. This is the step that uncovers primary listings (e.g. `FP AI`) that the keyword search alone never returns.

**WEB-050e — Primary pick per share class with mode-dependent cap (backend)**: Each share-class group is filtered against `GLOBAL_VENUE_PRIORITY` — a curated, hardcoded ordered list of primary venue `exchCode` values (e.g. `UN`, `UW`, `LO`, `JT`, `FP`, `GY`, `HK`, `SE`, `AT`, `CT`, `IM`, `NA`, …). On the ISIN path, the country-code prefix of the ISIN (chars 1–2) is resolved through the curated `ISIN_COUNTRY_TO_PRIMARY_VENUES` table to a (possibly empty) ordered list of home venues, which is prepended to the priority list so the home venue wins ranking. Entries whose `exchCode` is on the resulting list are kept, in priority order. The per-share-class cap depends on the path:

- **Keyword path**: up to 10 entries per share class — browsing intent, the user is exploring venues for a known instrument family.
- **ISIN path**: up to 3 entries per share class — selection intent, the user has already pinpointed the specific instrument and wants a clean shortlist headed by the home venue.

If no entry of the share-class group matches the priority list (only possible on the keyword path — the ISIN path's `/v3/mapping` response is already filtered to the ISIN's known listings and the priority list always contains at least the global venue universe), the first entry from OpenFIGI's order is kept as a fallback so the share class is not lost.

A country prefix may map to multiple home venues when the same prefix covers structurally different instrument types — e.g. `IE → [ID, LO, NA, GY]`: Dublin (`ID`) wins for Irish equities (Ryanair, CRH, Kerry Group), and London/Amsterdam/Xetra cover the Irish-domiciled UCITS ETF case, where the asset is registered in Ireland for tax/regulatory reasons but trades primarily abroad.

**WEB-050f — Final cap (backend)**: The combined result list across all share classes is truncated to the WEB-022 cap (30 entries).

---

## Workflow

```
Add Asset FAB / button
    → Web Lookup step (WEB-010)
        ┌─ ISIN path ─────────────────────────────────────────────┐
        │ user types ISIN, clicks ISIN Search                     │
        │   backend: ISIN format check (WEB-016)                  │
        │     ↳ fail → InvalidIsinFormat (WEB-025, WEB-033)       │
        │   backend: HTTP /v3/mapping (WEB-020)                   │
        │   → up to 3 venues per share class (WEB-050e)           │
        └─────────────────────────────────────────────────────────┘
        ┌─ Keyword path ──────────────────────────────────────────┐
        │ user types name/ticker, clicks Keyword Search           │
        │   backend: diacritics normalize (WEB-015)               │
        │   backend: HTTP /v3/search → /v3/mapping (WEB-020)      │
        │   → up to 10 venues per share class (WEB-050e)          │
        └─────────────────────────────────────────────────────────┘
        → both paths converge in the results list, capped at 30 (WEB-022)
        → results list shown (WEB-031)
        → user selects a result (WEB-040)
        → Add Asset form opens pre-filled (WEB-041–WEB-046)
            user reviews / edits fields (WEB-043)
            ← back action available to return to results (WEB-047)
            → save → existing add_asset command (WEB-045)
            → AssetUpdated published; asset appears in list

Bypass path:
    → "Fill manually" (WEB-013) → blank Add Asset form (existing behaviour)

No results (WEB-032):
    → "No instruments found" + retry or fill manually

Error (WEB-033):
    → inline error beside the offending field; the other path stays usable
```

---

## UX Draft

### Entry Point

Clicking the "Add Asset" FAB opens the web lookup dialog. A "Fill manually" link/button is always visible as an escape hatch.

### Main Component

A dialog or modal with two sequential states:

1. **Search state** — two stacked lookup rows (ISIN + Search button, Keyword + Search button) + "Fill manually" bypass + shared results list (or loading / empty / error state).
2. **Form state** — the existing Add Asset form, pre-filled (or blank if bypass used) + back action.

### States

- **Idle**: Both input fields empty, each search button disabled (WEB-011). "Fill manually" visible.
- **Loading**: Spinner shown beside the active field; both search actions disabled (WEB-030).
- **Results**: Up to 30 selectable rows (WEB-022, WEB-031), ordered by type priority (WEB-048). Each row: reference code + name (first line); type label + exchange (second line). The list is shared — switching paths replaces the previous results.
- **Empty**: "No instruments found" message; retry or fill manually (WEB-032).
- **Error**: Inline error message beside the field that triggered the action; the other field stays usable. "Fill manually" always accessible (WEB-033).
- **Form (pre-filled)**: Add Asset form fields populated from selected result; all editable (WEB-041–WEB-043). Back action returns to search results (WEB-047).
- **Form (manual)**: Blank Add Asset form — identical to current behaviour.

### User Flow

1. User clicks "Add Asset".
2. Web lookup dialog opens: two input rows (ISIN / Keyword) + "Fill manually" bypass.
3. User chooses one path:
   - **ISIN**: types a 12-character code (e.g. `IE00B53L3W79`), clicks the ISIN search button. If the format is invalid the action is rejected inline with a field-local error (no HTTP call).
   - **Keyword**: types an instrument name or ticker (e.g. `AAPL`, `iShares S&P 500`), clicks the keyword search button.
4. Backend fetches from OpenFIGI; results appear in the shared list.
5. User clicks the matching row.
6. Add Asset form opens with name, reference, currency, asset class pre-filled.
7. User reviews, adjusts category and risk level if needed, and saves.
8. Existing `add_asset` command runs; asset appears in the list.

---

## Open Questions

None — all questions have been resolved.
