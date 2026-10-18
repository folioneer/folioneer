//! ACC-040/041 — the tax wrapper an account sits in: a label from a generic list or from its
//! country's list, never a tax rule.

use serde::Serialize;
use specta::Type;

/// ACC-041 — the portfolio's country, which an account without its own takes. Fixed until the
/// first run asks it (TODO-092).
pub const PORTFOLIO_COUNTRY: &str = "FR";

/// ACC-040 — one wrapper of the lists, labelled in the three languages the lists are kept in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Wrapper {
    /// The identifier an account stores; never changes once published.
    pub id: &'static str,
    /// The country whose list holds it (ISO 3166-1 alpha-2); `None` for the generic list.
    pub country: Option<&'static str>,
    /// Its label in English.
    pub label_en: &'static str,
    /// Its label in French.
    pub label_fr: &'static str,
    /// Its label in German.
    pub label_de: &'static str,
}

const fn wrapper(
    id: &'static str,
    country: Option<&'static str>,
    labels: [&'static str; 3],
) -> Wrapper {
    Wrapper {
        id,
        country,
        label_en: labels[0],
        label_fr: labels[1],
        label_de: labels[2],
    }
}

/// ACC-040 — the lists. A further country is rows added here, nothing else.
pub const WRAPPERS: &[Wrapper] = &[
    wrapper("brokerage", None, ["Brokerage", "Compte-titres", "Depot"]),
    wrapper("savings", None, ["Savings", "Épargne", "Sparkonto"]),
    wrapper("pension", None, ["Pension", "Retraite", "Altersvorsorge"]),
    wrapper(
        "life_insurance",
        None,
        ["Life insurance", "Assurance vie", "Lebensversicherung"],
    ),
    wrapper(
        "crypto_wallet",
        None,
        ["Crypto wallet", "Portefeuille crypto", "Krypto-Wallet"],
    ),
    wrapper("other", None, ["Other", "Autre", "Sonstiges"]),
    wrapper("fr_pea", Some("FR"), ["PEA", "PEA", "PEA"]),
    wrapper("fr_pea_pme", Some("FR"), ["PEA-PME", "PEA-PME", "PEA-PME"]),
    wrapper(
        "fr_assurance_vie",
        Some("FR"),
        ["Assurance vie", "Assurance vie", "Assurance vie"],
    ),
    wrapper("fr_per", Some("FR"), ["PER", "PER", "PER"]),
    wrapper("de_depot", Some("DE"), ["Depot", "Compte-titres", "Depot"]),
    wrapper(
        "de_riester_ruerup",
        Some("DE"),
        ["Riester / Rürup", "Riester / Rürup", "Riester / Rürup"],
    ),
    wrapper("de_bav", Some("DE"), ["bAV", "bAV", "bAV"]),
];

/// ACC-045 — every country, ISO 3166-1 alpha-2 in code order (from the iso-codes data); the
/// window names each in its language.
pub const COUNTRIES: &[&str] = &[
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX", "AZ",
    "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR", "BS",
    "BT", "BV", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM", "CN",
    "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM", "DO", "DZ", "EC", "EE",
    "EG", "EH", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR", "GA", "GB", "GD", "GE", "GF",
    "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS", "GT", "GU", "GW", "GY", "HK", "HM",
    "HN", "HR", "HT", "HU", "ID", "IE", "IL", "IM", "IN", "IO", "IQ", "IR", "IS", "IT", "JE", "JM",
    "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN", "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC",
    "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY", "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK",
    "ML", "MM", "MN", "MO", "MP", "MQ", "MR", "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA",
    "NC", "NE", "NF", "NG", "NI", "NL", "NO", "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG",
    "PH", "PK", "PL", "PM", "PN", "PR", "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW",
    "SA", "SB", "SC", "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS",
    "ST", "SV", "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO",
    "TR", "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI",
    "VN", "VU", "WF", "WS", "YE", "YT", "ZA", "ZM", "ZW",
];

/// ACC-043 — whether the lists hold `wrapper_id`: a known wrapper fits an account of any
/// country (a wrapper belongs to its country, holding it elsewhere is the user's choice).
pub fn wrapper_known(wrapper_id: &str) -> bool {
    WRAPPERS.iter().any(|wrapper| wrapper.id == wrapper_id)
}

/// ACC-045 — the wrappers an account of `country` is offered, in order: its country's list,
/// then the generic one (ACC-043).
pub fn wrapper_ids_for(country: &str) -> Vec<&'static str> {
    let of_country = WRAPPERS
        .iter()
        .filter(|wrapper| wrapper.country == Some(country));
    of_country
        .map(|wrapper| wrapper.id)
        .chain(generic_wrapper_ids())
        .collect()
}

