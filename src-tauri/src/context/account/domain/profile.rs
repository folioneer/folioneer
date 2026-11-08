//! PFL — a profile: a set of accounts the owner follows apart from the others.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::context::account::error::AccountError;

/// PFL-011 — the default profile's identifier, the same on every device.
pub const DEFAULT_PROFILE_ID: &str = "default-profile";

/// The profile a change written before profiles existed reads as (PFL-060).
pub fn default_profile_id() -> String {
    DEFAULT_PROFILE_ID.to_string()
}

/// PFL — a set of accounts, named or, for the default profile, not yet named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Profile {
    /// Its identifier; the default profile's is `DEFAULT_PROFILE_ID` (PFL-011).
    pub id: String,
    /// The name the owner gave it; none for the default profile until named (PFL-011).
    pub name: Option<String>,
}

impl Profile {
    /// PFL-020 — a new profile with a trimmed name; an empty one is refused.
    pub fn new(name: &str) -> Result<Self, AccountError> {
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: Some(Self::checked_name(name)?),
        })
    }

    /// A profile read back from storage or a synced change, as it was stored.
    pub fn restore(id: String, name: Option<String>) -> Self {
        Self { id, name }
    }

    /// PFL-030 — this profile under a new name, trimmed; an empty one is refused.
    pub fn renamed(self, name: &str) -> Result<Self, AccountError> {
        Ok(Self {
            name: Some(Self::checked_name(name)?),
            ..self
        })
    }

    /// PFL-020 / PFL-030 — a name trimmed, refused when empty.
    pub fn checked_name(name: &str) -> Result<String, AccountError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AccountError::ProfileNameEmpty);
        }
        Ok(trimmed.to_string())
    }

    /// PFL-011 — whether this is the default profile.
    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_PROFILE_ID
    }
}

/// PFL-023 — a profile with the number of its accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ProfileSummary {
    /// The profile.
    pub profile: Profile,
    /// How many accounts belong to it.
    pub account_count: u32,
    /// PFL-040 — whether it may be removed: it holds no account and is not the default one.
    pub removable: bool,
}

impl ProfileSummary {
    /// PFL-023 / PFL-040 — a profile with its account count, and whether it may be removed.
    pub fn new(profile: Profile, account_count: u32) -> Self {
        let removable = account_count == 0 && !profile.is_default();
        Self {
            profile,
            account_count,
            removable,
        }
    }
}

/// PFL-023 — the profiles, with the profile in use (PFL-012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ProfileList {
    /// Every profile, the default one first.
    pub profiles: Vec<ProfileSummary>,
    /// The identifier of the profile in use, the default profile's when it is gone (PFL-014).
    pub in_use: String,
}

/// PFL — where profiles are kept; each write records its change for the sync (PFL-060).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ProfileRepository: Send + Sync {
    /// PFL-023 — every profile with its account count, the default one included.
    async fn list(&self) -> Result<Vec<ProfileSummary>>;
    /// The profile with this identifier, if any.
    async fn get(&self, id: &str) -> Result<Option<Profile>>;
    /// PFL-020 / PFL-030 — whether another profile than `except_id` has this name, case
    /// ignored.
    async fn name_taken(&self, name: &str, except_id: Option<String>) -> Result<bool>;
    /// PFL-020 / PFL-030 — creates or renames `profile`.
    async fn save(&self, profile: &Profile) -> Result<()>;
    /// PFL-040 — removes the profile when it holds no account and is not the default one, in
    /// one statement; whether it was removed.
    async fn delete_if_empty(&self, id: &str) -> Result<bool>;
    /// PFL-012 / PFL-014 — the profile in use: the default profile until another is chosen,
    /// or when the chosen one no longer exists.
    async fn in_use(&self) -> Result<String>;
    /// PFL-032 — makes `id` the profile in use; nothing is recorded for the sync (SYN-023).
    async fn set_in_use(&self, id: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // PFL-020 — a name is trimmed; an empty one is refused.
    #[test]
    fn a_name_is_trimmed_and_never_empty() {
        assert_eq!(
            Profile::new("  Parent ").expect("named").name.as_deref(),
            Some("Parent")
        );
        assert!(matches!(
            Profile::new("   "),
            Err(AccountError::ProfileNameEmpty)
        ));
    }

    // PFL-011 — the default profile is told apart by its identifier.
    #[test]
    fn the_default_profile_is_known_by_its_identifier() {
        let default = Profile::restore(default_profile_id(), None);
        assert!(default.is_default());
        assert!(!Profile::new("Parent").expect("named").is_default());
    }

    // PFL-030 — a rename trims the name, keeps the identifier, and refuses an empty one.
    #[test]
    fn a_rename_trims_and_keeps_the_identifier() {
        let default = Profile::restore(default_profile_id(), None);
        let renamed = default.clone().renamed(" Me ").expect("renamed");
        assert_eq!(renamed.id, default.id);
        assert_eq!(renamed.name.as_deref(), Some("Me"));
        assert!(matches!(
            default.renamed(" "),
            Err(AccountError::ProfileNameEmpty)
        ));
    }

    // PFL-040 — only a profile holding no account, never the default one, may be removed.
    #[test]
    fn a_profile_is_removable_when_empty_and_not_the_default() {
        let parent = Profile::new("Parent").expect("named");
        assert!(ProfileSummary::new(parent.clone(), 0).removable);
        assert!(!ProfileSummary::new(parent, 2).removable);
        let default = Profile::restore(default_profile_id(), None);
        assert!(!ProfileSummary::new(default, 0).removable);
    }
}
