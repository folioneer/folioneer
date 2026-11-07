//! The United Kingdom (ADR-025): its tax year from 6 April. Its share pooling and 30-day
//! matching are its own method, left until planned (#395), so it implies no cost basis yet.

use super::CountryRules;

/// The United Kingdom's rules, under its ISO code GB.
pub struct UnitedKingdom;

impl CountryRules for UnitedKingdom {
    fn code(&self) -> &'static str {
        "GB"
    }

    fn tax_year_start(&self) -> (u32, u32) {
        (4, 6)
    }
}
