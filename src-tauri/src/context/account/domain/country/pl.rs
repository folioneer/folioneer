//! Poland (ADR-025): first in, first out; no wrapper list of its own yet.

use super::{CostBasisMethod, CountryRules};

/// Poland's rules.
pub struct Poland;

impl CountryRules for Poland {
    fn code(&self) -> &'static str {
        "PL"
    }

    fn default_cost_basis(&self) -> Option<CostBasisMethod> {
        Some(CostBasisMethod::Fifo)
    }
}
