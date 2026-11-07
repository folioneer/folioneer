//! Spain (ADR-025): first in, first out; no wrapper list of its own yet.

use super::{CostBasisMethod, CountryRules};

/// Spain's rules.
pub struct Spain;

impl CountryRules for Spain {
    fn code(&self) -> &'static str {
        "ES"
    }

    fn default_cost_basis(&self) -> Option<CostBasisMethod> {
        Some(CostBasisMethod::Fifo)
    }
}
