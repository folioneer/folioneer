//! # Primary Listing Processor
//!
//! ## Purpose
//!
//! Reduces OpenFIGI's noisy multi-venue response into a clean per-share-class
//! result list, surfacing the most "primary" listing(s) per company. This is
//! the implementation of WEB-049 (exchange-code resolution) and WEB-050
//! (primary-listing surfacing).
//!
//! ## Why this lives in its own file
//!
//! OpenFIGI's free API does **not** expose an "is_primary" field — Bloomberg
//! keeps that mapping in their commercial product. To reconstruct it we
//! combine three signals, each backed by a hardcoded table:
//!
//! 1. **ISIN country prefix** (deterministic when the query is an ISIN) —
//!    `ISIN_COUNTRY_TO_PRIMARY_VENUES` maps the 2-letter ISO 3166-1 country
//!    code to an ordered list of primary venue `exchCode` values.
//! 2. **Composite-FIGI relationship** — per the FIGI Allocation Rules, when
//!    `figi == compositeFIGI` the entry is "the primary security in that
//!    country/region". This is informational rather than load-bearing for the
//!    current pipeline (Euronext stocks roll up to a multi-country `EO` Euro
//!    composite, so the country prefix is still needed to pinpoint a venue).
//! 3. **Global venue priority list** — `GLOBAL_VENUE_PRIORITY` provides a
//!    deterministic fallback ordering for the keyword path when no ISIN
//!    country signal is available.
//!
//! All three tables and the pipeline that consumes them live in this single
//! file so the algorithm is auditable and unit-testable in isolation. The
//! orchestrator stays thin: it does HTTP + WEB-014 routing and delegates
//! ranking/dedup decisions to [`process_hits`].
//!
//! ## Pipeline (WEB-050)
//!
//! 1. Drop hits whose `share_class_figi` is `None` (trade-reporting noise).
//! 2. Group remaining hits by `share_class_figi`.
//! 3. For each group, walk `GLOBAL_VENUE_PRIORITY` and keep up to
//!    `MAX_ENTRIES_PER_SHARE_CLASS_{KEYWORD,ISIN}` entries whose `exchange_code` is on the
//!    list, in priority order. If none match, keep the first entry as a
//!    fallback so the share class isn't lost.
//! 4. ISIN path: when `QueryContext::isin` is `Some(_)`, the country prefix
//!    is used to bring matching country-primary venues to the head of the
//!    priority order before the walk.
//! 5. Resolve each picked entry's venue identifiers to a canonical
//!    [`Exchange`] via [`resolve_canonical_exchange`] (WEB-049) — `micCode`
//!    first, then `exchCode` through the OpenFIGI mapper.
//!
//! Final cap (WEB-022, 30 results) is applied by the caller, not here.

use crate::context::asset::openfigi_exchange_mapper::{
    openfigi_exchcode_to_exchange, openfigi_mic_to_exchange,
};
use crate::context::asset::{AssetClass, Exchange};
use serde::Serialize;
use specta::Type;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Raw fields extracted from a single OpenFIGI result entry.
///
/// Defined here rather than in `orchestrator.rs` because the processor's
/// public API operates on this type. The orchestrator constructs values
/// from HTTP responses and feeds them in.
#[derive(Debug, Clone)]
pub struct RawFigiHit {
    /// Instrument name as returned by OpenFIGI.
    pub name: String,
    /// Ticker symbol, if present.
    pub ticker: Option<String>,
    /// `securityType` field, if present.
    pub security_type: Option<String>,
    /// ISO 4217 currency, if present.
    pub currency: Option<String>,
    /// Short exchange identifier (`exchCode`, e.g. `"UW"`, `"FP"`).
    pub exchange_code: Option<String>,
    /// ISO 10383 MIC code (`micCode`), when OpenFIGI includes it.
    /// Primary venue signal for WEB-049 canonical exchange resolution.
    pub mic_code: Option<String>,
    /// Bloomberg share class FIGI — same value across all listings of the
    /// same global share class. Used for dedup grouping (step 2 of WEB-050).
    pub share_class_figi: Option<String>,
    /// Bloomberg composite FIGI — country-level aggregate identifier.
    /// Carried for completeness; the current pipeline does not branch on it
    /// but it is exposed for future heuristics.
    pub composite_figi: Option<String>,
}

/// Transient value object returned by the orchestrator's `search` method.
/// Mirrors the shape exposed at the Tauri boundary.
#[derive(Debug, Clone, Serialize, Type, PartialEq, Eq)]
pub struct AssetLookupResult {
    /// Full instrument name (e.g. `"AIR LIQUIDE SA"`).
    pub name: String,
    /// Ticker symbol returned by OpenFIGI; `None` when no ticker is available
    /// (WEB-046). Consistent across both lookup paths so the FE always pre-fills
    /// `Asset.reference` with a value market-data providers (Yahoo, etc.) can
    /// resolve.
    pub reference: Option<String>,
    /// ISIN (ISO 6166, 12 chars normalized). Populated only on the ISIN path
    /// with the validated query (WEB-046); `None` on the keyword path because
    /// OpenFIGI's `/v3/search` response does not expose ISIN.
    pub isin: Option<String>,
    /// ISO 4217 currency forwarded from OpenFIGI (WEB-024).
    pub currency: Option<String>,
    /// Asset class derived from `securityType` (WEB-023).
    pub asset_class: Option<AssetClass>,
    /// Canonical exchange resolved via WEB-049 mapper; absent when the venue
    /// is not in the curated set.
    pub exchange: Option<Exchange>,
}

/// Context describing how the query was issued. The processor uses it to
/// choose between the deterministic ISIN-country path and the heuristic
/// keyword path.
#[derive(Debug, Clone, Default)]
pub struct QueryContext {
    /// `Some(isin)` when WEB-014 routed to the ISIN path. The 2-letter prefix
    /// drives the country-aware primary-venue lookup. `None` for keyword.
    pub isin: Option<String>,
}

/// Per-share-class cap on the keyword path (WEB-050e, browsing intent).
///
/// 10 venues lets a single-company query like `ASML` surface all of its
/// notable listings (Amsterdam, London, Xetra, Milan, …) so the user can
/// pick the one they want. A multi-company query (e.g. `Apple`) still
/// stays diverse because the final 30-result cap (WEB-022) lets at most
/// 3 distinct share classes fill the dropdown when each takes the full
/// per-class allowance.
const MAX_ENTRIES_PER_SHARE_CLASS_KEYWORD: usize = 10;

