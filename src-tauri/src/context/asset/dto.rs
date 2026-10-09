//! Inputs the asset service accepts from any adapter (the Tauri commands, a command line).

use serde::{Deserialize, Serialize};
use specta::Type;

use super::domain::{Asset, AssetClass, AssetKind, Exchange};

/// Parameters for creating a new asset.
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct CreateAssetDTO {
    /// How the asset is identified and priced (AST-030). Left out: the kind its class and
    /// ISIN make it (AST-034).
    pub kind: Option<AssetKind>,
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
    /// New kind (AST-030). Left out: the asset keeps its kind.
    pub kind: Option<AssetKind>,
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

/// An asset described the way a person names things (CLI-026): its category by name, its
/// exchange by code, and whatever is left out decided here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedAsset {
    /// Display name.
    pub name: String,
    /// Ticker or reference.
    pub reference: String,
    /// Classification.
    pub class: AssetClass,
    /// ISO currency code.
    pub currency: String,
    /// ISIN, when it has one.
    pub isin: Option<String>,
    /// MIC code of its exchange, when it has one.
    pub exchange_code: Option<String>,
    /// Risk level; the class's default when left out.
    pub risk_level: Option<u8>,
    /// Category name; the system category when left out.
    pub category_name: Option<String>,
}

/// An asset added by name, and what the user should know about it (CLI-026).
#[derive(Debug, Clone)]
pub struct AddedAsset {
    /// The asset as created.
    pub asset: Asset,
    /// Another asset, archived or not, has the same reference (AST-009): allowed — the
    /// same ticker trades on several markets — and worth saying.
    pub reference_shared: bool,
}
