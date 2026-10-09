# Business Rules — Asset Management (AST)

## Context

An asset represents a financial instrument or resource owned by the user: stock, ETF, bond, real estate, cryptocurrency, etc. Each asset belongs to a user category (e.g. "European Stocks", "Real Estate") and carries an ISO 4217 currency which is the security's quotation currency. Asset management is the foundation of the rest of the application: an account (`Account`) groups assets via operations (`Operation`); the performance dashboard relies on assets to compute portfolio value.

This spec covers asset creation, modification, and archival, both backend and frontend. CRUD rules for `AssetCategory` are in `docs/spec/category.md`. Asset prices (`AssetPrice`) are handled in `docs/spec/market-price.md`.

> Note: `reference` alone does not identify an instrument (the same ticker exists on several exchanges, in several currencies). What identifies an asset depends on its kind (AST-030, AST-032).

---

## Asset field definitions

### `name`

Human-readable name of the instrument (e.g. "Apple Inc.", "SCPI Pierval").

### `kind`

How the asset is identified and priced — Listed, Crypto, Custom or Cash (AST-030). Chosen when the asset is created and separate from its class: the class says what the asset is economically, the kind says what identifies it and where its price comes from.

### `class`

Type of financial asset among the fixed values of `AssetClass` (see AST-003). Not user-customizable.

### `category`

Free-form grouping defined by the user (e.g. "US Stocks", "European Real Estate"). Used to aggregate values in the performance dashboard. It is not a fixed taxonomy: the user creates their own categories.

### `currency`

**Quotation** currency of the security (ISO 4217: USD, EUR, BTC…). It is the currency in which the asset price is expressed — distinct from the account's reference currency. E.g. an Apple stock quoted in USD in an account whose reference currency is EUR.

### `risk_level`

Subjective risk score from 1 (low risk) to 5 (high risk). The frontend suggests a default value based on the chosen `class` (see AST-003), editable manually.

### `reference`

Security ticker (e.g. `AAPL`) or free-form identifier entered by the user (e.g. `APPART-PARIS-15`) for non-quoted assets. Required. For assets that also have an ISIN, the canonical ISO 6166 identity is stored separately in `isin` (see below); `reference` holds the ticker symbol used for market-data provider lookups (the External provider, etc.).

### `isin`