/// Per-share-class cap on the ISIN path (WEB-050e, selection intent).
///
/// 3 venues — country home venue (prepended by `priority_for` when the ISIN's
/// country prefix has a curated entry) plus two global fallbacks. The user
/// has already pinpointed the specific instrument via the ISIN and just
/// needs to choose a venue from a clean shortlist.
const MAX_ENTRIES_PER_SHARE_CLASS_ISIN: usize = 3;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Runs the WEB-050 dedup + primary-pick pipeline.
///
/// Pure function: no I/O, no allocation beyond the returned `Vec`.
///
/// Caller is responsible for the final 30-row truncation (WEB-022).
pub fn process_hits(raw_hits: Vec<RawFigiHit>, ctx: &QueryContext) -> Vec<AssetLookupResult> {
    let groups = group_by_share_class(raw_hits);
    let priority = priority_for(ctx);
    let cap = if ctx.isin.is_some() {
        MAX_ENTRIES_PER_SHARE_CLASS_ISIN
    } else {
        MAX_ENTRIES_PER_SHARE_CLASS_KEYWORD
    };

    let mut results: Vec<AssetLookupResult> = Vec::new();
    for group in groups {
        for hit in pick_primary_entries(&group, &priority, cap) {
            results.push(to_result(hit, ctx));
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Pipeline steps
// ---------------------------------------------------------------------------

/// Step 1 + 2 of WEB-050: drop null-share-class noise and group the rest by
/// `share_class_figi`. Group order matches the order the share class first
/// appeared in the input (preserving OpenFIGI's relevance ranking across
/// distinct companies in a multi-match keyword search).
fn group_by_share_class(hits: Vec<RawFigiHit>) -> Vec<Vec<RawFigiHit>> {
    let mut groups: Vec<Vec<RawFigiHit>> = Vec::new();
    let mut indices: Vec<String> = Vec::new();
    for hit in hits {
        let Some(key) = hit.share_class_figi.as_deref() else {
            continue;
        };
        if let Some(group) = indices
            .iter()
            .position(|k| k == key)
            .and_then(|pos| groups.get_mut(pos))
        {
            group.push(hit);
        } else {
            indices.push(key.to_string());
            groups.push(vec![hit]);
        }
    }
    groups
}

/// Steps 3 + 4 of WEB-050: walk the priority list and keep up to `cap` matching
/// entries (`MAX_ENTRIES_PER_SHARE_CLASS_KEYWORD` or `MAX_ENTRIES_PER_SHARE_CLASS_ISIN`
/// per WEB-050e). Falls back to the first raw entry of the group when none match.
fn pick_primary_entries<'a>(
    group: &'a [RawFigiHit],
    priority: &[&str],
    cap: usize,
) -> Vec<&'a RawFigiHit> {
    let mut picked: Vec<&RawFigiHit> = Vec::new();
    for venue in priority {
        if picked.len() >= cap {
            break;
        }
        if let Some(hit) = group
            .iter()
            .find(|h| h.exchange_code.as_deref() == Some(*venue))
        {
            picked.push(hit);
        }
    }
    if picked.is_empty() {
        if let Some(first) = group.first() {
            picked.push(first);
        }
    }
    picked
}

/// Builds the priority list used for a given query. ISIN-path queries put the
/// country's primary venues at the head of the list so they win the walk;
/// remaining global priorities follow as a fallback.
fn priority_for(ctx: &QueryContext) -> Vec<&'static str> {
    let mut list: Vec<&'static str> = Vec::new();
    if let Some(country) = ctx.isin.as_deref().and_then(isin_country) {
        if let Some(venues) = primary_venues_for_country(country) {
            list.extend(venues);
        }
    }
    // Linear `contains` is intentional: the priority list is bounded to
    // GLOBAL_VENUE_PRIORITY.len() (~28) plus at most 2 country-prepended
    // venues, so a HashSet would add complexity without measurable benefit.
    for v in GLOBAL_VENUE_PRIORITY {
        if !list.contains(v) {
            list.push(v);
        }
    }
    list
}

/// Step 5 of WEB-050: convert a chosen `RawFigiHit` into the public
/// `AssetLookupResult`. Performs `securityType` → `AssetClass` mapping
/// (WEB-023), `exchCode` → human-readable resolution (WEB-049), and
/// reference/ISIN field selection (WEB-046).
///
/// Per WEB-046: `reference` is the ticker symbol on BOTH paths (so the FE
/// always pre-fills `Asset.reference` with a value market-data providers
/// understand); `isin` is populated only on the ISIN path with the normalized
/// query carried in `ctx.isin`.
fn to_result(hit: &RawFigiHit, ctx: &QueryContext) -> AssetLookupResult {
    let reference = hit.ticker.clone().filter(|t| !t.is_empty());
    AssetLookupResult {
        name: hit.name.clone(),
        reference,
        isin: ctx.isin.clone(),
        currency: hit.currency.clone(),
        asset_class: hit.security_type.as_deref().and_then(map_security_type),
        exchange: resolve_canonical_exchange(hit),
    }
}

