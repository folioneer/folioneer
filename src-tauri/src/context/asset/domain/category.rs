use crate::context::asset::error::AssetError;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::result::Result as StdResult;
use uuid::Uuid;

/// The fixed ID of the system default category used as a fallback.
pub const SYSTEM_CATEGORY_ID: &str = "default-uncategorized";

/// A user-defined grouping for assets.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct AssetCategory {
    /// Unique identifier.
    pub id: String,
    /// Display name.
    pub name: String,
}

impl Default for AssetCategory {
    fn default() -> Self {
        Self {
            id: SYSTEM_CATEGORY_ID.to_string(),
            name: "generic.uncategorized".to_string(),
        }
    }
}

impl AssetCategory {
    /// Creates a new AssetCategory.
    pub fn new(label: String) -> StdResult<Self, AssetError> {
        if label.trim().is_empty() {
            return Err(AssetError::LabelEmpty);
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: label,
        })
    }

    /// Creates a new AssetCategory with a known deterministic ID.
    /// Used by system-seeded categories (e.g. the Cash category, CSH-017).
    pub fn with_id(id: String, label: String) -> StdResult<Self, AssetError> {
        if label.trim().is_empty() {
            return Err(AssetError::LabelEmpty);
        }
        Ok(Self { id, name: label })
    }

    /// Aggregate root method: applies a rename to this category. Enforces the
    /// system-category invariant (raises `SystemReadonly` for the seeded
    /// system category, via `ensure_renameable`) and validates the new label.
    /// Returns the updated `AssetCategory` for the caller to persist.
    pub fn update_from(self, label: String) -> Result<Self, AssetError> {
        self.ensure_renameable()?;
        if label.trim().is_empty() {
            return Err(AssetError::LabelEmpty);
        }
        Ok(Self {
            id: self.id,
            name: label,
        })
    }

    /// Returns true if this is the seeded system default category.
    fn is_system(&self) -> bool {
        self.id == SYSTEM_CATEGORY_ID
    }

    /// Aggregate-level invariant: the system category is read-only — its
    /// label cannot be changed by the user.
    pub fn ensure_renameable(&self) -> Result<(), AssetError> {
        if self.is_system() {
            return Err(AssetError::SystemReadonly);
        }
        Ok(())
    }

    /// Aggregate-level invariant: the system category is protected — it
    /// cannot be deleted.
    pub fn ensure_deletable(&self) -> Result<(), AssetError> {
        if self.is_system() {
            return Err(AssetError::SystemProtected);
        }
        Ok(())
    }

    /// Creates a new AssetCategory from storage.
    pub fn from_storage(category_id: String, label: String) -> Self {
        Self {
            id: category_id,
            name: label,
        }
    }
}

#[cfg(test)]
mod aggregate_tests {
    use super::*;

    fn user_category() -> AssetCategory {
        AssetCategory::from_storage("cat-bonds".to_string(), "Bonds".to_string())
    }

    fn system_category() -> AssetCategory {
        AssetCategory::from_storage(SYSTEM_CATEGORY_ID.to_string(), "uncategorized".to_string())
    }

    // R2 — system category cannot be renamed via update_from.
    #[test]
    fn update_from_rejects_system_category() {
        let err = system_category().update_from("Renamed".into()).unwrap_err();
        assert!(matches!(err, AssetError::SystemReadonly));
    }

    // update_from validates label after the state check passes.
    #[test]
    fn update_from_rejects_empty_label() {
        let err = user_category().update_from("   ".into()).unwrap_err();
        assert!(matches!(err, AssetError::LabelEmpty));
    }

    // update_from on a user category renames in place, preserving id.
    #[test]
    fn update_from_renames_user_category() {
        let updated = user_category().update_from("Stocks".into()).unwrap();
        assert_eq!(updated.id, "cat-bonds");
        assert_eq!(updated.name, "Stocks");
    }

    // R2 — system category is not deletable.
    #[test]
    fn ensure_deletable_rejects_system_category() {
        assert!(matches!(
            system_category().ensure_deletable().unwrap_err(),
            AssetError::SystemProtected
        ));
    }

    // ensure_renameable mirrors update_from's first guard for fail-fast use in services.
    #[test]
    fn ensure_renameable_rejects_system_category() {
        assert!(matches!(
            system_category().ensure_renameable().unwrap_err(),
            AssetError::SystemReadonly
        ));
    }

    // R2 takes precedence over input validation: an empty label on the system category
    // must surface SystemReadonly (state check runs first), not LabelEmpty.
    #[test]
    fn update_from_check_order_system_before_label() {
        let err = system_category().update_from("   ".into()).unwrap_err();
        assert!(matches!(err, AssetError::SystemReadonly));
    }
}

/// Interface for category persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AssetCategoryRepository: Send + Sync {
    /// Fetches all active categories.
    async fn get_all(&self) -> Result<Vec<AssetCategory>>;
    /// Fetches a category by its ID.
    async fn get_by_id(&self, id: &str) -> Result<Option<AssetCategory>>;
    /// Finds a category by name (case-insensitive).
    async fn find_by_name(&self, name: &str) -> Result<Option<AssetCategory>>;
    /// Persists a new category.
    async fn create(&self, category: AssetCategory) -> Result<AssetCategory>;
    /// Updates an existing category.
    async fn update(&self, category: AssetCategory) -> Result<AssetCategory>;
    /// Reassigns all assets from category_id to fallback_id, then soft-deletes the category.
    /// Both operations run in a single atomic transaction.
    async fn reassign_assets_and_delete(&self, category_id: &str, fallback_id: &str) -> Result<()>;
}
