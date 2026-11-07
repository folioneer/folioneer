//! France (ADR-025): the weighted average cost, and its wrappers.

use super::{CostBasisMethod, CountryRules};
use crate::context::account::domain::wrapper::{wrapper, Wrapper};

/// France's rules.
pub struct France;

const WRAPPERS: &[Wrapper] = &[
    wrapper("fr_pea", Some("FR"), ["PEA", "PEA", "PEA"]),
    wrapper("fr_pea_pme", Some("FR"), ["PEA-PME", "PEA-PME", "PEA-PME"]),
    wrapper(
        "fr_assurance_vie",
        Some("FR"),
        ["Assurance vie", "Assurance vie", "Assurance vie"],
    ),
    wrapper("fr_per", Some("FR"), ["PER", "PER", "PER"]),
];

impl CountryRules for France {
    fn code(&self) -> &'static str {
        "FR"
    }

    fn default_cost_basis(&self) -> Option<CostBasisMethod> {
        Some(CostBasisMethod::WeightedAverage)
    }

    fn wrappers(&self) -> &'static [Wrapper] {
        WRAPPERS
    }
}
