//! Italy (ADR-025): the weighted average cost; no wrapper list of its own yet.

use super::{CostBasisMethod, CountryRules};

/// Italy's rules.
pub struct Italy;

impl CountryRules for Italy {
    fn code(&self) -> &'static str {
        "IT"
    }

    fn default_cost_basis(&self) -> Option<CostBasisMethod> {
        Some(CostBasisMethod::WeightedAverage)
    }
}