/// ACC-045 — the countries the form offers, every one once: the portfolio's first, then each
/// other country with a list of its own in the lists' order, then the others in code order.
pub fn offered_countries() -> Vec<&'static str> {
    let mut countries = vec![PORTFOLIO_COUNTRY];
    let listed = WRAPPERS.iter().filter_map(|wrapper| wrapper.country);
    for country in listed.chain(COUNTRIES.iter().copied()) {
        if !countries.contains(&country) {
            countries.push(country);
        }
    }
    countries
}

/// ACC-040 — the generic list, which a country with no list of its own is offered.
pub fn generic_wrapper_ids() -> Vec<&'static str> {
    WRAPPERS
        .iter()
        .filter(|wrapper| wrapper.country.is_none())
        .map(|wrapper| wrapper.id)
        .collect()
}

/// ACC-045 — the wrappers each country with a list of its own is offered (the portfolio's
/// first), in order; any other country is offered `generic_wrapper_ids`.
pub fn country_offers() -> Vec<(&'static str, Vec<&'static str>)> {
    offered_countries()
        .into_iter()
        .filter(|country| {
            *country == PORTFOLIO_COUNTRY
                || WRAPPERS
                    .iter()
                    .any(|wrapper| wrapper.country == Some(country))
        })
        .map(|country| (country, wrapper_ids_for(country)))
        .collect()
}

/// ACC-041 — a country is written as two upper-case letters (ISO 3166-1 alpha-2).
pub fn country_well_formed(country: &str) -> bool {
    country.len() == 2 && country.chars().all(|c| c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ACC-040 — every wrapper is listed once, and the lists hold what the entry names.
    #[test]
    fn acc_040_the_lists_hold_the_generic_french_and_german_wrappers_once() {
        let mut ids: Vec<&str> = WRAPPERS.iter().map(|wrapper| wrapper.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "a wrapper identifier is listed twice");
        let of = |country: Option<&str>| {
            WRAPPERS
                .iter()
                .filter(|wrapper| wrapper.country == country)
                .count()
        };
        assert_eq!((of(None), of(Some("FR")), of(Some("DE"))), (6, 4, 3));
    }

    // ACC-043 — a wrapper the lists hold is known, whatever its country; an unknown
    // identifier is not.
    #[test]
    fn acc_043_a_wrapper_of_any_list_is_known() {
        assert!(wrapper_known("brokerage"));
        assert!(wrapper_known("fr_pea"));
        assert!(wrapper_known("de_bav"));
        assert!(!wrapper_known("isa"));
    }

    // ACC-045 — every country is offered once: the portfolio's first, then the countries with
    // a list of their own, then the others in code order.
    #[test]
    fn acc_045_every_country_is_offered_once_the_portfolios_first() {
        let offered = offered_countries();
        assert_eq!(&offered[..3], &["FR", "DE", "AD"]);
        assert_eq!(offered.len(), COUNTRIES.len());
        assert!(offered.contains(&"ES") && offered.contains(&"ZW"));
        let mut once = offered.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), offered.len(), "a country is offered twice");
        assert!(COUNTRIES.iter().all(|country| country_well_formed(country)));
    }

    // ACC-041 — a country is two upper-case letters.
    #[test]
    fn acc_041_a_country_is_two_upper_case_letters() {
        assert!(country_well_formed("DE"));
        assert!(!country_well_formed("de"));
        assert!(!country_well_formed("FRA"));
        assert!(!country_well_formed(""));
    }
}
