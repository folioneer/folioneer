//! Account Creation: cross-context orchestration that seeds the per-currency
//! Cash Asset and the account's 0-balance Cash Holding at creation (ACC-025,
//! CSH-010 / CSH-012). The `add_account` command lives here (not in the account
//! bounded context) because creation now spans the account and asset contexts.

mod api;
mod orchestrator;

pub use api::*;
pub use orchestrator::AccountCreationUseCase;