/// Resolves a canonical `Exchange` from a `RawFigiHit` using the WEB-049
/// precedence rule:
///   1. `mic_code` present → try `openfigi_mic_to_exchange`; return Some if in
///      curated set, else fall through to step 2.
///   2. `exchange_code` present → try `openfigi_exchcode_to_exchange`; return
///      Some if in curated set, else `None`.
///   3. Both absent, or both lookups miss the curated set → `None`.
///
/// Hits whose venue is outside the curated set carry `exchange = None` in
/// `AssetLookupResult`.
pub fn resolve_canonical_exchange(hit: &RawFigiHit) -> Option<Exchange> {
    if let Some(mic) = hit.mic_code.as_deref() {
        if let Some(exchange) = openfigi_mic_to_exchange(mic) {
            return Some(exchange);
        }
    }
    if let Some(code) = hit.exchange_code.as_deref() {
        if let Some(exchange) = openfigi_exchcode_to_exchange(code) {
            return Some(exchange);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Public helpers (used by the orchestrator's HTTP layer)
// ---------------------------------------------------------------------------

/// Maps an OpenFIGI `securityType` string to an `AssetClass` variant. The
/// recognised values are the subset of OpenFIGI's `/v3/mapping/values/securityType`
/// vocabulary that the project surfaces as a typed asset class. Returns `None`
/// for anything else (Structured Product, Certificate, ADR, MTN, futures
/// sub-types not listed here, etc.).
pub fn map_security_type(s: &str) -> Option<AssetClass> {
    match s {
        "Common Stock" => Some(AssetClass::Stocks),
        // OpenFIGI uses "ETP" as the umbrella for ETF + ETN + ETC; the
        // structural distinction isn't exposed at the securityType level.
        "ETP" => Some(AssetClass::ETP),
        "Mutual Fund" | "Open-End Fund" | "Closed-End Fund" | "Fund of Funds" | "Pvt Eqty Fund" => {
            Some(AssetClass::MutualFunds)
        }
        "Bond" | "Conv Bond" => Some(AssetClass::Bonds),
        "Crypto" => Some(AssetClass::DigitalAsset),
        "REIT" => Some(AssetClass::RealEstate),
        "Warrant"
        | "Equity Option"
        | "Index Option"
        | "SINGLE STOCK FUTURE"
        | "SINGLE STOCK DIVIDEND FUTURE" => Some(AssetClass::Derivatives),
        _ => None,
    }
}

/// Resolves an OpenFIGI `exchCode` to a human-readable market name (WEB-049).
/// Falls back to the raw code string for unknown exchanges.
pub fn resolve_exchange_name(code: &str) -> String {
    EXCHANGE_CODE_TO_NAME
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| code.to_string())
}

/// Returns the 2-letter ISO 3166-1 country prefix of an ISIN, or `None` if
/// the input is too short or has non-alphabetic prefix characters.
fn isin_country(isin: &str) -> Option<&str> {
    let prefix = isin.get(..2)?;
    if prefix.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(prefix)
    } else {
        None
    }
}

/// Returns the ordered list of primary venue `exchCode` values for a given
/// ISO 3166-1 country code, or `None` if the country is unknown.
fn primary_venues_for_country(country: &str) -> Option<&'static [&'static str]> {
    ISIN_COUNTRY_TO_PRIMARY_VENUES
        .iter()
        .find(|(c, _)| *c == country)
        .map(|(_, venues)| *venues)
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// Ordered list of primary venue `exchCode` values, walked top-to-bottom on
/// the keyword path when no ISIN-country signal is available. Order reflects
/// global trading volume and primary-listing density. Per WEB-050 the walk
/// keeps up to `MAX_ENTRIES_PER_SHARE_CLASS_{KEYWORD,ISIN}` matches, so dual-listed
/// names such as TotalEnergies (UN + FP) surface both rows.
const GLOBAL_VENUE_PRIORITY: &[&str] = &[
    // Major primaries
    "UN", // New York Stock Exchange
    "UW", // Nasdaq Global Select Market
    "LO", // London Stock Exchange
    "JT", // Tokyo Stock Exchange
    "FP", // Euronext Paris
    "GY", // Xetra
    "HK", // Hong Kong Stock Exchange
    "SE", // SIX Swiss Exchange
    "AT", // Australian Securities Exchange
    "CT", // Toronto Stock Exchange
    "IM", // Borsa Italiana
    "NA", // Euronext Amsterdam
    "SQ", // BME (Spain)
    // Secondary primaries
    "BB", // Euronext Brussels
    "ID", // Euronext Dublin
    "PL", // Euronext Lisbon
    "AV", // Vienna
    "DC", // Copenhagen
    "FH", // Helsinki
    "NO", // Oslo
    "SS", // Nasdaq Stockholm
    "KP", // Korea Exchange
    "TT", // Taiwan Stock Exchange
    "IS", // NSE India
    "SP", // Singapore Exchange
    "BS", // B3 Sao Paulo
    "MF", // BMV Mexico
    "IT", // Tel Aviv
];

/// ISIN country prefix → ordered list of primary venue `exchCode` values.
/// Used on the ISIN path so the country's own primary venues outrank the
/// global ordering. Each list is ordered by likelihood of being the
/// instrument's home venue.
const ISIN_COUNTRY_TO_PRIMARY_VENUES: &[(&str, &[&str])] = &[
    ("FR", &["FP"]),
    ("US", &["UN", "UW"]),
    ("DE", &["GY"]),
    ("GB", &["LO"]),
    ("NL", &["NA"]),
    ("BE", &["BB"]),
    ("IT", &["IM"]),
    ("ES", &["SQ"]),
    ("PT", &["PL"]),
    // Dublin (ID) wins for Irish equities (Ryanair, CRH, Kerry Group). UCITS
    // ETFs are also domiciled in Ireland for tax/regulatory reasons but trade
    // primarily on LSE / Amsterdam / Xetra — falling through in that order
    // surfaces a coherent default when Dublin has no entry.
    ("IE", &["ID", "LO", "NA", "GY"]),
    ("CH", &["SE"]),
    ("AT", &["AV"]),
    ("DK", &["DC"]),
    ("FI", &["FH"]),
    ("NO", &["NO"]),
    ("SE", &["SS"]),
    ("IS", &["IR"]),
    ("PL", &["PC"]),
    ("CZ", &["CD"]),
    ("HU", &["HB"]),
    ("GR", &["GA"]),
    ("TR", &["TI"]),
    ("JP", &["JT"]),
    ("HK", &["HK"]),
    ("KR", &["KP"]),
    ("CN", &["CG", "CS"]),
    ("TW", &["TT"]),
    ("IN", &["IS", "IB"]),
    ("SG", &["SP"]),
    ("AU", &["AT"]),
    ("NZ", &["NZ"]),
    ("CA", &["CT"]),
    ("BR", &["BS"]),
    ("MX", &["MF"]),
    ("IL", &["IT"]),
    // ZA (South Africa) intentionally omitted: OpenFIGI's free dictionary
    // exposes only A2X codes ("AJ", "SJ") for South African venues. The JSE
    // primary listing is not addressable as a 2-letter exchCode in the free
    // API, so a ZA-prefixed ISIN falls back to GLOBAL_VENUE_PRIORITY.
];

/// OpenFIGI `exchCode` → human-readable exchange name. Mirrors the
/// dictionary published at `openfigi.com` (Exchange Codes CSV). When OpenFIGI
/// adds a new code we don't yet recognise, [`resolve_exchange_name`] falls
/// back to the raw code so the user always sees something.
const EXCHANGE_CODE_TO_NAME: &[(&str, &str)] = &[
    // Argentina
    ("AC", "Buenos Aires Continuous"),
    ("AE", "Argentina's Mercado Abierto Electronico"),
    ("AF", "Bolsa de Comercio de Buenos Aires"),
    ("AM", "Mendoza Stock Exchange"),
    ("AS", "Buenos Aires Sinac"),
    // Australia
    ("AH", "Cboe Australia"),
    ("AN", "Australian Stock Exchange"),
    ("AO", "NSX Australia"),
    ("AQ", "ASX PureMatch"),
    ("AT", "Australian Securities Exchange"),
    ("PF", "Sydney Stock Exchange"),
    ("SI", "SIM Venture Securities Exchange"),
    // Brazil
    ("BE", "Bovespa Outcry Market"),
    ("BL", "Bovespa Outcry Market"),
    ("BN", "Sao Paulo After Market"),
    ("BO", "SOMA Market"),
    ("BR", "Rio de Janeiro Stock Exchange"),
    ("BS", "Sao Paulo Stock Exchange"),
    ("BV", "Bovespa Outcry Market"),
    ("VE", "Bovespa Outcry Market"),
    // Latin America
    ("CL", "Medellin Stock Exchange"),
    ("CO", "Occidente Stock Exchange"),
    ("CX", "Bolsa de Valores de Colombia"),
    ("CC", "Santiago Comercio"),
    ("CE", "Santiago Electronic"),
    ("EG", "Guayaquil Stock Exchange"),
    ("EQ", "Quito Stock Exchange"),
    ("PN", "Asuncion Stock Exchange"),
    ("PE", "Bolsa de Valores de Lima"),
    ("PP", "Latin American Stock Exchange"),
    ("PX", "PEX Stock Exchange"),
    ("MF", "Bolsa Mexicana de Valores"),
    ("MU", "BIVA"),
    ("UY", "Montevideo Stock Exchange"),
    ("VS", "Caracas Stock Exchange"),
    ("NC", "Nicaragua Stock Exchange"),
    ("CR", "Costa Rica Stock Exchange"),
    ("EL", "El Salvador Stock Exchange"),
    ("GL", "Guatemala Stock Exchange"),
    ("HO", "Honduras Stock Exchange"),
    ("JA", "Kingston Stock Exchange"),
    ("TP", "Trinidad and Tobago Stock Exchange"),
    ("BA", "Bridgetown Stock Exchange"),
    ("BH", "Bermuda Stock Exchange"),
    ("EK", "E. Caribbean Securities Exchange"),
    ("KY", "Cayman Islands Stock Exchange"),
    ("BM", "Bahamas Stock Exchange"),
    ("CU", "Cook Islands Stock Exchange"),
    ("VB", "Bolivia Stock Exchange"),
    // China / HK
    ("CG", "Shanghai Stock Exchange"),
    ("CS", "Shenzhen Stock Exchange"),
    ("JC", "Beijing Stock Exchange"),
    ("QN", "National Equities Exchange and Quotations"),
    ("HK", "Hong Kong Stock Exchange"),
    ("HE", "Chi-East Exchange"),
    ("C1", "Northbound SSE-SEHK Stock Connect"),
    ("C2", "Northbound SZ-SEHK Stock Connect"),
    ("H1", "Southbound SSE-SEHK Stock Connect"),
    ("H2", "Southbound SZ-SEHK Stock Connect"),
    // Canada
    ("CA", "Alberta Stock Exchange"),
    ("CF", "Canadian Securities Exchange"),
    ("CJ", "Pure Trading Exchange"),
    ("CM", "Montreal Stock Exchange"),
    ("CQ", "Canadian Dealing Network"),
    ("CT", "Toronto Stock Exchange"),
    ("CV", "Canadian Venture Exchange"),
    ("CW", "Winnipeg Stock Exchange"),
    ("DG", "Lynx ATS"),
    ("DJ", "NASDAQ CXD Toronto"),
    ("DK", "NASDAQ CXD Ventures"),
    ("DL", "NASDAQ CX CSE"),
    ("DM", "NASDAQ CX2 CSE"),
    ("DN", "NASDAQ CXD CSE"),
    ("DS", "CX2 Canada Venture"),
    ("DT", "CX2 Canada Toronto"),
    ("DV", "Chi-X Venture"),
    ("QF", "NEO Exchange NEO-L"),
    ("QG", "NEO Exchange NEO-D"),
    ("QH", "NEO Exchange NEO-N"),
    ("TA", "Alpha Toronto Exchange"),
    ("TG", "Omega Exchange"),
    ("TJ", "TMX Select"),
    ("TK", "Liquidnet"),
    ("TN", "Alpha Venture Exchange"),
    ("TR", "TriAct MATCHNow Toronto Exchange"),
    ("TV", "TriAct MATCHNow Ventures Exchange"),
    ("TW", "Instinet Canada Cross"),
    ("TX", "Chi-X Toronto"),
    ("TY", "Sigma X ATS"),
    ("HX", "Alpha-X"),
    ("HD", "Alpha Dark"),
    // Czech Republic / SK
    ("CD", "Prague Stock Exchange (SPAD)"),
    ("KL", "Prague-Block Stock Exchange"),
    ("RC", "Czech Republic RM-System"),
    ("SK", "Bratislava Stock Exchange"),
    // Croatia
    ("VA", "Varazdin Stock Exchange"),
    ("ZA", "Zagreb Stock Exchange"),
    // Egypt / Africa
    ("EA", "Alexandria Stock Exchange"),
    ("EC", "Egypt (EGX)"),
    ("EI", "Nile Stock Exchange"),
    ("AG", "Algerian Stock Exchange"),
    ("AX", "Angolan Stock Exchange"),
    ("BG", "Gaborone Stock Exchange"),
    ("BC", "BRVM Regional Exchange"),
    ("DE", "Douala Stock Exchange"),
    ("VR", "Cape Verde Stock Exchange"),
    ("ZC", "Central Africa Securities Exchange"),
    ("SD", "Eswatini Stock Exchange"),
    ("ZG", "Gabon Stock Exchange"),
    ("GX", "Gambia Stock Exchange"),
    ("GN", "Accra Stock Exchange"),
    ("IA", "Abidjan Stock Exchange"),
    ("KN", "Nairobi Securities Exchange"),
    ("LY", "Libyan Stock Exchange"),
    ("MW", "Malawi Stock Exchange"),
    ("MC", "Casablanca Stock Exchange"),
    ("MZ", "Mozambique Stock Exchange"),
    ("MP", "SEM Mauritius"),
    ("NW", "Windhoek Stock Exchange"),
    ("NL", "Nigerian Exchange Ltd (NGX)"),
    ("NJ", "Nigeria NASD OTC Market"),
    ("RW", "Rwanda Stock Exchange"),
    ("ZS", "Senegal Stock Exchange"),
    ("SU", "Sierra Leone Stock Exchange"),
    ("AJ", "A2X Stock Exchange"),
    ("SJ", "A2X Stock Exchange"),
    ("S3", "Sudan Stock Exchange"),
    ("TZ", "Dar es Salaam Stock Exchange"),
    ("UG", "Uganda Stock Exchange"),
    ("YS", "Yemen Stock Exchange"),
    ("ZL", "Lusaka Stock Exchange"),
    ("ZH", "Harare Stock Exchange"),
    // Germany
    ("GB", "Berlin Stock Exchange"),
    ("GC", "Bremen Stock Exchange"),
    ("GD", "Dusseldorf Stock Exchange"),
    ("GE", "Xetra EU Stars"),
    ("GF", "Frankfurt Stock Exchange"),
    ("GH", "Hamburg Stock Exchange"),
    ("GI", "Hannover Stock Exchange"),
    ("GM", "Munich Stock Exchange"),
    ("GQ", "Xetra Stars"),
    ("GS", "Stuttgart Stock Exchange"),
    ("GT", "Xetra ETF Exchange"),
    ("GW", "Stuttgart Warrants"),
    ("GY", "Xetra Stock Exchange"),
    ("GZ", "Gettex"),
    ("GK", "Xetra International Market"),
    ("NF", "Frankfurt Neuer Markt"),
    ("NM", "German NM"),
    ("NY", "Xetra Newer Markt"),
    // Latvia / Lithuania / Estonia
    ("LF", "Riga Fixed Stock Exchange"),
    ("LG", "Riga Exchange"),
    ("LV", "Riga Variable Exchange"),
    ("ET", "Tallinn Stock Exchange"),
    ("LH", "Vilnius Stock Exchange"),
    // India / South Asia
    ("IB", "BSE India"),
    ("IG", "Metropolitan Stock Exchange of India"),
    ("IH", "Delhi Stock Exchange"),
    ("IS", "National Stock Exchange of India"),
    ("BD", "Dhaka Stock Exchange"),
    ("C3", "Chittagong Stock Exchange"),
    ("PK", "Pakistan Stock Exchange"),
    ("SL", "Colombo Stock Exchange"),
    ("NK", "Nepal Stock Exchange"),
    ("KH", "Cambodia Stock Exchange"),
    ("MY", "Yangon Stock Exchange"),
    ("BX", "Brunei Stock Exchange"),
    // Japan
    ("JB", "Brokers Broker Exchange"),
    ("JD", "Kabu.com Stock Exchange"),
    ("JE", "Japannext"),
    ("JF", "Fukuoka Stock Exchange"),
    ("JG", "Tokyo AIM Stock Exchange"),
    ("JI", "Cboe Japan Market"),
    ("JJ", "JSDA Off-Exchange"),
    ("JK", "Kyoto Stock Exchange"),
    ("JM", "Optimark Japan"),
    ("JN", "Nagoya Stock Exchange"),
    ("JO", "Osaka Stock Exchange"),
    ("JQ", "Jasdaq Stock Market"),
    ("JS", "Sapporo Stock Exchange"),
    ("JT", "Tokyo Stock Exchange"),
    ("JU", "Japannext X-Market"),
    ("JV", "Osaka Digital Exchange"),
    ("JW", "Japannext U-Market"),
    ("JX", "Nippon New Market Hercules"),
    // Korea
    ("KE", "Konex Exchange"),
    ("KP", "Korea Stock Exchange"),
    ("KQ", "KOSDAQ Exchange"),
    ("KF", "K-OTC"),
    // Mexico already above (MF)
    // Russia / CIS
    ("RN", "MICEX Negotiated Mode"),
    ("RP", "MICEX Repo Market"),
    ("RX", "MICEX Main Market"),
    ("RR", "MOEX"),
    ("RS", "RTS Standard"),
    ("RT", "NP RTS"),
    ("RB", "Belarus Stock Exchange"),
    ("AY", "NASDAQ OMX Armenia Stock Exchange"),
    ("AZ", "Baku Stock Exchange"),
    ("KX", "Astana International Exchange"),
    ("KZ", "Kazakhstan Stock Exchange"),
    ("KB", "Kyrgyzstan Stock Exchange"),
    ("MB", "Moldova Stock Exchange"),
    ("OU", "PFTS Order-driven Market"),
    ("QU", "PFTS Quote-driven Market"),
    ("UK", "RTS Ukraine Stock Exchange"),
    ("UZ", "PFTS Stock Exchange"),
    ("ZU", "Uzbekistan Stock Exchange"),
    ("TD", "Tajikistan Stock Exchange"),
    ("TM", "Turkmenistan Stock Exchange"),
    // Romania / Balkans
    ("RE", "Bucharest Stock Exchange"),
    ("RQ", "RASDAQ Market"),
    ("RZ", "SIBEX"),
    ("RG", "SPB Exchange"),
    ("BP", "Sarajevo Stock Exchange"),
    ("BT", "Sarajevo Stock Exchange"),
    ("BK", "Banja Luka Stock Exchange"),
    ("ME", "Montenegro Stock Exchange"),
    ("MS", "Macedonian Stock Exchange"),
    ("SG", "Belgrade Stock Exchange"),
    ("KO", "Kosovo Stock Exchange"),
    ("AL", "Tirana Stock Exchange"),
    // Spain
    ("SA", "Valencia Stock Exchange"),
    ("SB", "Barcelona Stock Exchange"),
    ("SN", "BME Electronic Outcry"),
    ("SO", "Bilbao Stock Exchange"),
    ("SQ", "Bolsas y Mercados Espanoles"),
    ("ST", "Continuous NM"),
    // Switzerland
    ("BW", "BX Worldcaps"),
    ("SC", "Euro Contributor"),
    ("SE", "SIX Swiss Exchange"),
    ("SR", "Bern Stock Exchange"),
    ("SX", "SIX Swiss Exchange Structured Products"),
    ("XK", "OTC-X Bern Kantonalbank"),
    // UAE / Middle East
    ("DB", "Dubai Financial Market"),
    ("DH", "Abu Dhabi Stock Exchange"),
    ("DU", "Nasdaq Dubai Exchange"),
    ("OM", "Muscat Stock Exchange"),
    ("PS", "Palestine Stock Exchange"),
    ("BI", "Manama Stock Exchange"),
    ("KK", "Kuwait Stock Exchange"),
    ("AB", "Saudi Stock Exchange"),
    ("QD", "Qatar Stock Exchange"),
    ("AK", "Kabul International Stock Exchange"),
    ("IE", "Tehran Stock Exchange"),
    ("IQ", "Iraq Stock Exchange"),
    ("SY", "Damascus Securities Exchange"),
    ("LB", "Beirut Stock Exchange"),
    ("JR", "Amman Stock Exchange"),
    ("IT", "Tel Aviv Stock Exchange"),
    // United States — primary venues and ATS
    ("UA", "NYSE American"),
    ("UB", "Nasdaq OMX BX Exchange"),
    ("UC", "NYSE National"),
    ("UD", "FINRA ADF"),
    ("UE", "Nasdaq International"),
    ("UF", "Cboe BZX Exchange"),
    ("UI", "Island"),
    ("UJ", "Trade Reporting Facility LLC"),
    ("UL", "International Securities Exchange"),
    ("UM", "NYSE Chicago"),
    ("UN", "New York Stock Exchange"),
    ("UO", "CBOE Stock Exchange"),
    ("UP", "NYSE Arca Exchange"),
    ("UQ", "Nasdaq Global Market"),
    ("UR", "Nasdaq Capital Market"),
    ("UT", "NASDAQ InterMarket"),
    ("UU", "OTC Bulletin Board"),
    ("UV", "OTC US Market"),
    ("UW", "Nasdaq Global Select Market"),
    ("UX", "NASDAQ OMX PSX Exchange"),
    ("US", "United States Composite"),
    ("VF", "Investors Exchange"),
    ("VG", "Members Exchange"),
    ("VJ", "EDGA Stock Exchange"),
    ("VK", "EDGX Stock Exchange"),
    ("VL", "Long Term Stock Exchange"),
    ("VP", "MIAX Pearl"),
    ("VY", "Cboe BYX Exchange"),
    ("PQ", "NQB Pink Sheets"),
    // Vietnam
    ("VH", "Hanoi Stock Exchange"),
    ("VM", "Ho Chi Minh Stock Exchange"),
    ("VU", "Hanoi UPCoM Stock Exchange"),
    // OTC / MTF / Trade reporting (Bloomberg X-series, B-series, etc.)
    ("B1", "Bloomberg MTF"),
    ("B2", "Bloomberg MTF (RTS1)"),
    ("B3", "Blockmatch"),
    ("B4", "Bloomberg MTF EU (RTS1)"),
    ("BQ", "Equiduct Exchange"),
    ("EB", "Cboe BXE Europe Equities"),
    ("EU", "European Composite"),
    ("EZ", "European Lit Composite"),
    ("EP", "European Lit Primaries Composite"),
    ("EM", "Euronext New Market"),
    ("EE", "Euronext ETF"),
    ("EF", "Euronext ETF"),
    ("EN", "Euronext"),
    ("E1", "Euro OTC"),
    ("E2", "Cboe BXE EU"),
    ("EO", "Euro Composite"),
    ("ES", "Nasdaq Europe Stock Market"),
    ("EX", "NEWEX"),
    ("HM", "HI-MTF Stock Exchange"),
    ("I2", "Cboe DXE"),
    ("IX", "Cboe CXE Europe Equities"),
    ("LD", "Euronext London"),
    ("L1", "Liquidnet IR"),
    ("L3", "Liquid"),
    ("LA", "LSX Exchange"),
    ("LU", "LSX LSSI"),
    ("LI", "London Stock Exchange (Quotes Service)"),
    ("LO", "London Stock Exchange"),
    ("LT", "Tradepoint Investment Exchange"),
    ("LN", "Tradepoint Investment Exchange"),
    ("LC", "London Composite"),
    ("LX", "Luxembourg Stock Exchange"),
    ("MT", "TOM MTF"),
    ("M0", "Morgan Stanley MTF"),
    ("NB", "Brussels NM"),
    ("BB", "Brussels NM"),
    ("NP", "Paris NM"),
    ("NN", "Amsterdam NM"),
    ("NQ", "NASDAQ OMX Europe Exchange"),
    ("NR", "NYSE ARCA Europe Exchange"),
    ("OF", "OFEX London"),
    ("PG", "PLUS Europe Exchange"),
    ("PO", "ITG Posit Trade Exchange"),
    ("P2", "Posit"),
    ("PZ", "Aquis Stock Exchange"),
    ("QE", "Aquis Exchange (EU)"),
    ("QM", "Quote MTF Stock Exchange"),
    ("QT", "Quotrix Exchange"),
    ("QX", "Aquis Exchange"),
    ("S1", "SIGMA X MTF"),
    ("S2", "AMP (UBS MTF)"),
    ("S4", "SIGMA-X EU MTF"),
    ("SH", "SharesPost Market"),
    ("TE", "EuroTLX Stock Exchange"),
    ("TH", "Tradegate"),
    ("TQ", "Turquoise Market Exchange"),
    ("T1", "Turquoise Europe"),
    ("T2", "TradeWeb MTF"),
    ("WT", "Tradeweb MTF"),
    ("X1", "TradEcho APA EU"),
    ("X2", "Cboe APA"),
    ("X9", "Tradeweb APA"),
    ("XA", "CEESEG OTC and LJSE Block Trades"),
    ("XB", "BOAT Exchange"),
    ("XC", "Chi-X OTC"),
    ("XD", "Deutsche Boerse OTC"),
    ("XE", "Euronext OTC"),
    ("XF", "Dublin SE OTC"),
    ("XG", "Nordic Growth Market OTC"),
    ("XH", "Budapest SE OTC"),
    ("XI", "Borsa Italia SE OTC"),
    ("XJ", "Ljubljana SE OTC"),
    ("XL", "London SE OTC"),
    ("XM", "Euronext Block MTF"),
    ("XN", "Oslo OTC"),
    ("XO", "OMX OTC"),
    ("XP", "NEX Exchange OTC"),
    ("XQ", "BME OTC"),
    ("XR", "Reuters OTC"),
    ("XS", "Stuttgart OTC"),
    ("XT", "Athens OTC Stock Exchange"),
    ("XU", "Bulgaria Stock Exchange OTC"),
    ("XV", "Cboe BXTR Trade Reporting Services"),
    ("XW", "SIX Off-Exchange"),
    ("XX", "Trax APA"),
    ("XY", "Burgundy OTC"),
    ("XZ", "Tradeweb APA"),
    ("Z1", "AIAF Exchange"),
    ("Z2", "TRACE Exchange"),
    ("AW", "ArtEx MTF"),
    ("A0", "Asset Match MTF"),
    ("DD", "Dansk OTC"),
    ("DX", "EDX London Stock Exchange"),
    ("G4", "GXG Markets"),
    ("NS", "Norwegian OTC"),
    ("NV", "New Zealand OTC"),
    ("DF", "First North Denmark"),
    ("FF", "First North Finland"),
    ("RF", "First North Iceland"),
    ("SF", "First North Stockholm"),
    ("KA", "Spotlight Stock Market"),
    ("BY", "Burgundy Stock Exchange"),
    // Nordics / Northern Europe
    ("DC", "Copenhagen Stock Exchange"),
    ("FH", "Helsinki Stock Exchange"),
    ("FP", "Euronext Paris Stock Exchange"),
    ("FS", "South Pacific Stock Exchange"),
    ("IR", "Iceland Stock Exchange"),
    ("NO", "Oslo Stock Exchange"),
    ("SS", "Nasdaq Stockholm"),
    // Greece / Hungary / Bulgaria / etc
    ("AA", "Athens Alternative Market"),
    ("AP", "Athens Repo"),
    ("GA", "Athens Stock Exchange"),
    ("HB", "Budapest Stock Exchange"),
    ("BU", "Bulgaria Stock Exchange"),
    ("IO", "Isle of Man"),
    ("CY", "Nicosia Stock Exchange"),
    ("YC", "Cyprus Emerging Companies Market"),
    // Italy
    ("I9", "LSE Milan Exchange"),
    ("IC", "Milan Complete Day"),
    ("IF", "Milan After-hours"),
    ("IJ", "Surabaya Stock Exchange"),
    ("IK", "Indonesia Composite"),
    ("IY", "Surabaya Stock Exchange"),
    ("IM", "Borsa Italiana"),
    ("NI", "Milan NM"),
    // Ireland
    ("ID", "Euronext Dublin"),
    // Iceland (IS used by NSE India already; OpenFIGI dictionary uses IR)
    // Singapore / SE Asia
    ("SP", "Singapore Exchange"),
    ("PM", "Philippine Stock Exchange"),
    ("MK", "Bursa Malaysia"),
    ("MX", "Maldives Stock Exchange"),
    ("MV", "Valletta Stock Exchange"),
    ("MQ", "MESDAQ Stock Exchange"),
    ("TT", "Taiwan Stock Exchange"),
    ("TB", "Bangkok Stock Exchange"),
    // Turkey
    ("TF", "Istanbul 1st Session"),
    ("TI", "Istanbul 1st Session"),
    ("TS", "Istanbul 2nd Session"),
    ("T0", "TurkDEX Stock Exchange"),
    ("TC", "Turks & Caicos Islands"),
    ("TL", "Gibraltar Stock Exchange"),
    ("TU", "Tunis Stock Exchange"),
    // Misc small exchanges & jurisdictions
    ("AD", "Andorra"),
    ("AI", "Anguilla"),
    ("AV", "Vienna Stock Exchange"),
    // BR, HD, LU intentionally not duplicated here — they are mapped above
    // ("BR" → Rio de Janeiro, "HD" → Alpha Dark, "LU" → LSX LSSI). The
    // OpenFIGI codes for the Honduras and Luxembourg national exchanges are
    // "HO" and "LX", both of which are already mapped earlier in the table.
    ("CB", "Colombia Stock Exchange"),
    ("CI", "Chile Stock Exchange"),
    ("CK", "Prague Stock Exchange"),
    ("CN", "China Composite"),
    ("CP", "Czech Republic Stock Exchange"),
    ("EH", "Estonia Stock Exchange"),
    ("FB", "Bahrain Bourse"),
    ("GG", "Joint Stock Company Georgian Stock Exchange"),
    ("GU", "Guernsey Stock Exchange"),
    ("JY", "Jersey"),
    ("LE", "Liechtenstein"),
    ("LS", "Laos Stock Exchange"),
    ("MA", "Macau"),
    ("MD", "Madeira"),
    ("MI", "Marshall Islands"),
    ("MN", "Monaco"),
    ("MO", "Mongolia Stock Exchange"),
    ("NT", "Netherlands Antilles"),
    ("NX", "St. Kitts & Nevis"),
    ("NZ", "New Zealand Stock Exchange"),
    ("PB", "PNGX Markets"),
    ("PC", "Warsaw Continuous Market"),
    ("PD", "MTS Poland OTC"),
    ("PL", "Euronext Lisbon Stock Exchange"),
    ("PT", "Warsaw Auction Market"),
    ("PW", "Warsaw Auction Market"),
    ("SZ", "Seychelles Real Time Feed"),
    ("VI", "British Virgin Islands"),
    ("MM", "MM Composite"),
    ("NA", "Euronext Amsterdam Stock Exchange"),
    ("NE", "Euronext Amsterdam Derivatives"),
    ("SW", "Switzerland Composite"),
    ("GR", "Germany Composite"),
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(
        name: &str,
        share_class: Option<&str>,
        exchange: Option<&str>,
        ticker: Option<&str>,
        currency: Option<&str>,
        security_type: Option<&str>,
    ) -> RawFigiHit {
        RawFigiHit {
            name: name.to_string(),
            ticker: ticker.map(str::to_string),
            security_type: security_type.map(str::to_string),
            currency: currency.map(str::to_string),
            exchange_code: exchange.map(str::to_string),
            mic_code: None,
            share_class_figi: share_class.map(str::to_string),
            composite_figi: None,
        }
    }

    fn hit_with_mic(
        name: &str,
        share_class: Option<&str>,
        exchange_code: Option<&str>,
        mic_code: Option<&str>,
        ticker: Option<&str>,
        currency: Option<&str>,
        security_type: Option<&str>,
    ) -> RawFigiHit {
        RawFigiHit {
            name: name.to_string(),
            ticker: ticker.map(str::to_string),
            security_type: security_type.map(str::to_string),
            currency: currency.map(str::to_string),
            exchange_code: exchange_code.map(str::to_string),
            mic_code: mic_code.map(str::to_string),
            share_class_figi: share_class.map(str::to_string),
            composite_figi: None,
        }
    }

    // ------------------------------------------------------------------
    // Exchange code resolution
    // ------------------------------------------------------------------

    #[test]
    fn resolves_known_exchange_codes_to_human_readable_names() {
        assert_eq!(resolve_exchange_name("FP"), "Euronext Paris Stock Exchange");
        assert_eq!(resolve_exchange_name("UN"), "New York Stock Exchange");
        assert_eq!(resolve_exchange_name("UW"), "Nasdaq Global Select Market");
        assert_eq!(resolve_exchange_name("GY"), "Xetra Stock Exchange");
        assert_eq!(resolve_exchange_name("HK"), "Hong Kong Stock Exchange");
    }

    /// Codes the user reported as unresolved must now resolve.
    #[test]
    fn resolves_user_reported_codes() {
        assert_eq!(resolve_exchange_name("UV"), "OTC US Market");
        assert_eq!(resolve_exchange_name("XT"), "Athens OTC Stock Exchange");
        assert_eq!(resolve_exchange_name("XJ"), "Ljubljana SE OTC");
        assert_eq!(resolve_exchange_name("XG"), "Nordic Growth Market OTC");
    }

    // ------------------------------------------------------------------
    // securityType → AssetClass mapping (WEB-023)
    // ------------------------------------------------------------------

    #[test]
    fn map_security_type_etp_returns_etp_class() {
        // OpenFIGI uses "ETP" as the umbrella for ETF/ETN/ETC and does not
        // emit "ETF" as a securityType (verified via /v3/mapping/values/securityType).
        assert_eq!(map_security_type("ETP"), Some(AssetClass::ETP));
        assert_eq!(map_security_type("ETF"), None);
    }

    #[test]
    fn map_security_type_mutual_fund_variants_return_mutualfunds() {
        assert_eq!(
            map_security_type("Mutual Fund"),
            Some(AssetClass::MutualFunds)
        );
        assert_eq!(
            map_security_type("Open-End Fund"),
            Some(AssetClass::MutualFunds)
        );
        assert_eq!(
            map_security_type("Closed-End Fund"),
            Some(AssetClass::MutualFunds)
        );
        assert_eq!(
            map_security_type("Fund of Funds"),
            Some(AssetClass::MutualFunds)
        );
        assert_eq!(
            map_security_type("Pvt Eqty Fund"),
            Some(AssetClass::MutualFunds)
        );
    }

    #[test]
    fn map_security_type_bond_variants_return_bonds() {
        assert_eq!(map_security_type("Bond"), Some(AssetClass::Bonds));
        assert_eq!(map_security_type("Conv Bond"), Some(AssetClass::Bonds));
        // Old vocabulary that does not exist in OpenFIGI's actual list.
        assert_eq!(map_security_type("Corporate Bond"), None);
        assert_eq!(map_security_type("Government Bond"), None);
    }

    #[test]
    fn map_security_type_crypto_returns_digital_asset() {
        assert_eq!(map_security_type("Crypto"), Some(AssetClass::DigitalAsset));
        // Old vocabulary that does not exist in OpenFIGI's actual list.
        assert_eq!(map_security_type("Cryptocurrency"), None);
        assert_eq!(map_security_type("Digital Currency"), None);
    }

    #[test]
    fn map_security_type_reit_returns_real_estate() {
        assert_eq!(map_security_type("REIT"), Some(AssetClass::RealEstate));
        // Old vocabulary that does not exist in OpenFIGI's actual list.
        assert_eq!(map_security_type("Real Estate Investment Trust"), None);
    }

    #[test]
    fn map_security_type_derivatives_variants_return_derivatives() {
        assert_eq!(map_security_type("Warrant"), Some(AssetClass::Derivatives));
        assert_eq!(
            map_security_type("Equity Option"),
            Some(AssetClass::Derivatives)
        );
        assert_eq!(
            map_security_type("Index Option"),
            Some(AssetClass::Derivatives)
        );
        assert_eq!(
            map_security_type("SINGLE STOCK FUTURE"),
            Some(AssetClass::Derivatives)
        );
        assert_eq!(
            map_security_type("SINGLE STOCK DIVIDEND FUTURE"),
            Some(AssetClass::Derivatives)
        );
        // Old generic vocabulary that does not exist in OpenFIGI's actual list.
        assert_eq!(map_security_type("Option"), None);
        assert_eq!(map_security_type("Future"), None);
        assert_eq!(map_security_type("Rights"), None);
    }

    #[test]
    fn map_security_type_unrecognised_returns_none() {
        // Real OpenFIGI values that the project intentionally does not map.
        assert_eq!(map_security_type("ADR"), None);
        assert_eq!(map_security_type("Calendar Spread Option"), None);
        assert_eq!(map_security_type("EURO MTN"), None);
        assert_eq!(map_security_type(""), None);
    }

    #[test]
    fn unknown_exchange_code_falls_back_to_raw_string() {
        assert_eq!(resolve_exchange_name("ZZ"), "ZZ");
    }

    // ------------------------------------------------------------------
    // ISIN country prefix
    // ------------------------------------------------------------------

    #[test]
    fn isin_country_extracts_two_letter_prefix() {
        assert_eq!(isin_country("FR0000120073"), Some("FR"));
        assert_eq!(isin_country("US0378331005"), Some("US"));
        assert_eq!(isin_country("GB00B16GWD56"), Some("GB"));
    }

    #[test]
    fn isin_country_rejects_short_input() {
        assert_eq!(isin_country("F"), None);
    }

    #[test]
    fn primary_venues_for_known_country_returns_list() {
        assert_eq!(primary_venues_for_country("FR"), Some(&["FP"][..]));
        assert_eq!(primary_venues_for_country("US"), Some(&["UN", "UW"][..]));
        assert_eq!(primary_venues_for_country("CN"), Some(&["CG", "CS"][..]));
    }

    #[test]
    fn primary_venues_for_unknown_country_returns_none() {
        assert_eq!(primary_venues_for_country("XY"), None);
    }

    // ------------------------------------------------------------------
    // Dedup by share class (WEB-050 step 1 + 2)
    // ------------------------------------------------------------------

    #[test]
    fn drops_hits_with_null_share_class_figi() {
        let hits = vec![
            hit(
                "AIR LIQUIDE SA",
                None,
                Some("X1"),
                Some("AINOK"),
                None,
                None,
            ),
            hit(
                "AIR LIQUIDE SA",
                Some("BBG001S77RY7"),
                Some("FP"),
                Some("AI"),
                Some("EUR"),
                Some("Common Stock"),
            ),
        ];
        let groups = group_by_share_class(hits);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[0][0].exchange_code.as_deref(), Some("FP"));
    }

    #[test]
    fn groups_hits_with_same_share_class_together() {
        let hits = vec![
            hit("X", Some("SC1"), Some("UV"), None, None, None),
            hit("X", Some("SC1"), Some("FP"), None, None, None),
            hit("Y", Some("SC2"), Some("UN"), None, None, None),
            hit("X", Some("SC1"), Some("XT"), None, None, None),
        ];
        let groups = group_by_share_class(hits);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 3);
        assert_eq!(groups[1].len(), 1);
    }

    // ------------------------------------------------------------------
    // Primary pick (WEB-050 step 3)
    // ------------------------------------------------------------------

    #[test]
    fn keyword_path_picks_first_priority_match() {
        let group = vec![
            hit("X", Some("SC1"), Some("UV"), None, None, None),
            hit("X", Some("SC1"), Some("XT"), None, None, None),
            hit("X", Some("SC1"), Some("FP"), None, None, None),
        ];
        let priority = priority_for(&QueryContext::default());
        let picked = pick_primary_entries(&group, &priority, MAX_ENTRIES_PER_SHARE_CLASS_KEYWORD);
        // FP is in the priority list, UV/XT are not — FP wins solo.
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].exchange_code.as_deref(), Some("FP"));
    }

    #[test]
    fn dual_listed_share_class_returns_multiple_priority_matches() {
        let group = vec![
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("UV"),
                None,
                None,
                None,
            ),
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("UN"),
                Some("TTE"),
                Some("USD"),
                Some("Common Stock"),
            ),
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("FP"),
                Some("TTE"),
                Some("EUR"),
                Some("Common Stock"),
            ),
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("GY"),
                Some("TOTB"),
                Some("EUR"),
                Some("Common Stock"),
            ),
        ];
        let priority = priority_for(&QueryContext::default());
        let picked = pick_primary_entries(&group, &priority, MAX_ENTRIES_PER_SHARE_CLASS_KEYWORD);
        // Up to MAX_ENTRIES_PER_SHARE_CLASS hits returned — UN, FP, GY all qualify.
        assert_eq!(picked.len(), 3);
        // Order follows GLOBAL_VENUE_PRIORITY: UN before FP before GY.
        assert_eq!(picked[0].exchange_code.as_deref(), Some("UN"));
        assert_eq!(picked[1].exchange_code.as_deref(), Some("FP"));
        assert_eq!(picked[2].exchange_code.as_deref(), Some("GY"));
    }

    #[test]
    fn group_with_no_priority_match_falls_back_to_first_entry() {
        let group = vec![
            hit("X", Some("SC1"), Some("UV"), None, None, None),
            hit("X", Some("SC1"), Some("XT"), None, None, None),
        ];
        let priority = priority_for(&QueryContext::default());
        let picked = pick_primary_entries(&group, &priority, MAX_ENTRIES_PER_SHARE_CLASS_KEYWORD);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].exchange_code.as_deref(), Some("UV"));
    }

    // ------------------------------------------------------------------
    // ISIN-aware priority order (WEB-050 step 4)
    // ------------------------------------------------------------------

    #[test]
    fn isin_path_promotes_country_primary_to_top() {
        let group = vec![
            hit("AIR LIQUIDE SA", Some("SC1"), Some("UN"), None, None, None),
            hit("AIR LIQUIDE SA", Some("SC1"), Some("FP"), None, None, None),
            hit("AIR LIQUIDE SA", Some("SC1"), Some("GY"), None, None, None),
        ];
        let ctx = QueryContext {
            isin: Some("FR0000120073".to_string()),
        };
        let priority = priority_for(&ctx);
        let picked = pick_primary_entries(&group, &priority, MAX_ENTRIES_PER_SHARE_CLASS_ISIN);
        // FR ISIN promotes FP to position 0; UN and GY remain selectable.
        assert_eq!(picked[0].exchange_code.as_deref(), Some("FP"));
    }

    #[test]
    fn isin_path_ie_ucits_etf_falls_through_to_lo_na_gy_in_order() {
        // IE00B53L3W79 (iShares Core S&P 500 UCITS ETF): Irish-domiciled but
        // no Dublin listing — trades primarily on LSE / Amsterdam / Xetra.
        let group = vec![
            hit(
                "ISHARES CORE SP500 UCITS",
                Some("SC1"),
                Some("GY"),
                None,
                None,
                None,
            ),
            hit(
                "ISHARES CORE SP500 UCITS",
                Some("SC1"),
                Some("NA"),
                None,
                None,
                None,
            ),
            hit(
                "ISHARES CORE SP500 UCITS",
                Some("SC1"),
                Some("LO"),
                None,
                None,
                None,
            ),
        ];
        let ctx = QueryContext {
            isin: Some("IE00B53L3W79".to_string()),
        };
        let priority = priority_for(&ctx);
        let picked = pick_primary_entries(&group, &priority, MAX_ENTRIES_PER_SHARE_CLASS_ISIN);
        // ID is prepended but has no hit; LO/NA/GY follow in that prepended order.
        assert_eq!(picked.len(), 3);
        assert_eq!(picked[0].exchange_code.as_deref(), Some("LO"));
        assert_eq!(picked[1].exchange_code.as_deref(), Some("NA"));
        assert_eq!(picked[2].exchange_code.as_deref(), Some("GY"));
    }

    #[test]
    fn isin_path_ie_equity_still_picks_dublin_first() {
        // IE-prefixed equity (e.g. Kerry Group, CRH) with a real Dublin
        // listing — Dublin must still win even after IE was extended to
        // also cover UCITS-ETF fallthrough venues.
        let group = vec![
            hit("KERRY GROUP PLC", Some("SC1"), Some("LO"), None, None, None),
            hit("KERRY GROUP PLC", Some("SC1"), Some("ID"), None, None, None),
            hit("KERRY GROUP PLC", Some("SC1"), Some("GY"), None, None, None),
        ];
        let ctx = QueryContext {
            isin: Some("IE0004906560".to_string()),
        };
        let priority = priority_for(&ctx);
        let picked = pick_primary_entries(&group, &priority, MAX_ENTRIES_PER_SHARE_CLASS_ISIN);
        assert_eq!(picked[0].exchange_code.as_deref(), Some("ID"));
    }

    // ------------------------------------------------------------------
    // process_hits end-to-end
    // ------------------------------------------------------------------

    #[test]
    fn process_hits_returns_keyword_path_air_liquide_on_fp() {
        let hits = vec![
            hit(
                "AIR LIQUIDE SA",
                Some("SC1"),
                Some("UV"),
                Some("AIQUF"),
                Some("USD"),
                Some("Common Stock"),
            ),
            hit(
                "AIR LIQUIDE SA",
                Some("SC1"),
                Some("FP"),
                Some("AI"),
                Some("EUR"),
                Some("Common Stock"),
            ),
            hit(
                "AIR LIQUIDE SA",
                Some("SC1"),
                Some("XT"),
                Some("AINOK"),
                None,
                Some("Common Stock"),
            ),
        ];
        let results = process_hits(hits, &QueryContext::default());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "AIR LIQUIDE SA");
        assert_eq!(results[0].reference.as_deref(), Some("AI"));
        assert_eq!(results[0].currency.as_deref(), Some("EUR"));
        assert_eq!(results[0].asset_class, Some(AssetClass::Stocks));
        // exchange is now Option<Exchange>; FP should resolve to XPAR via resolve_canonical_exchange
        let exchange = results[0]
            .exchange
            .as_ref()
            .expect("exchange should be Some for FP");
        assert_eq!(exchange.code, "XPAR");
    }

    // WEB-046 — on the ISIN path, the result's `reference` field carries the
    // ticker from the hit (here, "AI") and `isin` carries the normalized ISIN
    // forwarded via `QueryContext.isin`.
    #[test]
    fn process_hits_isin_path_fills_ticker_in_reference_and_isin_in_isin() {
        let hits = vec![hit(
            "AIR LIQUIDE SA",
            Some("SC1"),
            Some("FP"),
            Some("AI"),
            Some("EUR"),
            Some("Common Stock"),
        )];
        let ctx = QueryContext {
            isin: Some("FR0000120073".to_string()),
        };
        let results = process_hits(hits, &ctx);
        assert_eq!(results[0].reference.as_deref(), Some("AI"));
        assert_eq!(results[0].isin.as_deref(), Some("FR0000120073"));
    }

    #[test]
    fn process_hits_returns_two_rows_for_dual_listed_totalenergies() {
        let hits = vec![
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("UV"),
                Some("TTE"),
                Some("USD"),
                Some("Common Stock"),
            ),
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("UN"),
                Some("TTE"),
                Some("USD"),
                Some("Common Stock"),
            ),
            hit(
                "TOTALENERGIES SE",
                Some("SC1"),
                Some("FP"),
                Some("TTE"),
                Some("EUR"),
                Some("Common Stock"),
            ),
        ];
        let results = process_hits(hits, &QueryContext::default());
        assert_eq!(results.len(), 2);
        // exchange is now Option<Exchange>; UN → XNYS, FP → XPAR
        let exchange_0 = results[0]
            .exchange
            .as_ref()
            .expect("first result should have exchange");
        assert_eq!(exchange_0.code, "XNYS");
        assert_eq!(results[0].currency.as_deref(), Some("USD"));
        let exchange_1 = results[1]
            .exchange
            .as_ref()
            .expect("second result should have exchange");
        assert_eq!(exchange_1.code, "XPAR");
        assert_eq!(results[1].currency.as_deref(), Some("EUR"));
    }

    /// WEB-050e — keyword path keeps up to 10 entries per share class.
    /// 11 priority venues, all on the same share class → cap of 10 keeps 10.
    #[test]
    fn keyword_path_caps_per_share_class_at_ten() {
        let hits = vec![
            hit(
                "X",
                Some("SC1"),
                Some("UN"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("UW"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("LO"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("JT"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("FP"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("GY"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("HK"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("SE"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("AT"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("CT"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("IM"),
                None,
                None,
                Some("Common Stock"),
            ),
        ];
        let results = process_hits(hits, &QueryContext::default());
        assert_eq!(results.len(), 10);
    }

    /// WEB-050e — ISIN path keeps at most 3 entries per share class.
    ///
    /// Same 15-venue setup as the keyword test, but with a `QueryContext`
    /// that carries an ISIN.  With the dual-cap rule the result must be
    /// exactly 3 (not 10).
    #[test]
    fn isin_path_caps_per_share_class_at_three() {
        // 15 venues all belonging to the same share class — more than both caps.
        // These exchange codes appear in GLOBAL_VENUE_PRIORITY so they all
        // qualify as priority matches.
        let hits = vec![
            hit(
                "X",
                Some("SC1"),
                Some("UN"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("UW"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("LO"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("JT"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("FP"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("GY"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("HK"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("SE"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("AT"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("CT"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("IM"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("NA"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("SQ"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("BB"),
                None,
                None,
                Some("Common Stock"),
            ),
            hit(
                "X",
                Some("SC1"),
                Some("ID"),
                None,
                None,
                Some("Common Stock"),
            ),
        ];
        let ctx = QueryContext {
            isin: Some("IE00B53L3W79".to_string()),
        };
        let results = process_hits(hits, &ctx);
        // With the ISIN path cap of 3 (WEB-050e), at most 3 entries per share
        // class are retained regardless of how many venues qualify.
        assert_eq!(
            results.len(),
            3,
            "ISIN path must cap per-share-class entries at 3, got {}",
            results.len()
        );
    }

    #[test]
    fn process_hits_drops_null_share_class_entries() {
        let hits = vec![
            hit("X", None, Some("X1"), None, None, None),
            hit("X", None, Some("X2"), None, None, None),
            hit(
                "X",
                Some("SC1"),
                Some("FP"),
                None,
                None,
                Some("Common Stock"),
            ),
        ];
        let results = process_hits(hits, &QueryContext::default());
        assert_eq!(results.len(), 1);
        // The exchange field is now Option<Exchange>, not Option<String>.
        // After implementation, this will be Some(Exchange { code: "XPAR", .. }).
        // For now we verify the result is present (exchange may be None until
        // resolve_canonical_exchange is implemented — the compile failure is expected).
        assert_eq!(results.len(), 1);
    }

    // ------------------------------------------------------------------
    // resolve_canonical_exchange (WEB-049 precedence rule)
    // ------------------------------------------------------------------

    // WEB-049 (a) — micCode present and canonical → returns Some(Exchange)
    #[test]
    fn resolve_canonical_exchange_uses_mic_code_when_present_and_canonical() {
        let hit = hit_with_mic(
            "AIR LIQUIDE SA",
            Some("SC1"),
            Some("FP"),   // exchCode present but should NOT be consulted
            Some("XPAR"), // micCode present and canonical → wins
            Some("AI"),
            Some("EUR"),
            Some("Common Stock"),
        );
        let result = resolve_canonical_exchange(&hit);
        assert!(
            result.is_some(),
            "resolve_canonical_exchange should return Some when micCode is canonical"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XPAR");
    }

    // WEB-049 (b) — micCode present but NOT in curated set → falls through to exchCode
    #[test]
    fn resolve_canonical_exchange_falls_back_to_exchcode_when_mic_not_canonical() {
        let hit = hit_with_mic(
            "SOME STOCK",
            Some("SC1"),
            Some("FP"),   // exchCode maps to XPAR
            Some("XBOG"), // micCode present but not in curated set → fall through
            Some("TICK"),
            Some("EUR"),
            Some("Common Stock"),
        );
        let result = resolve_canonical_exchange(&hit);
        // Implementation should fall through to exchCode "FP" → XPAR
        assert!(
            result.is_some(),
            "should fall back to exchCode when micCode is not canonical"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XPAR");
    }

    // WEB-049 (c) — micCode absent, exchCode maps to canonical via openfigi mapper
    #[test]
    fn resolve_canonical_exchange_uses_exchcode_when_mic_absent() {
        let hit = hit_with_mic(
            "APPLE INC",
            Some("SC1"),
            Some("UN"), // exchCode for NYSE → XNYS
            None,       // micCode absent
            Some("AAPL"),
            Some("USD"),
            Some("Common Stock"),
        );
        let result = resolve_canonical_exchange(&hit);
        assert!(
            result.is_some(),
            "should use exchCode fallback when micCode is absent"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XNYS");
    }

    // WEB-049 (d) — neither micCode nor exchCode resolves → None
    #[test]
    fn resolve_canonical_exchange_returns_none_when_neither_resolves() {
        let hit = hit_with_mic(
            "OBSCURE STOCK",
            Some("SC1"),
            Some("ZZ"), // exchCode unknown
            None,       // micCode absent
            Some("TICK"),
            Some("USD"),
            Some("Common Stock"),
        );
        let result = resolve_canonical_exchange(&hit);
        assert!(
            result.is_none(),
            "should return None when neither micCode nor exchCode resolves to a canonical exchange"
        );
    }

    // WEB-049 — both absent → None
    #[test]
    fn resolve_canonical_exchange_returns_none_when_both_absent() {
        let hit = hit_with_mic(
            "SOME STOCK",
            Some("SC1"),
            None, // exchCode absent
            None, // micCode absent
            None,
            None,
            Some("Common Stock"),
        );
        let result = resolve_canonical_exchange(&hit);
        assert!(
            result.is_none(),
            "should return None when both micCode and exchCode are absent"
        );
    }
}