Optional International Securities Identification Number (12 characters, ISO 6166). When present, MUST satisfy the format validation defined in WEB-016 (length, charset, Luhn check digit). Every listed asset has one; a crypto or a custom asset has none (AST-031). A keyword lookup result carries none (OpenFIGI's free `/v3/search` response does not expose it): the user types it to save the asset as listed. Independent of `reference`.

### `exchange`

Optional canonical market identifier where the instrument is listed. Stored as a value of `Exchange` (see Entity Definition below). Absent for assets that are not listed on a tracked venue (e.g. real estate, free-form references) or for legacy assets created before the exchange field existed. The auto-fetch task uses it to select the correct provider symbol per MKT-110.

### `is_archived`

Indicates whether the asset is archived (removed from active lists). An archived asset retains all its historical data but cannot be modified or receive new prices.

### `interest_bearing`

Opt-in flag marking the asset as an eligible target for Interest credits (see AST-024 and `docs/spec/interest-credit.md` INT-012). Off by default; typically enabled for cash-like fund lines such as a "fonds en euros". The account's Cash Asset is always interest-eligible regardless of this flag (INT-023).

---

## Entity Definition

### `Exchange`

A canonical reference to a trading venue, independent of any market-data provider.

| Field   | Required | Business meaning                                                                           |
| ------- | -------- | ------------------------------------------------------------------------------------------ |
| `code`  | yes      | ISO 10383 Market Identifier Code (MIC), e.g. `XPAR` for Euronext Paris, `XNAS` for NASDAQ. |
| `label` | yes      | Human-readable display name, e.g. "Euronext Paris", "NASDAQ", "Deutsche Börse Xetra".      |

The list of supported `Exchange` values is finite and curated. Provider symbols (Yahoo venue suffixes, OpenFIGI exchange codes) are NOT stored on `Exchange` — they are resolved by per-provider mappers at the boundary.

---

## Business rules

### Asset — Backend

**AST-001 (was R1) — Field validation (backend)**: An asset is valid if and only if: `name` is non-empty, `reference` is non-empty, `category` is set, `class` is a value of `AssetClass`, `currency` is a valid ISO 4217 code, `risk_level` is an integer between 1 and 5 inclusive, — if present — `exchange.code` is a member of the canonical curated `Exchange` set, and — if present — `isin` satisfies the format validation defined in WEB-016. Any violation is rejected by the backend with an explicit error.

**AST-003 (was R3) — Asset classes and default risk (backend)**: Classification (`AssetClass`) is a fixed pre-seeded list, not user-customizable:

| Class          | `default_risk` |
| -------------- | -------------- |
| `Cash`         | 1              |
| `Bonds`        | 2              |
| `RealEstate`   | 2              |
| `MutualFunds`  | 3              |
| `ETF`          | 3              |
| `ETP`          | 3              |
| `Stocks`       | 4              |
| `DigitalAsset` | 5              |
| `Derivatives`  | 5              |

The default value is `Cash`. `Derivatives` covers leveraged or contingent instruments derived from an underlying asset: warrants, options, futures, and rights. The specific `securityType` values that map to this class from the OpenFIGI API are defined in WEB-023.

**AST-004 (was R4) — Reference normalization (backend)**: The reference is normalized at receipt: leading and trailing whitespace stripped, converted to uppercase (internal whitespace preserved).

**AST-005 (was R5) — Asset update (backend)**: All asset fields are editable after creation, within what the kind allows (AST-031) and unless the edit makes the asset the same as another (AST-032). The validation rules (AST-001) and reference normalization (AST-004) apply to modification as well as creation.

**AST-006 (was R6) — Asset archival (backend)**: Archiving an asset sets `is_archived = true`. The asset disappears from active lists but all associated data is preserved (operations, prices, holdings). An archived asset can no longer receive new prices or be modified. The price-mutation commands enforce this: `record_asset_price`, `update_asset_price`, and `delete_asset_price` reject with the `Archived` error when the target asset is archived. Read paths (`get_asset_prices`, the price-display chain MKT-031) remain available — archive blocks mutations, not visibility of historical data.

**AST-018 (was R18) — Asset unarchival (backend)**: Unarchiving an asset sets `is_archived = false`. The asset becomes active again, reappears in active lists, and can again be modified and receive new prices.

**AST-030 — An asset has a kind (backend)**: Every asset has one kind, stored with it and carried with it in sync:

| Kind     | What it is                                                       | Identified by                                 | Price                                        |
| -------- | ---------------------------------------------------------------- | --------------------------------------------- | -------------------------------------------- |
| `Listed` | A listing: an instrument on an exchange, in a currency           | its ISIN, its exchange and its currency       | fetched where a provider exists, or typed    |
| `Crypto` | A crypto asset; an asset, not money: bought and sold with a gain | its symbol (the reference), among crypto ones | the pair's, fetched where possible, or typed |
| `Custom` | What no market lists (real estate, a fund inside a contract)     | its reference, among custom ones              | typed                                        |
| `Cash`   | The application's own, one per currency (CSH-015, CSH-016)       | its currency                                  | none: one unit is worth one unit             |

The Price column says where the price of an asset of that kind is expected to come from. Which assets a price fetch covers is not decided by the kind: the fetch scope stays as `market-price.md` defines it (MKT-110, MKT-116, MKT-151).

**AST-031 — What each kind requires and forbids (backend)**: A listed asset has an ISIN; its exchange stays optional. A crypto or a custom asset has neither an ISIN nor an exchange. Class and kind go together both ways: an asset of the cash kind is of the `Cash` class and no other asset is; an asset of the crypto kind is of the `DigitalAsset` class and no other asset is — a listed crypto product is given another class (`ETP`) to be listed. The cash kind is the application's: a user never creates an asset of it (CSH-015 refuses the `Cash` class first) nor edits an asset into it, by its kind or by its class (`CashAssetNotEditable`); it carries no ISIN and no exchange. Creating or editing an asset against these rules is rejected, the class first, then the ISIN, then the exchange: `ClassNotAllowed`, `IsinRequired`, `IsinNotAllowed`, `ExchangeNotAllowed`. An edit may change the kind among listed, crypto and custom; the asset is then held to the rules of the kind it is given. An edit that states no kind keeps the asset's.

**AST-032 — The same asset is refused (backend)**: Two listed assets are the same when they share ISIN, exchange and currency (two without an exchange included); two crypto assets when they share a symbol — a crypto asset's symbol is its reference; two custom assets when they share a reference. References are compared without case; an archived asset counts. Creating a second one, or editing an asset into one, is rejected with `AssetAlreadyExists`, which names the existing asset. The same ISIN on another exchange, or in another currency, is another asset and is allowed. The rule is the core's, checked when a user creates or edits an asset; nothing in storage enforces it, since assets that existed before kinds may break it (AST-035). Unarchiving an asset checks nothing.

**AST-033 — Sync applies an asset as it is (backend)**: A change received from another device is applied without AST-031 or AST-032 (CFR-017): two devices that each create the same asset end with both, and both are reported (AST-035).

**AST-034 — The kind of an asset that states none (backend)**: An asset created without stating a kind, and an asset that existed before kinds, take the kind their class and ISIN make: the `Cash` class is cash, the `DigitalAsset` class is crypto, an ISIN makes it listed, anything else is custom. Existing assets are given theirs once, when the application is updated; nothing else of them changes and no change is recorded to publish, every device deriving the same kind from the same fields.

**AST-035 — Assets to settle (backend)**: An asset may break AST-031 or be the same as another (AST-032) without having been refused: it was given its kind by AST-034 or AST-036 (a custom asset on an exchange, a crypto asset with an ISIN), or it came from another device (AST-033). It is not changed. The core reports these assets, archived ones included, each with one rule it breaks — the first of AST-031 in that rule's order, otherwise the asset it is the same as. The user settles one by editing it into an asset the rules accept, an edit that leaves the breach being refused like any other; an archived one is unarchived first (AST-006).

**AST-036 — An asset change written before kinds (backend)**: A change written in data format version 1 carries no kind (SYN-038). Applying one, the asset keeps the kind this device already holds for it; an asset this device has never held takes the kind AST-034 gives. Two devices can therefore differ on the kind of one asset when a version 1 change moves its class or its ISIN after one of them was updated; they agree again at the asset's next edit on an updated device, whose change carries the kind.

### Asset — Frontend

**AST-002 (was R2) — Category pre-selection (frontend)**: The form pre-selects `default-uncategorized` if no category is chosen by the user, ensuring the field is always set on submission.

**AST-007 (was R7) — Asset table (frontend)**: The table displays the following columns, in this order, sorted by Name ascending by default:

| Column    | Content                                                                                    | Sortable |
| --------- | ------------------------------------------------------------------------------------------ | -------- |
| Name      | `asset.name`, and under it the other listings of its instrument (AST-039)                  | Yes      |
| Reference | `asset.reference`                                                                          | Yes      |
| Class     | `asset.class`                                                                              | Yes      |
| Category  | `asset.category.name`                                                                      | Yes      |
| CCY       | `asset.currency`                                                                           | Yes      |
| Risk      | `asset.risk_level` — risk badge (AST-011)                                                  | Yes      |
| Status    | "Archived" badge if `is_archived = true`; "Managed by Folioneer" on a Cash Asset (CSH-015) | No       |
| Actions   | See AST-013, AST-019, AST-020                                                              | No       |

The table displays only active assets (`is_archived = false`) by default. A page header shows the title "Assets" and the total active asset count.

**AST-016 (was R16) — Fuzzy search (frontend)**: A search field in the header filters the list in real time on name, reference, class, and category. The search applies only to assets currently displayed: active assets only if the AST-019 toggle is off, both active and archived if the toggle is on. If no result matches, the table displays "No results for this search."

**AST-017 (was R17) — Column sorting (frontend)**: Clicking a sortable column header sorts the list by that column ascending. A second click toggles to descending. Every primary sort breaks ties by name ascending as the secondary key — independent of the primary direction — so rows sharing a primary value stay in alphabetical (default) order.

**AST-008 (was R8) — Creation via FAB (frontend)**: A floating FAB at the bottom right opens the "New asset" dialog, which starts with the kind (AST-040). The form of the kind chosen contains the fields the core says that kind has (AST-037), among: Name (required), Reference (required — the ticker of a listed asset, the symbol of a crypto asset, the reference of a custom one, proposed from its name, AST-038), ISIN (a listed asset only, required), ISO Currency (required), Exchange (a listed asset only, optional — AST-021), Interest bearing (not for a crypto asset), Class (the classes of that kind; not shown when the kind has one class), Category (select, pre-selected to `default-uncategorized`, see AST-002), Risk level (1–5 selector, pre-filled per class, see AST-010). Submission is blocked if name, reference, or currency is missing; every other refusal is the core's, shown beside the actions (AST-014).

**AST-009 (was R9) — Reference duplicate warning (frontend)**: _Retired._ The form no longer warns about a shared reference: saving the same asset twice is refused by the core, naming the existing asset (AST-032, AST-040), and a reference two listings share is expected.

**AST-010 (was R10) — Risk level suggestion at creation (frontend)**: At creation only, the `risk_level` field is pre-filled with the `default_risk` (AST-003) of the class shown — the one the kind preselects, then each one the user selects — and stays editable manually.

**AST-011 (was R11) — Risk badge in the table (frontend)**: The risk level is displayed in the table as a colored badge, one color per level: light green (1), green (2), orange (3), light red (4), red (5).

**AST-012 (was R12) — Asset modification (frontend)**: The Edit button opens a modal showing the same form as creation, pre-filled with the current asset values. The same validation rules apply (AST-008): submission is blocked if name, reference, or currency is missing; every other refusal is the core's. The existing `risk_level` is shown as-is and is never automatically replaced when the class changes — the automatic suggestion (AST-010) does not apply in edit mode. After save, the modal closes and the table refreshes. The form opens on the asset's kind, on its fields and never on the lookup, and lets the user change the kind (AST-031), which is how an asset to settle is settled (AST-035). What the kind chosen does not have is saved empty: no ISIN, no exchange, not bearing interest. No reference is proposed (AST-038).

**AST-013 (was R13) — Asset archival (frontend)**: The Archive button opens a confirmation dialog stating that the asset will be removed from active lists and will no longer receive new prices, but that all historical data is preserved. Confirmation triggers archival (AST-006).

**AST-014 (was R14) — Backend errors (frontend)**: The modal stays open during the backend call and only closes on success. Any failure displays an inline error message in the active modal or dialog. In the asset dialogs the message sits beside the actions, always in view.

**AST-015 (was R15) — Load error state (frontend)**: If the initial list load fails, the table displays an error message with a Retry button.

**AST-019 (was R19) — Archived assets toggle (frontend)**: The header exposes a "Show archived" toggle. When on, archived assets appear in the table with a dimmed visual style on the entire row (not only the badge), making it immediately clear which assets are active vs archived. The Archive button is replaced by an Unarchive button on archived rows; the Edit button is disabled.

**AST-020 (was R20) — Unarchive from the table (frontend)**: The Unarchive button (visible only on archived rows when the AST-019 toggle is on) opens a confirmation dialog. Confirmation triggers unarchival (AST-018) and the asset reappears in the active list.

**AST-021 — Optional exchange picker (frontend)**: The asset creation (AST-008) and edit (AST-012) forms expose an optional `Exchange` picker for a listed asset. The picker lists the canonical curated set (see Entity Definition). Selecting "(none)" submits `exchange = None`. The picker pre-fill behavior from the web-lookup path is defined in WEB-041.

**AST-022 — Exchange persistence (backend)**: The backend accepts the submitted `Exchange` value as-is and persists it without transformation. Editing an asset MAY set, change, or clear `exchange` on a listed asset (subject to AST-005, AST-001, AST-031 and AST-032); an asset of another kind has none.

**AST-023 — Optional ISIN field (backend)**: An asset MAY carry an optional `isin: Option<String>`. When present, `isin` MUST satisfy the ISIN format validation defined in WEB-016 (12 characters, ASCII alphanumeric with letter prefix and digit suffix, Luhn-mod-10 check digit); the trimmed + uppercased form is the value persisted. When absent, the asset has no canonical ISO 6166 identity (typical for non-quoted assets or assets discovered via the keyword path of web lookup). The `isin` field is independent of `reference`: both may be populated for quoted assets discovered via the ISIN path. Whether an asset carries one follows its kind (AST-031): a listed asset always does, a crypto or a custom asset never; editing a listed asset MAY change its `isin` (subject to AST-005, AST-001 and AST-032).

**AST-024 — Interest-bearing opt-in flag (backend)**: An asset carries an `interest_bearing: bool` flag, `false` by default. The flag is set at creation and freely editable afterwards (subject to AST-005), persisted as-is with no validation of its own. It marks the asset as an eligible target for Interest credits: the interest record path rejects a non-cash, non-`interest_bearing` target (INT-012), and the interest modal's asset selector only lists flagged non-cash holdings (INT-020). The account's Cash Asset is always interest-eligible regardless of this flag (INT-023); the flag has no effect on a Cash-class asset.

**AST-037 — The core says what each kind's form asks for (backend)**: For each kind a user may create an asset of — listed, crypto, custom, in that order, never cash — the core states: the classes offered and the one preselected (listed: every class a user may pick but `DigitalAsset`, `Stocks` preselected; crypto: `DigitalAsset` alone; custom: the same classes as listed, `RealEstate` preselected), whether an asset of that kind has an ISIN, may be on an exchange, starts from the lookup (a listed asset does all three; the others none), has its reference proposed from its name (a custom asset), and may be marked as bearing interest (not a crypto asset). The kind preselected is listed. No interface decides any of it.

**AST-038 — A custom asset's reference is proposed from its name (frontend + backend)**: At creation only, while the user has not typed a reference, the reference of a custom asset is the one the core proposes from its name: its letters and digits in capitals, a Latin letter without its accent, every run of anything else as one hyphen ("SCPI Pierval Santé" gives `SCPI-PIERVAL-SANTE`); a name with no letter or digit proposes nothing. The form says under the field that it was proposed and can be changed; once the user types in the field, nothing is proposed again. A reference that was only proposed is dropped when the kind changes to one that proposes none.

**AST-039 — Other listings of the same instrument (frontend + backend)**: Two listed assets that share an ISIN are listings of one instrument (AST-032 makes them two assets). The core reports, for each of them, the other active listings, by exchange label then currency, a listing on no exchange first. The table shows them under the asset's name as "also held as" followed by reference, exchange and currency: one in full; several as the first followed by "…", the whole list being the hint of the line. No figure of the two assets is merged.

**AST-040 — The kind comes first in the asset dialogs (frontend)**: "New asset" opens on the choice of kind — Listed, Crypto, Custom — with one line saying what the chosen kind means; the edit dialog carries the same choice (AST-012). At creation, a listed asset starts from the lookup (WEB-010), with a way to fill it in by hand and, once a result is chosen, a way back to the search (WEB-047); a crypto or a custom asset opens on its form. Changing the kind between two that show their form keeps what was typed, a reference that was only proposed apart (AST-038); choosing Listed at creation returns to the search, and what was typed is not carried there.

**AST-041 — A refusal for the same asset names it (frontend)**: When the core refuses an asset because it already exists (AST-032), the message names the existing asset with its reference, its exchange when it has one, and its currency.

---

## Workflow

```
[User opens "Assets"]
  → Asset table (default sort: Name asc) + FAB
          │
          ├─ [Search] → Real-time fuzzy filter (AST-016)
          ├─ [Click on header] → Ascending/descending sort (AST-017)
          │
          ├─ [FAB] → Creation modal
          │            → Kind first: lookup for a listed asset, form otherwise (AST-040)
          │            → Class preselected or selected → risk_level pre-filled (AST-010)
          │            → The same asset twice is refused, naming the existing one (AST-032)
          │            → Submit → asset created → modal closed → table refreshed
          │
          ├─ [Edit] → Edit modal pre-filled → Modification → table refreshed
          │
          ├─ [Archive] → Dialog (asset removed from active lists, data preserved)
          │              → Confirmation → Archival (AST-006)
          │
          └─ [Archived toggle] → Shows archived assets with Unarchive button (AST-019)
                                  → [Unarchive] → Confirmation dialog → Unarchival (AST-018/AST-020)
```

---

## UX Mockup

### Entry point

**Assets** — item in the main navigation drawer.

### Main component

Full-width table page, sorted by Name ascending by default. Floating FAB at the bottom right. Edit (primary icon) and Archive (archive icon) buttons on each row.

### States

- **Empty**: "No assets. Create your first asset with the + button."
- **Loading**: Loading indicator in the table
- **Load error**: Error message + Retry button (AST-015)
- **Refusal**: beside the actions of the dialog, naming the existing asset when it is the same (AST-041)
- **Archive confirmation**: Dialog explaining that the asset will be removed from active lists and that historical data is preserved (AST-013)
- **Archived assets visible**: Visually distinct rows, Unarchive button instead of Archive, Edit button disabled (AST-019)
- **Unarchive confirmation**: Confirmation dialog before reactivation (AST-020)
- **Backend error**: Inline error message in the active modal or dialog (AST-014)

### User flow — asset creation

1. The user clicks the FAB → the "New asset" dialog opens on the kind (AST-040).
2. They choose a kind; its class is preselected, and where the kind offers several they may select another → `risk_level` automatically pre-filled (AST-010).
3. They fill in the other fields. An asset that already exists is refused on submit, naming it (AST-032).
4. They submit → asset created → modal closed → table refreshed.

### User flow — asset archival

1. The user clicks Archive → dialog explaining the asset will be removed from active lists and that data is preserved.
2. They confirm → archival (AST-006).

---

## Future features

### Hard-delete an asset

Allow physical deletion (hard delete) of an archived asset, only if no operation references it. If operations exist, hard delete is blocked — archival remains the only option. This feature is not in scope for the current implementation.

### Asset operations history

Display, from the Assets page, the list of operations linked to a given asset. Likely entry point: a contextual action on the asset row in the table, opening a panel or dedicated sub-page. This feature will be handled in the `operation` spec.

### Success feedback via snackbar

Display a snackbar notification after mutation operations (creation, modification, archival, unarchival), replacing the simple "modal closed" visual feedback. Requires a snackbar component in the design system first.

---

## Open questions

None — all questions have been resolved.

## Cross-amendments

- **CFR-017** — a transaction or price recorded on another device against an asset archived here is applied as it is; AST-006's "can no longer be modified" binds what a user does to the asset and its prices on a device, not merge (see `sync-conflict-resolution.md`). Transactions on an archived asset have their own rules: a purchase is saved (TRX-028), an opening balance and a sale are refused (TRX-050, SEL-037).
- **SYN-038** — the kind is a field of a synced record: it arrives with data format version 2 (see `multi-device-sync.md`); AST-036 says what a version 1 change becomes.
- **WEB-041 / WEB-046** — a keyword lookup result pre-fills an exchange and no ISIN; saving it as it is is refused (AST-031): the user types the ISIN, or chooses the custom kind (see `asset-web-lookup.md`).
- **CSH-015** — Cash Assets appear in the table as the application's (AST-040) (see `cash-tracking.md`).
- **CLI-026** — `asset add` states a kind or leaves AST-034 to decide it; either way AST-031 and AST-032 refuse what the window refuses (see `command-line.md`).
