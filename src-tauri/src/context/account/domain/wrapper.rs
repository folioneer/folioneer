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

/// ACC-043 — whether `wrapper_id` may be set on an account of `country`: one of the generic
/// list or of that country's list.
pub fn wrapper_available(wrapper_id: &str, country: &str) -> bool {
    WRAPPERS.iter().any(|wrapper| {
        wrapper.id == wrapper_id
            && wrapper
                .country
                .is_none_or(|list_country| list_country == country)
    })
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

    // ACC-043 — a wrapper of the generic list fits any country; a country's wrapper fits that
    // country only; an unknown identifier fits none.
    #[test]
    fn acc_043_a_wrapper_fits_the_generic_list_or_its_country() {
        assert!(wrapper_available("brokerage", "FR"));
        assert!(wrapper_available("brokerage", "ES"));
        assert!(wrapper_available("fr_pea", "FR"));
        assert!(!wrapper_available("fr_pea", "DE"));
        assert!(wrapper_available("de_bav", "DE"));
        assert!(!wrapper_available("isa", "FR"));
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
