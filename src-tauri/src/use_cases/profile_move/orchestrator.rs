use std::sync::Arc;

use crate::context::account::{Account, AccountError, AccountServiceContract, ProfileService};

/// PFL-031 — moves an account into an existing profile with its whole history.
pub struct ProfileMoveUseCase {
    profiles: Arc<ProfileService>,
    accounts: Arc<dyn AccountServiceContract>,
}

impl ProfileMoveUseCase {
    /// The use case over the profiles and the accounts.
    pub fn new(profiles: Arc<ProfileService>, accounts: Arc<dyn AccountServiceContract>) -> Self {
        Self { profiles, accounts }
    }

    /// PFL-031 / PFL-022 / PFL-033 — an unknown profile is rejected; the account service
    /// checks the account, its name in the destination and its transfers. An account whose
    /// profile is removed meanwhile reads as the default one (PFL-061).
    pub async fn move_account(
        &self,
        account_id: &str,
        profile_id: &str,
    ) -> Result<Account, AccountError> {
        self.profiles.existing(profile_id).await?;
        self.accounts.move_to_profile(account_id, profile_id).await
    }
}
