//! Inputs the asset service accepts from any adapter (the Tauri commands, a command line).

use serde::{Deserialize, Serialize};
use specta::Type;

use super::domain::{AssetClass, Exchange};

/// Parameters for creating a new asset.
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct CreateAssetDTO {
    /// Display name.
    pub name: String,
    /// Ticker / user-defined reference (mandatory — R1).
    pub reference: String,
    /// Optional ISIN (ISO 6166, 12 chars). Validated + normalized by the domain
    /// when present (AST-023, WEB-016).
    pub isin: Option<String>,
    /// Classification type.
    pub class: AssetClass,
    /// ISO currency code.
    pub currency: String,
    /// 1-5 risk score.
    pub risk_level: u8,
    /// ID of the primary category.
    pub category_id: String,
    /// Optional canonical trading venue (AST-021).
    pub exchange: Option<Exchange>,
    /// Whether the asset is an eligible Interest-credit target (AST-024).
    pub interest_bearing: bool,
}

/// Parameters for updating an existing asset.
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct UpdateAssetDTO {
    /// Target asset ID.
    pub asset_id: String,
    /// New display name.
    pub name: String,
    /// New reference (mandatory — R1).
    pub reference: String,
    /// New optional ISIN (AST-023). `None` clears the field.
    pub isin: Option<String>,
    /// New classification.
    pub class: AssetClass,
    /// New currency.
    pub currency: String,
    /// New risk level.
    pub risk_level: u8,
    /// New category link.
    pub category_id: String,
    /// New optional canonical trading venue (AST-021 / AST-022).
    pub exchange: Option<Exchange>,
    /// New Interest-credit eligibility (AST-024).
    pub interest_bearing: bool,
}
