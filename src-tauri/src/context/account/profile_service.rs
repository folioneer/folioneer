//! PFL — profiles: created, renamed, removed and listed, and the profile in use chosen.

use std::sync::Arc;

use super::domain::{Profile, ProfileList, ProfileRepository, ProfileSummary};
use super::error::AccountError;
use crate::core::{logger::BACKEND, Event, SideEffectEventBus};

/// The profiles of the portfolio (PFL).
pub struct ProfileService {
    profiles: Box<dyn ProfileRepository>,
    event_bus: Option<Arc<SideEffectEventBus>>,
}

fn database(what: &str, e: anyhow::Error) -> AccountError {
    tracing::error!(target: BACKEND, err = ?e, "{what}: repository failure");
    AccountError::DatabaseError
}

impl ProfileService {
    /// The service over the profiles' storage.
    pub fn new(profiles: Box<dyn ProfileRepository>) -> Self {
        Self {
            profiles,
            event_bus: None,
        }
    }

    /// The service publishing `ProfileUpdated` (PFL-034).
    pub fn with_event_bus(mut self, bus: Arc<SideEffectEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    fn signal(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::ProfileUpdated);
        }
    }

    /// PFL-023 — every profile with its account count: the default one first, then by name.
    async fn summaries(&self) -> Result<Vec<ProfileSummary>, AccountError> {
        let mut profiles = self
            .profiles
            .list()
            .await
            .map_err(|e| database("list_profiles", e))?;
        // PFL-023 — the default one first, then by name, case ignored, then by identifier.
        profiles.sort_by_cached_key(|summary| {
            (
                !summary.profile.is_default(),
                summary.profile.name.as_deref().map(str::to_lowercase),
                summary.profile.id.clone(),
            )
        });
        Ok(profiles)
    }

    /// The profile with this identifier, or `ProfileNotFound`.
    pub async fn existing(&self, id: &str) -> Result<Profile, AccountError> {
        self.profiles
            .get(id)
            .await
            .map_err(|e| database("get_profile", e))?
            .ok_or_else(|| AccountError::ProfileNotFound {
                profile_id: id.to_string(),
            })
    }

    async fn ensure_free(&self, name: &str, except_id: Option<String>) -> Result<(), AccountError> {
        if self
            .profiles
            .name_taken(name, except_id)
            .await
            .map_err(|e| database("profile_name_taken", e))?
        {
            return Err(AccountError::ProfileNameAlreadyExists);
        }
        Ok(())
    }

    /// PFL-023 — every profile with its account count, the default one first, then by
    /// name, with the profile in use (PFL-012).
    pub async fn list(&self) -> Result<ProfileList, AccountError> {
        Ok(ProfileList {
            profiles: self.summaries().await?,
            in_use: self.in_use().await?,
        })
    }

    /// PFL-012 / PFL-014 — the identifier of the profile in use.
    pub async fn in_use(&self) -> Result<String, AccountError> {
        self.profiles
            .in_use()
            .await
            .map_err(|e| database("profile_in_use", e))
    }

    /// PFL-032 — makes an existing profile the profile in use; an unknown one is rejected, and
    /// choosing the one in use changes nothing.
    pub async fn choose(&self, id: &str) -> Result<(), AccountError> {
        self.existing(id).await?;
        if self.in_use().await? == id {
            return Ok(());
        }
        self.profiles
            .set_in_use(id)
            .await
            .map_err(|e| database("choose_profile", e))?;
        self.signal();
        Ok(())
    }

    /// PFL-020 — creates a profile with a trimmed name no other profile has.
    pub async fn create(&self, name: &str) -> Result<Profile, AccountError> {
        let profile = Profile::new(name)?;
        if let Some(name) = &profile.name {
            self.ensure_free(name, None).await?;
        }
        self.profiles
            .save(&profile)
            .await
            .map_err(|e| database("create_profile", e))?;
        self.signal();
        Ok(profile)
    }

    /// PFL-030 — renames a profile, the default one included.
    pub async fn rename(&self, id: &str, name: &str) -> Result<Profile, AccountError> {
        let profile = self.existing(id).await?.renamed(name)?;
        if let Some(name) = &profile.name {
            self.ensure_free(name, Some(id.to_string())).await?;
        }
        self.profiles
            .save(&profile)
            .await
            .map_err(|e| database("rename_profile", e))?;
        self.signal();
        Ok(profile)
    }

    /// PFL-040 — removes a profile holding no account; never the default one.
    pub async fn remove(&self, id: &str) -> Result<(), AccountError> {
        let profile = self.existing(id).await?;
        if profile.is_default() {
            return Err(AccountError::DefaultProfileKept);
        }
        let removed = self
            .profiles
            .delete_if_empty(id)
            .await
            .map_err(|e| database("remove_profile", e))?;
        if !removed {
            return Err(AccountError::ProfileNotEmpty);
        }
        self.signal();
        Ok(())
    }
}
