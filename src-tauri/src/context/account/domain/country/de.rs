//! Germany (ADR-025): first in, first out, and its wrappers.

use super::{CostBasisMethod, CountryRules};
use crate::context::account::domain::wrapper::{wrapper, Wrapper};

/// Germany's rules.
pub struct Germany;

const WRAPPERS: &[Wrapper] = &[
    wrapper("de_depot", Some("DE"), ["Depot", "Compte-titres", "Depot"]),
    wrapper(
        "de_riester_ruerup",
        Some("DE"),
        ["Riester / Rürup", "Riester / Rürup", "Riester / Rürup"],
    ),
    wrapper("de_bav", Some("DE"), ["bAV", "bAV", "bAV"]),
];

impl CountryRules for Germany {
    fn code(&self) -> &'static str {
        "DE"
    }

    fn default_cost_basis(&self) -> Option<CostBasisMethod> {
        Some(CostBasisMethod::Fifo)
    }

    fn wrappers(&self) -> &'static [Wrapper] {
        WRAPPERS
    }
}
