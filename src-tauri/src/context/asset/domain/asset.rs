use super::asset_price::AssetPrice;
use super::category::AssetCategory;
use super::exchange::{self, Exchange};
use super::isin::validate_isin;
use crate::context::asset::error::AssetError;
use crate::shared::domain::{Rank, RecordKind, SyncedRecord};
use anyhow::Result;
use async_trait::async_trait;
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqliteConnection;
use std::result::Result as StdResult;
use std::str::FromStr;
use uuid::Uuid;

/// Represents the classification of an asset.

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
    Type,
    PartialEq,
    Eq,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum AssetClass {
    /// Real estate properties or REITs.
    RealEstate,
    /// Fiat currency or highly liquid equivalents.
    #[default]
    Cash,
    /// Individual company equities.
    Stocks,
    /// Fixed income securities.
    Bonds,
    /// Exchange Traded Funds.
    ETF,
    /// Exchange Traded Products — umbrella class for ETF, ETN, and ETC
    /// instruments that OpenFIGI returns under a single `"ETP"` securityType
    /// (WEB-023). Used when the structural distinction isn't available; users
    /// can edit the class manually if they need ETF/ETN/ETC granularity.
    ETP,
    /// Managed investment funds.
    MutualFunds,
    /// Cryptocurrencies or other blockchain-based assets.
    DigitalAsset,
    /// Leveraged or contingent instruments derived from an underlying asset (warrants, options, futures, rights).
    Derivatives,
}

impl AssetClass {
    /// The classes a user may create an asset in (CSH-015): every class but Cash, whose
    /// assets the application seeds itself.
    pub const fn user_addable() -> &'static [AssetClass] {
        &[
            AssetClass::RealEstate,
            AssetClass::Stocks,
            AssetClass::Bonds,
            AssetClass::ETF,
            AssetClass::ETP,
            AssetClass::MutualFunds,
            AssetClass::DigitalAsset,
            AssetClass::Derivatives,
        ]
    }

    /// Returns the default risk level for this asset class (R3).
    pub fn default_risk(&self) -> u8 {
        match self {
            AssetClass::Cash => 1,
            AssetClass::Bonds => 2,
            AssetClass::RealEstate => 2,
            AssetClass::MutualFunds => 3,
            AssetClass::ETF => 3,
            AssetClass::ETP => 3,
            AssetClass::Stocks => 4,
            AssetClass::DigitalAsset => 5,
            AssetClass::Derivatives => 5,
        }
    }
}

/// How an asset is identified and priced (AST-030); separate from its class, which says
/// what it is economically.
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Type,
    PartialEq,
    Eq,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum AssetKind {
    /// A listing: an instrument, by its ISIN, on an exchange, in a currency.
    Listed,
    /// Identified by its symbol, unique among crypto assets.
    Crypto,
    /// What no market lists; its reference is unique among custom assets.
    Custom,
    /// The application's own, one per currency.
    Cash,
}

impl AssetKind {
    /// The kind of an asset that carries none (AST-034): one that existed before kinds, or
    /// arrives in a change written before them. The Cash class is cash, the digital-asset
    /// class is crypto, an ISIN makes it listed, anything else is custom.
    pub fn of(class: &AssetClass, isin: Option<&str>) -> Self {
        match class {
            AssetClass::Cash => AssetKind::Cash,
            AssetClass::DigitalAsset => AssetKind::Crypto,
            _ if isin.is_some_and(|isin| !isin.trim().is_empty()) => AssetKind::Listed,
            _ => AssetKind::Custom,
        }
    }
}

/// A class a user may create an asset in, with the risk level a new asset of it starts at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AssetClassDefault {
    /// The class.
    pub class: AssetClass,
    /// The risk level preselected for it (R3).
    pub default_risk: u8,
}

/// The risk scale of an asset: the levels a user may pick, lowest risk first.
pub const RISK_LEVELS: std::ops::RangeInclusive<u8> = 1..=5;

/// What a new asset starts from, in every interface: the classes a user may pick
/// (CSH-015), each with its default risk level (R3), and the category preselected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AssetCreationDefaults {
    /// The classes a user may create an asset in, in the order they are offered.
    pub classes: Vec<AssetClassDefault>,
    /// The class preselected.
    pub class: AssetClass,
    /// The risk levels a user may pick, lowest risk first.
    pub risk_levels: Vec<u8>,
    /// The risk level preselected: the preselected class's.
    pub risk_level: u8,
    /// The category a new asset is in until the user picks one.
    pub category_id: String,
}

impl AssetCreationDefaults {
    /// The defaults of this application.
    pub fn current() -> Self {
        Self {
            classes: AssetClass::user_addable()
                .iter()
                .map(|class| AssetClassDefault {
                    class: class.clone(),
                    default_risk: class.default_risk(),
                })
                .collect(),
            class: AssetClass::Stocks,
            risk_levels: RISK_LEVELS.collect(),
            risk_level: AssetClass::Stocks.default_risk(),
            category_id: super::category::SYSTEM_CATEGORY_ID.to_string(),
        }
    }
}

/// An asset the rules of its kind would refuse today, and the first rule it breaks (AST-035).
#[derive(Debug, Clone, Serialize, Type)]
pub struct AssetToSettle {
    /// The asset, as it is.
    pub asset: Asset,
    /// What its kind forbids, or the asset it is the same as.
    pub problem: AssetError,
}

/// A financial instrument or resource held by a user.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct Asset {
    /// Unique identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// How the asset is identified and priced (AST-030).
    pub kind: AssetKind,
    /// Asset classification.
    pub class: AssetClass,
    /// Category link.
    pub category: AssetCategory,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Risk score from 1 to 5.
    pub risk_level: u8,
    /// Ticker / free-form reference (mandatory — R1). For quoted assets that
    /// also carry an ISIN, the canonical ISO 6166 identity lives in `isin`
    /// (AST-023); `reference` holds the provider-lookup symbol (Yahoo, etc.).
    pub reference: String,
    /// Optional ISIN (ISO 6166, 12 chars, Luhn-validated — AST-023). Stored
    /// in the normalized uppercase form returned by `validate_isin`.
    pub isin: Option<String>,
    /// Whether the asset is archived (soft-archived, reversible).
    pub is_archived: bool,
    /// Optional canonical trading venue (AST-021).
    pub exchange: Option<Exchange>,
    /// When true, the asset is excluded from every price-fetch task scope
    /// (MKT-150 / MKT-151, ADR-014), preserving its most recently recorded
    /// price. Independent of `is_archived`; toggled only by the dedicated
    /// `block_price_refresh` / `unblock_price_refresh` actions.
    pub price_refresh_blocked: bool,
    /// When true, the asset is an eligible target for Interest credits
    /// (AST-024 / INT-012). The account's Cash Asset is always eligible
    /// regardless of this flag (INT-023).
    pub interest_bearing: bool,
}

impl Asset {
    /// Creates a new Asset.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: AssetKind,
        name: String,
        class: AssetClass,
        category: AssetCategory,
        currency: String,
        risk_level: u8,
        reference: String,
        isin: Option<String>,
        exchange: Option<Exchange>,
        interest_bearing: bool,
    ) -> StdResult<Self, AssetError> {
        Self::validate(&name, risk_level, &currency, &reference, exchange.as_ref())?;

        let reference = reference.trim().to_uppercase();
        let isin = normalize_optional_isin(isin)?;
        Self::ensure_kind_allows(kind, &class, isin.as_deref(), exchange.as_ref())?;

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            kind,
            name,
            class,
            category,
            currency,
            risk_level,
            reference,
            isin,
            is_archived: false,
            exchange,
            price_refresh_blocked: false,
            interest_bearing,
        })
    }

    /// Reconstructs an Asset with a known ID (used for updates).
    #[allow(clippy::too_many_arguments)]
    pub fn with_id(
        asset_id: String,
        kind: AssetKind,
        name: String,
        class: AssetClass,
        category: AssetCategory,
        currency: String,
        risk_level: u8,
        reference: String,
        isin: Option<String>,
        is_archived: bool,
        exchange: Option<Exchange>,
        price_refresh_blocked: bool,
        interest_bearing: bool,
    ) -> StdResult<Self, AssetError> {
        Self::validate(&name, risk_level, &currency, &reference, exchange.as_ref())?;

        let reference = reference.trim().to_uppercase();
        let isin = normalize_optional_isin(isin)?;
        Self::ensure_kind_allows(kind, &class, isin.as_deref(), exchange.as_ref())?;

        Ok(Self {
            id: asset_id,
            kind,
            name,
            class,
            category,
            currency,
            risk_level,
            reference,
            isin,
            is_archived,
            exchange,
            price_refresh_blocked,
            interest_bearing,
        })
    }

    /// AST-031 — what each kind requires and forbids: a listed asset has an ISIN; a crypto
    /// or a custom one has neither ISIN nor exchange; the Cash class and the cash kind go
    /// together, as do the digital-asset class and the crypto kind. Cash is the
    /// application's and carries nothing else to check.
    fn ensure_kind_allows(
        kind: AssetKind,
        class: &AssetClass,
        isin: Option<&str>,
        exchange: Option<&Exchange>,
    ) -> StdResult<(), AssetError> {
        let kind_of_class = AssetKind::of(class, None);
        let class_decides = kind_of_class != AssetKind::Custom;
        let kind_decides = matches!(kind, AssetKind::Cash | AssetKind::Crypto);
        if (class_decides || kind_decides) && kind_of_class != kind {
            return Err(AssetError::ClassNotAllowed {
                kind,
                class: class.clone(),
            });
        }
        match kind {
            AssetKind::Cash => Ok(()),
            AssetKind::Listed if isin.is_none() => Err(AssetError::IsinRequired),
            AssetKind::Listed => Ok(()),
            AssetKind::Crypto | AssetKind::Custom if isin.is_some() => {
                Err(AssetError::IsinNotAllowed { kind })
            }
            AssetKind::Crypto | AssetKind::Custom if exchange.is_some() => {
                Err(AssetError::ExchangeNotAllowed { kind })
            }
            AssetKind::Crypto | AssetKind::Custom => Ok(()),
        }
    }

    /// AST-031 — the rule of its kind this asset breaks, if any: one that existed before
    /// kinds may break one, and is left as it is for the user to settle (AST-035).
    pub fn kind_problem(&self) -> Option<AssetError> {
        Self::ensure_kind_allows(
            self.kind,
            &self.class,
            self.isin.as_deref(),
            self.exchange.as_ref(),
        )
        .err()
    }

    /// AST-032 — whether `other` is the same asset as this one: two listed assets sharing
    /// ISIN, exchange and currency; two crypto assets sharing a symbol; two custom assets
    /// sharing a reference. References are compared without case. Cash is never compared:
    /// the application keeps one per currency itself.
    pub fn is_same_as(&self, other: &Asset) -> bool {
        if self.id == other.id || self.kind != other.kind {
            return false;
        }
        match self.kind {
            AssetKind::Listed => {
                self.isin.is_some()
                    && self.isin == other.isin
                    && self.exchange.as_ref().map(|exchange| &exchange.code)
                        == other.exchange.as_ref().map(|exchange| &exchange.code)
                    && self.currency == other.currency
            }
            AssetKind::Crypto | AssetKind::Custom => {
                self.reference.to_uppercase() == other.reference.to_uppercase()
            }
            AssetKind::Cash => false,
        }
    }

    fn validate(
        name: &str,
        risk_level: u8,
        currency: &str,
        reference: &str,
        exchange: Option<&Exchange>,
    ) -> StdResult<(), AssetError> {
        if name.trim().is_empty() {
            return Err(AssetError::NameEmpty);
        }
        if reference.trim().is_empty() {
            return Err(AssetError::ReferenceEmpty);
        }
        if !RISK_LEVELS.contains(&risk_level) {
            return Err(AssetError::InvalidRiskLevel {
                received: risk_level,
            });
        }
        if Currency::from_str(currency).is_err() {
            return Err(AssetError::InvalidCurrency {
                currency: currency.to_string(),
            });
        }
        if let Some(exchange) = exchange {
            if exchange::lookup(&exchange.code).is_none() {
                return Err(AssetError::InvalidExchange {
                    exchange_code: exchange.code.clone(),
                });
            }
        }
        Ok(())
    }

    /// Restores an Asset from storage (no validation — already validated at write time).
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        asset_id: String,
        kind: AssetKind,
        name: String,
        class: AssetClass,
        category: AssetCategory,
        currency: String,
        risk_level: u8,
        reference: String,
        isin: Option<String>,
        is_archived: bool,
        exchange: Option<Exchange>,
        price_refresh_blocked: bool,
        interest_bearing: bool,
    ) -> Self {
        Self {
            id: asset_id,
            kind,
            name,
            class,
            category,
            currency,
            risk_level,
            reference,
            isin,
            is_archived,
            exchange,
            price_refresh_blocked,
            interest_bearing,
        }
    }

    /// Returns true if this is the system Cash Asset (CSH-016 / CSH-017).
    pub(crate) fn is_cash(&self) -> bool {
        self.class == AssetClass::Cash
    }

    /// Aggregate-level invariant: the system Cash Asset cannot be edited, archived,
    /// unarchived, or deleted by the user (CSH-016).
    pub fn ensure_user_managed(&self) -> Result<(), AssetError> {
        if self.is_cash() {
            return Err(AssetError::CashAssetNotEditable);
        }
        Ok(())
    }

    /// Aggregate-level invariant: an archived asset cannot be edited (R6). Archive
    /// must be reverted (`unarchive`) first.
    pub fn ensure_not_archived(&self) -> Result<(), AssetError> {
        if self.is_archived {
            return Err(AssetError::Archived);
        }
        Ok(())
    }

    /// Aggregate root method: applies an edit to this asset. Enforces the
    /// system-asset (CSH-016) and not-archived (R6) invariants on the loaded
    /// state, refuses an edit into the application's cash (AST-031), then
    /// validates the proposed input. Returns the updated `Asset`
    /// for the caller to persist.
    // The argument count mirrors the field set of Asset and matches the
    // existing `with_id` factory (B32 precedent above). It cannot be split
    // without introducing an intermediate value object.
    #[allow(clippy::too_many_arguments)]
    pub fn update_from(
        self,
        kind: AssetKind,
        name: String,
        class: AssetClass,
        category: AssetCategory,
        currency: String,
        risk_level: u8,
        reference: String,
        isin: Option<String>,
        exchange: Option<Exchange>,
        interest_bearing: bool,
    ) -> Result<Self, AssetError> {
        self.ensure_user_managed()?;
        self.ensure_not_archived()?;
        if kind == AssetKind::Cash || class == AssetClass::Cash {
            return Err(AssetError::CashAssetNotEditable);
        }
        Self::validate(&name, risk_level, &currency, &reference, exchange.as_ref())?;
        let reference = reference.trim().to_uppercase();
        let isin = normalize_optional_isin(isin)?;
        Self::ensure_kind_allows(kind, &class, isin.as_deref(), exchange.as_ref())?;
        Ok(Self {
            id: self.id,
            kind,
            name,
            class,
            category,
            currency,
            risk_level,
            reference,
            isin,
            is_archived: self.is_archived,
            exchange,
            price_refresh_blocked: self.price_refresh_blocked,
            interest_bearing,
        })
    }

    /// Aggregate root method: archives this asset (R6 — reversible).
    /// Enforces the system-asset invariant (CSH-016).
    pub fn archive(self) -> Result<Self, AssetError> {
        self.ensure_user_managed()?;
        Ok(Self {
            is_archived: true,
            ..self
        })
    }

    /// Aggregate root method: unarchives this asset (R18). Enforces the
    /// system-asset invariant (CSH-016).
    pub fn unarchive(self) -> Result<Self, AssetError> {
        self.ensure_user_managed()?;
        Ok(Self {
            is_archived: false,
            ..self
        })
    }

    /// Aggregate root method: blocks automated price fetches for this asset
    /// (MKT-150 / MKT-151, ADR-014 — the lock). Enforces the system-asset
    /// invariant (CSH-016 / MKT-154). Idempotent.
    pub fn block_price_refresh(self) -> Result<Self, AssetError> {
        self.ensure_user_managed()?;
        Ok(Self {
            price_refresh_blocked: true,
            ..self
        })
    }

    /// Aggregate root method: re-allows automated price fetches for this asset
    /// (MKT-156). Enforces the system-asset invariant (CSH-016 / MKT-154).
    /// Idempotent.
    pub fn unblock_price_refresh(self) -> Result<Self, AssetError> {
        self.ensure_user_managed()?;
        Ok(Self {
            price_refresh_blocked: false,
            ..self
        })
    }
}

/// Runs `validate_isin` on `Some(raw)` and returns the normalized form, or
/// passes `None` through. The sub-variants of `IsinFormatError` collapse to
/// the single domain code `InvalidIsinFormat` (per `isin.rs` doc — the wire
/// does not need sub-variant granularity); the sub-variant is preserved
/// server-side via `tracing::debug!` for diagnostics.
fn normalize_optional_isin(isin: Option<String>) -> StdResult<Option<String>, AssetError> {
    match isin {
        Some(raw) => validate_isin(&raw).map(Some).map_err(|err| {
            tracing::debug!(
                target: crate::core::logger::BACKEND,
                raw = %raw,
                err = ?err,
                "ISIN validation failed; collapsing to InvalidIsinFormat",
            );
            AssetError::InvalidIsinFormat
        }),
        None => Ok(None),
    }
}

#[cfg(test)]
mod aggregate_tests {
    use super::*;

    // The risk levels offered are exactly those an asset accepts, and every class's default
    // is one of them.
    #[test]
    fn creation_defaults_offer_the_risk_levels_an_asset_accepts() {
        let defaults = AssetCreationDefaults::current();
        let accepts = |risk_level: u8| Asset::validate("Name", risk_level, "EUR", "REF", None);

        assert_eq!(defaults.risk_levels, vec![1, 2, 3, 4, 5]);
        assert!(defaults
            .risk_levels
            .iter()
            .all(|level| accepts(*level).is_ok()));
        assert!(matches!(
            accepts(0),
            Err(AssetError::InvalidRiskLevel { received: 0 })
        ));
        assert!(matches!(
            accepts(6),
            Err(AssetError::InvalidRiskLevel { received: 6 })
        ));
        assert!(defaults
            .classes
            .iter()
            .all(|entry| defaults.risk_levels.contains(&entry.default_risk)));
    }

    // CSH-015 / R3 — what a new asset starts from: every class but Cash, each with its
    // default risk level, the preselected class among them, and the system category.
    #[test]
    fn creation_defaults_offer_every_class_but_cash_with_its_risk() {
        let defaults = AssetCreationDefaults::current();

        assert!(defaults
            .classes
            .iter()
            .all(|entry| entry.class != AssetClass::Cash));
        assert_eq!(defaults.classes.len(), 8);
        assert!(defaults
            .classes
            .iter()
            .all(|entry| entry.default_risk == entry.class.default_risk()));
        assert!(defaults.classes.iter().any(
            |entry| entry.class == defaults.class && entry.default_risk == defaults.risk_level
        ));
        assert_eq!(
            defaults.category_id,
            crate::context::asset::SYSTEM_CATEGORY_ID
        );
    }

    fn equity(id: &str, archived: bool) -> Asset {
        Asset::restore(
            id.to_string(),
            AssetKind::Custom,
            "Apple".to_string(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".to_string(),
            3,
            "AAPL".to_string(),
            None,
            archived,
            None,
            false,
            false,
        )
    }

    fn cash() -> Asset {
        Asset::restore(
            "cash-usd".to_string(),
            AssetKind::Cash,
            "USD Cash".to_string(),
            AssetClass::Cash,
            AssetCategory::default(),
            "USD".to_string(),
            1,
            "USD".to_string(),
            None,
            false,
            None,
            false,
            false,
        )
    }

    // CSH-016 — system Cash Asset cannot be edited via update_from.
    #[test]
    fn update_from_rejects_system_cash_asset() {
        let err = cash()
            .update_from(
                AssetKind::Custom,
                "Renamed".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap_err();
        assert!(matches!(err, AssetError::CashAssetNotEditable));
    }

    // R6 — archived asset cannot be edited via update_from.
    #[test]
    fn update_from_rejects_archived_asset() {
        let err = equity("a1", true)
            .update_from(
                AssetKind::Custom,
                "Renamed".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap_err();
        assert!(matches!(err, AssetError::Archived));
    }

    // update_from validates input after the state checks pass.
    #[test]
    fn update_from_rejects_empty_name() {
        let err = equity("a1", false)
            .update_from(
                AssetKind::Custom,
                "".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap_err();
        assert!(matches!(err, AssetError::NameEmpty));
    }

    // CSH-016 — system Cash Asset cannot be archived.
    #[test]
    fn archive_rejects_system_cash_asset() {
        assert!(matches!(
            cash().archive().unwrap_err(),
            AssetError::CashAssetNotEditable
        ));
    }

    // CSH-016 — system Cash Asset cannot be unarchived.
    #[test]
    fn unarchive_rejects_system_cash_asset() {
        assert!(matches!(
            cash().unarchive().unwrap_err(),
            AssetError::CashAssetNotEditable
        ));
    }

    // archive sets is_archived = true on a regular asset.
    #[test]
    fn archive_sets_archived_flag_on_user_asset() {
        let archived = equity("a1", false).archive().unwrap();
        assert!(archived.is_archived);
    }

    // unarchive clears is_archived on a regular asset.
    #[test]
    fn unarchive_clears_archived_flag_on_user_asset() {
        let unarchived = equity("a1", true).unarchive().unwrap();
        assert!(!unarchived.is_archived);
    }

    // MKT-150 — block_price_refresh sets the lock on a regular asset.
    #[test]
    fn block_price_refresh_sets_flag_on_user_asset() {
        let locked = equity("a1", false).block_price_refresh().unwrap();
        assert!(locked.price_refresh_blocked);
    }

    // MKT-156 — unblock_price_refresh clears the lock on a regular asset.
    #[test]
    fn unblock_price_refresh_clears_flag_on_user_asset() {
        let locked = equity("a1", false).block_price_refresh().unwrap();
        let unlocked = locked.unblock_price_refresh().unwrap();
        assert!(!unlocked.price_refresh_blocked);
    }

    // MKT-154 / CSH-016 — system Cash Asset cannot be locked.
    #[test]
    fn block_price_refresh_rejects_system_cash_asset() {
        assert!(matches!(
            cash().block_price_refresh().unwrap_err(),
            AssetError::CashAssetNotEditable
        ));
    }

    // MKT-154 / CSH-016 — system Cash Asset cannot be unlocked.
    #[test]
    fn unblock_price_refresh_rejects_system_cash_asset() {
        assert!(matches!(
            cash().unblock_price_refresh().unwrap_err(),
            AssetError::CashAssetNotEditable
        ));
    }

    // MKT-150 — block_price_refresh preserves all other fields (only the lock flips).
    #[test]
    fn block_price_refresh_preserves_other_fields() {
        let before = equity("a1", false);
        let after = before.clone().block_price_refresh().unwrap();
        assert_eq!(after.id, before.id);
        assert_eq!(after.name, before.name);
        assert_eq!(after.is_archived, before.is_archived);
        assert_eq!(after.reference, before.reference);
    }

    // MKT-155 — update_from preserves the price-refresh lock across an edit.
    #[test]
    fn update_from_preserves_price_refresh_lock() {
        let locked = equity("a1", false).block_price_refresh().unwrap();
        let updated = locked
            .update_from(
                AssetKind::Custom,
                "Renamed".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap();
        assert!(updated.price_refresh_blocked);
    }

    // ensure_user_managed rejects the system Cash Asset (used by delete service path).
    #[test]
    fn ensure_user_managed_rejects_system_cash_asset() {
        assert!(matches!(
            cash().ensure_user_managed().unwrap_err(),
            AssetError::CashAssetNotEditable
        ));
    }

    // update_from rewrites every mutable field while preserving id and is_archived.
    #[test]
    fn update_from_applies_all_field_changes() {
        let updated = equity("a1", false)
            .update_from(
                AssetKind::Custom,
                "Microsoft".into(),
                AssetClass::ETF,
                AssetCategory::from_storage("cat-tech".into(), "Tech".into()),
                "EUR".into(),
                2,
                "MSFT".into(),
                None,
                None,
                false,
            )
            .unwrap();
        assert_eq!(updated.id, "a1");
        assert!(!updated.is_archived);
        assert_eq!(updated.name, "Microsoft");
        assert_eq!(updated.class, AssetClass::ETF);
        assert_eq!(updated.category.id, "cat-tech");
        assert_eq!(updated.currency, "EUR");
        assert_eq!(updated.risk_level, 2);
        assert_eq!(updated.reference, "MSFT");
    }

    // update_from normalizes reference: trims whitespace and uppercases.
    #[test]
    fn update_from_normalizes_reference() {
        let updated = equity("a1", false)
            .update_from(
                AssetKind::Custom,
                "Apple".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "  msft  ".into(),
                None,
                None,
                false,
            )
            .unwrap();
        assert_eq!(updated.reference, "MSFT");
    }

    // CSH-016 takes precedence over R6: a system Cash Asset that is also archived
    // must surface CashAssetNotEditable, not Archived (cash check runs first).
    #[test]
    fn update_from_check_order_cash_before_archived() {
        let archived_cash = Asset::restore(
            "cash-usd".into(),
            AssetKind::Cash,
            "USD Cash".into(),
            AssetClass::Cash,
            AssetCategory::default(),
            "USD".into(),
            1,
            "USD".into(),
            None,
            true,
            None,
            false,
            false,
        );
        let err = archived_cash
            .update_from(
                AssetKind::Custom,
                "x".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap_err();
        assert!(matches!(err, AssetError::CashAssetNotEditable));
    }

    // archive preserves all other fields (only is_archived flips).
    #[test]
    fn archive_preserves_other_fields() {
        let before = equity("a1", false);
        let after = before.clone().archive().unwrap();
        assert_eq!(after.id, before.id);
        assert_eq!(after.name, before.name);
        assert_eq!(after.class, before.class);
        assert_eq!(after.category.id, before.category.id);
        assert_eq!(after.currency, before.currency);
        assert_eq!(after.risk_level, before.risk_level);
        assert_eq!(after.reference, before.reference);
    }

    // unarchive preserves all other fields (only is_archived flips).
    #[test]
    fn unarchive_preserves_other_fields() {
        let before = equity("a1", true);
        let after = before.clone().unarchive().unwrap();
        assert_eq!(after.id, before.id);
        assert_eq!(after.name, before.name);
        assert_eq!(after.class, before.class);
        assert_eq!(after.category.id, before.category.id);
        assert_eq!(after.currency, before.currency);
        assert_eq!(after.risk_level, before.risk_level);
        assert_eq!(after.reference, before.reference);
    }

    // AST-024 — new() carries the interest_bearing opt-in through construction.
    #[test]
    fn new_carries_interest_bearing_flag() {
        let asset = Asset::new(
            AssetKind::Custom,
            "Euro Fund".into(),
            AssetClass::MutualFunds,
            AssetCategory::default(),
            "EUR".into(),
            2,
            "EUROFUND".into(),
            None,
            None,
            true,
        )
        .unwrap();
        assert!(asset.interest_bearing);
    }

    // AST-024 — update_from can set and clear interest_bearing like any other
    // editable field.
    #[test]
    fn update_from_sets_and_clears_interest_bearing() {
        let flagged = equity("a1", false)
            .update_from(
                AssetKind::Custom,
                "Apple".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                true,
            )
            .unwrap();
        assert!(flagged.interest_bearing);

        let cleared = flagged
            .update_from(
                AssetKind::Custom,
                "Apple".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap();
        assert!(!cleared.interest_bearing);
    }
}

#[cfg(test)]
mod isin_tests {
    use super::*;

    // AST-023 — None ISIN is always valid (the field is optional).
    #[test]
    fn new_accepts_no_isin() {
        let asset = Asset::new(
            AssetKind::Custom,
            "Apple".into(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".into(),
            3,
            "AAPL".into(),
            None,
            None,
            false,
        )
        .unwrap();
        assert!(asset.isin.is_none());
    }

    // AST-023 + WEB-016 — well-formed ISIN passes validation and is stored
    // in the normalized uppercase form (trimmed + uppercased).
    #[test]
    fn new_accepts_and_normalizes_valid_isin() {
        let asset = Asset::new(
            AssetKind::Listed,
            "iShares S&P 500".into(),
            AssetClass::ETF,
            AssetCategory::default(),
            "USD".into(),
            3,
            "CSPX".into(),
            Some("  ie00b53l3w79  ".into()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(asset.isin.as_deref(), Some("IE00B53L3W79"));
    }

    // AST-023 — malformed ISIN (here: 11 chars) is rejected with
    // InvalidIsinFormat. Sub-variants of IsinFormatError collapse to the
    // single domain code per `isin.rs` doc.
    #[test]
    fn new_rejects_malformed_isin() {
        let err = Asset::new(
            AssetKind::Custom,
            "Apple".into(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".into(),
            3,
            "AAPL".into(),
            Some("IE00B53L3W7".into()),
            None,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, AssetError::InvalidIsinFormat));
    }

    // AST-023 — ISIN with a bad Luhn check digit also collapses to
    // InvalidIsinFormat (no sub-variant exposure on the wire).
    #[test]
    fn new_rejects_isin_with_bad_check_digit() {
        let err = Asset::new(
            AssetKind::Custom,
            "Apple".into(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".into(),
            3,
            "AAPL".into(),
            Some("IE00B53L3W70".into()),
            None,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, AssetError::InvalidIsinFormat));
    }

    // AST-023 — update_from can set ISIN when previously None and clear it
    // when None is passed.
    #[test]
    fn update_from_can_set_and_clear_isin() {
        let base = Asset::new(
            AssetKind::Custom,
            "Apple".into(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".into(),
            3,
            "AAPL".into(),
            None,
            None,
            false,
        )
        .unwrap();
        // Set
        let with_isin = base
            .clone()
            .update_from(
                AssetKind::Listed,
                "Apple".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                Some("US0378331005".into()),
                None,
                false,
            )
            .unwrap();
        assert_eq!(with_isin.isin.as_deref(), Some("US0378331005"));
        // Clear
        let cleared = with_isin
            .update_from(
                AssetKind::Custom,
                "Apple".into(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".into(),
                3,
                "AAPL".into(),
                None,
                None,
                false,
            )
            .unwrap();
        assert!(cleared.isin.is_none());
    }
}

#[cfg(test)]
mod exchange_tests {
    use super::super::exchange::Exchange;
    use super::*;

    /// Constructs a canonical exchange value for use in tests.
    fn xpar() -> Exchange {
        super::super::exchange::lookup("XPAR").expect("XPAR must be in the curated set")
    }

    /// Constructs a non-canonical exchange value for use in tests (AST-001 rejection path).
    fn bogus_exchange() -> Exchange {
        Exchange {
            code: "BOGUS".to_string(),
            label: "Bogus Exchange".to_string(),
        }
    }

    fn equity_with_exchange(id: &str, exchange: Option<Exchange>) -> Asset {
        Asset::restore(
            id.to_string(),
            AssetKind::Custom,
            "Apple".to_string(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".to_string(),
            3,
            "AAPL".to_string(),
            None,
            false,
            exchange,
            false,
            false,
        )
    }

    // Asset::restore round-trips exchange = None
    #[test]
    fn restore_accepts_exchange_none() {
        let asset = equity_with_exchange("a1", None);
        assert!(asset.exchange.is_none());
    }

    // Asset::restore round-trips exchange = Some(canonical)
    #[test]
    fn restore_accepts_canonical_exchange() {
        let asset = equity_with_exchange("a1", Some(xpar()));
        let exchange = asset.exchange.expect("exchange should be Some");
        assert_eq!(exchange.code, "XPAR");
    }

    // Asset::new accepts exchange = None (AST-001 — absent is always valid)
    #[test]
    fn new_accepts_no_exchange() {
        let result = Asset::new(
            AssetKind::Custom,
            "Apple".to_string(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".to_string(),
            3,
            "AAPL".to_string(),
            None,
            None,
            false,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().exchange.is_none());
    }

    // Asset::new accepts exchange = Some(canonical) (AST-001 — curated membership passes)
    #[test]
    fn new_accepts_canonical_exchange() {
        let result = Asset::new(
            AssetKind::Listed,
            "Air Liquide".to_string(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "EUR".to_string(),
            4,
            "AI".to_string(),
            Some("US0378331005".to_string()),
            Some(xpar()),
            false,
        );
        assert!(result.is_ok());
        let asset = result.unwrap();
        let exchange = asset.exchange.expect("exchange should be Some");
        assert_eq!(exchange.code, "XPAR");
    }

    // Asset::new rejects exchange = Some(non-curated) with InvalidExchange (AST-001)
    #[test]
    fn new_rejects_non_curated_exchange() {
        let err = Asset::new(
            AssetKind::Custom,
            "Some Asset".to_string(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "USD".to_string(),
            3,
            "REF".to_string(),
            None,
            Some(bogus_exchange()),
            false,
        )
        .unwrap_err();
        assert!(
            matches!(&err, AssetError::InvalidExchange { exchange_code } if exchange_code == "BOGUS"),
            "expected InvalidExchange {{ code: \"BOGUS\" }}, got: {err:?}"
        );
    }

    // update_from accepts exchange = None → no exchange after update (AST-022 clear)
    #[test]
    fn update_from_clears_exchange_when_none_passed() {
        let asset = equity_with_exchange("a1", Some(xpar()));
        let updated = asset
            .update_from(
                AssetKind::Custom,
                "Apple".to_string(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".to_string(),
                3,
                "AAPL".to_string(),
                None,
                None,
                false,
            )
            .unwrap();
        assert!(updated.exchange.is_none());
    }

    // update_from accepts exchange = Some(canonical) when currently None (AST-022 set)
    #[test]
    fn update_from_sets_exchange_when_previously_none() {
        let asset = equity_with_exchange("a1", None);
        let updated = asset
            .update_from(
                AssetKind::Listed,
                "Air Liquide".to_string(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "EUR".to_string(),
                4,
                "AI".to_string(),
                Some("US0378331005".to_string()),
                Some(xpar()),
                false,
            )
            .unwrap();
        let exchange = updated
            .exchange
            .expect("exchange should be Some after update");
        assert_eq!(exchange.code, "XPAR");
    }

    // update_from changes exchange from one canonical value to another (AST-022 change)
    #[test]
    fn update_from_changes_exchange_to_different_canonical_value() {
        let initial =
            super::super::exchange::lookup("XNAS").expect("XNAS must be in the curated set");
        let asset = equity_with_exchange("a1", Some(initial));
        let updated = asset
            .update_from(
                AssetKind::Listed,
                "Air Liquide".to_string(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "EUR".to_string(),
                4,
                "AI".to_string(),
                Some("US0378331005".to_string()),
                Some(xpar()),
                false,
            )
            .unwrap();
        let exchange = updated
            .exchange
            .expect("exchange should be Some after update");
        assert_eq!(exchange.code, "XPAR");
    }

    // update_from rejects non-curated exchange with InvalidExchange (AST-001)
    #[test]
    fn update_from_rejects_non_curated_exchange() {
        let asset = equity_with_exchange("a1", None);
        let err = asset
            .update_from(
                AssetKind::Custom,
                "Some Asset".to_string(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".to_string(),
                3,
                "REF".to_string(),
                None,
                Some(bogus_exchange()),
                false,
            )
            .unwrap_err();
        assert!(
            matches!(&err, AssetError::InvalidExchange { exchange_code } if exchange_code == "BOGUS"),
            "expected InvalidExchange {{ code: \"BOGUS\" }}, got: {err:?}"
        );
    }
}

/// Interface for asset persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AssetRepository: Send + Sync {
    /// Fetches all active (non-archived) assets.
    async fn get_all(&self) -> Result<Vec<Asset>>;
    /// Fetches all assets including archived ones.
    async fn get_all_including_archived(&self) -> Result<Vec<Asset>>;
    /// Fetches an asset by its ID.
    async fn get_by_id(&self, id: &str) -> Result<Option<Asset>>;
    /// Persists a new asset.
    async fn create(&self, asset: Asset) -> Result<Asset>;
    /// Updates an existing asset.
    async fn update(&self, asset: Asset) -> Result<Asset>;
    /// Soft-deletes an asset.
    async fn delete(&self, id: &str) -> Result<()>;
    /// Archives an asset (reversible).
    async fn archive(&self, id: &str) -> Result<()>;
    /// Unarchives an asset.
    async fn unarchive(&self, id: &str) -> Result<()>;
    /// Sets the price-refresh lock on an asset (MKT-150).
    async fn block_price_refresh(&self, id: &str) -> Result<()>;
    /// Clears the price-refresh lock on an asset (MKT-150).
    async fn unblock_price_refresh(&self, id: &str) -> Result<()>;
    /// Stamps `rank` on every asset, category, and asset price whose rank columns are still
    /// NULL (CFR-014, D6), on `conn` — the first publish's enrolment transaction (SYN-013).
    /// Returns how many rows were stamped.
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64>;

    /// The synced record of `kind` this device holds for `identity` — its rank and its
    /// content as the change capture serializes it (CFR-014) — on `conn`; `None` when it
    /// holds none. Covers assets, categories, and asset prices.
    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>>;
    /// The rank of another live category carrying `name` (case-insensitive, CFR-035), on
    /// `conn` — the lowest-ranked one, then by id, when several do; `None` when no other
    /// category does or it has never been ranked.
    async fn clashing_category_name_rank(
        &self,
        conn: &mut SqliteConnection,
        category_id: &str,
        name: &str,
    ) -> Result<Option<Rank>>;
    /// Writes `asset` verbatim — created or replaced in full, whatever its archived state —
    /// stamped with `rank` (CFR-017/014), on `conn`.
    async fn apply_asset(
        &self,
        conn: &mut SqliteConnection,
        asset: &Asset,
        rank: &Rank,
    ) -> Result<()>;
    /// Writes `category` verbatim, stamped with `rank`, on `conn` (CFR-017).
    async fn apply_category(
        &self,
        conn: &mut SqliteConnection,
        category: &AssetCategory,
        rank: &Rank,
    ) -> Result<()>;
    /// Writes an asset price verbatim, stamped with `rank`, on `conn` (CFR-050).
    async fn apply_asset_price(
        &self,
        conn: &mut SqliteConnection,
        price: &AssetPrice,
        rank: &Rank,
    ) -> Result<()>;
    /// Removes the synced record of `kind` for `identity`, on `conn`: an asset or a category
    /// is soft-deleted, a price deleted. A no-op when absent.
    async fn remove_synced(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<()>;
    /// SYN-083 — deletes every asset price, on `conn`.
    async fn discard_asset_prices(&self, conn: &mut SqliteConnection) -> Result<()>;
    /// Ensures the system-seeded `category` and `asset` exist, on `conn` (SYN-027/CSH-010);
    /// rows already present are left untouched.
    async fn ensure_seeded(
        &self,
        conn: &mut SqliteConnection,
        category: &AssetCategory,
        asset: &Asset,
    ) -> Result<()>;
}

#[cfg(test)]
mod default_risk_tests {
    use super::*;

    // R3 — the risk level a new asset of each class starts at.
    #[test]
    fn each_class_has_its_default_risk() {
        let expected = [
            (AssetClass::Cash, 1),
            (AssetClass::Bonds, 2),
            (AssetClass::RealEstate, 2),
            (AssetClass::MutualFunds, 3),
            (AssetClass::ETF, 3),
            (AssetClass::ETP, 3),
            (AssetClass::Stocks, 4),
            (AssetClass::DigitalAsset, 5),
            (AssetClass::Derivatives, 5),
        ];
        for (class, risk) in expected {
            assert_eq!(class.default_risk(), risk, "{class:?}");
        }
    }
}

#[cfg(test)]
mod kind_tests {
    use super::*;

    const ISIN: &str = "US0378331005";

    fn build(
        kind: AssetKind,
        class: AssetClass,
        reference: &str,
        isin: Option<&str>,
        exchange_code: Option<&str>,
        currency: &str,
    ) -> StdResult<Asset, AssetError> {
        Asset::new(
            kind,
            "Some asset".to_string(),
            class,
            AssetCategory::default(),
            currency.to_string(),
            3,
            reference.to_string(),
            isin.map(str::to_string),
            exchange_code.map(|code| exchange::lookup(code).expect("a curated exchange")),
            false,
        )
    }

    // AST-034 — the kind of an asset that carries none.
    #[test]
    fn ast_034_the_class_then_the_isin_decide_the_kind_of_an_asset_that_carries_none() {
        assert_eq!(AssetKind::of(&AssetClass::Cash, None), AssetKind::Cash);
        assert_eq!(
            AssetKind::of(&AssetClass::DigitalAsset, Some(ISIN)),
            AssetKind::Crypto
        );
        assert_eq!(
            AssetKind::of(&AssetClass::Stocks, Some(ISIN)),
            AssetKind::Listed
        );
        assert_eq!(
            AssetKind::of(&AssetClass::Stocks, Some("  ")),
            AssetKind::Custom
        );
        assert_eq!(
            AssetKind::of(&AssetClass::RealEstate, None),
            AssetKind::Custom
        );
    }

    // AST-031 — a listed asset has an ISIN; its exchange is optional.
    #[test]
    fn ast_031_a_listed_asset_without_an_isin_is_refused() {
        let refused = build(
            AssetKind::Listed,
            AssetClass::Stocks,
            "AAPL",
            None,
            Some("XNAS"),
            "USD",
        );
        assert!(matches!(refused, Err(AssetError::IsinRequired)));

        let listed = build(
            AssetKind::Listed,
            AssetClass::Stocks,
            "AAPL",
            Some(ISIN),
            None,
            "USD",
        )
        .expect("a listed asset with an ISIN and no exchange");
        assert_eq!(listed.kind, AssetKind::Listed);
    }

    // AST-031 — a crypto or a custom asset has neither ISIN nor exchange.
    #[test]
    fn ast_031_a_crypto_or_custom_asset_with_an_isin_or_an_exchange_is_refused() {
        for (kind, class) in [
            (AssetKind::Crypto, AssetClass::DigitalAsset),
            (AssetKind::Custom, AssetClass::RealEstate),
        ] {
            assert!(matches!(
                build(kind, class.clone(), "REF", Some(ISIN), None, "EUR"),
                Err(AssetError::IsinNotAllowed { kind: refused }) if refused == kind
            ));
            assert!(matches!(
                build(kind, class.clone(), "REF", None, Some("XPAR"), "EUR"),
                Err(AssetError::ExchangeNotAllowed { kind: refused }) if refused == kind
            ));
            let accepted = build(kind, class, "REF", None, None, "EUR").expect("neither");
            assert_eq!(accepted.kind, kind);
        }
    }

    // AST-031 — the Cash class goes with the cash kind and the digital-asset class with the
    // crypto kind, and neither with another.
    #[test]
    fn ast_031_a_class_and_a_kind_that_do_not_go_together_are_refused() {
        for (kind, class) in [
            (AssetKind::Cash, AssetClass::Stocks),
            (AssetKind::Crypto, AssetClass::Stocks),
            (AssetKind::Listed, AssetClass::DigitalAsset),
            (AssetKind::Custom, AssetClass::Cash),
        ] {
            let isin = (kind == AssetKind::Listed).then_some(ISIN);
            assert!(
                matches!(
                    build(kind, class.clone(), "REF", isin, None, "EUR"),
                    Err(AssetError::ClassNotAllowed { kind: refused, class: of })
                        if refused == kind && of == class
                ),
                "{kind} with {class}"
            );
        }
        let cash = build(AssetKind::Cash, AssetClass::Cash, "EUR", None, None, "EUR")
            .expect("the application's cash");
        assert_eq!(cash.kind_problem().map(|problem| problem.to_string()), None);
    }

    // AST-031 — changing an asset checks the rules of the kind it is given.
    #[test]
    fn ast_031_an_edit_is_held_to_the_rules_of_the_kind_it_gives() {
        let custom = build(
            AssetKind::Custom,
            AssetClass::Stocks,
            "AAPL",
            None,
            None,
            "USD",
        )
        .expect("custom");
        let edit = |kind: AssetKind, isin: Option<&str>| {
            custom.clone().update_from(
                kind,
                "Apple".to_string(),
                AssetClass::Stocks,
                AssetCategory::default(),
                "USD".to_string(),
                3,
                "AAPL".to_string(),
                isin.map(str::to_string),
                None,
                false,
            )
        };
        assert!(matches!(
            edit(AssetKind::Custom, Some(ISIN)),
            Err(AssetError::IsinNotAllowed {
                kind: AssetKind::Custom
            })
        ));
        assert!(matches!(
            edit(AssetKind::Listed, None),
            Err(AssetError::IsinRequired)
        ));
        let listed = edit(AssetKind::Listed, Some(ISIN)).expect("the kind may change");
        assert_eq!(listed.kind, AssetKind::Listed);
        assert_eq!(listed.id, custom.id);
    }

    // AST-031 — a user's asset is never edited into the application's cash, by its kind or
    // by its class.
    #[test]
    fn ast_031_an_edit_never_makes_an_asset_the_application_s_cash() {
        let custom = build(
            AssetKind::Custom,
            AssetClass::Stocks,
            "MINE",
            None,
            None,
            "EUR",
        )
        .expect("custom");
        for (kind, class) in [
            (AssetKind::Cash, AssetClass::Cash),
            (AssetKind::Cash, AssetClass::Stocks),
            (AssetKind::Custom, AssetClass::Cash),
        ] {
            let edited = custom.clone().update_from(
                kind,
                "Mine".to_string(),
                class,
                AssetCategory::default(),
                "EUR".to_string(),
                1,
                "EUR".to_string(),
                None,
                None,
                false,
            );
            assert!(matches!(edited, Err(AssetError::CashAssetNotEditable)));
        }
    }

    // AST-035 — an asset restored from storage may break a rule of its kind, and says which.
    #[test]
    fn ast_035_a_stored_asset_names_the_rule_of_its_kind_it_breaks() {
        let custom_on_an_exchange = Asset::restore(
            "a1".to_string(),
            AssetKind::Custom,
            "Old".to_string(),
            AssetClass::Stocks,
            AssetCategory::default(),
            "EUR".to_string(),
            3,
            "OLD".to_string(),
            None,
            false,
            exchange::lookup("XPAR"),
            false,
            false,
        );
        assert!(matches!(
            custom_on_an_exchange.kind_problem(),
            Some(AssetError::ExchangeNotAllowed {
                kind: AssetKind::Custom
            })
        ));
    }

    // AST-032 — two listed assets are the same when they share ISIN, exchange and currency.
    #[test]
    fn ast_032_two_listings_are_the_same_only_with_isin_exchange_and_currency() {
        let listing = |exchange_code: Option<&str>, currency: &str| {
            build(
                AssetKind::Listed,
                AssetClass::Stocks,
                "ASML",
                Some(ISIN),
                exchange_code,
                currency,
            )
            .expect("listed")
        };
        let amsterdam = listing(Some("XAMS"), "EUR");
        assert!(amsterdam.is_same_as(&listing(Some("XAMS"), "EUR")));
        assert!(!amsterdam.is_same_as(&listing(Some("XNAS"), "EUR")));
        assert!(!amsterdam.is_same_as(&listing(Some("XAMS"), "USD")));
        assert!(!amsterdam.is_same_as(&listing(None, "EUR")));
        assert!(listing(None, "EUR").is_same_as(&listing(None, "EUR")));
        assert!(!amsterdam.is_same_as(&amsterdam));
    }

    // AST-032 — two crypto assets are the same by symbol, two custom ones by reference,
    // without case; assets of two kinds are never the same.
    #[test]
    fn ast_032_crypto_and_custom_assets_are_the_same_by_reference_within_their_kind() {
        let crypto = |symbol: &str| {
            build(
                AssetKind::Crypto,
                AssetClass::DigitalAsset,
                symbol,
                None,
                None,
                "EUR",
            )
            .expect("crypto")
        };
        let custom = |reference: &str, currency: &str| {
            build(
                AssetKind::Custom,
                AssetClass::RealEstate,
                reference,
                None,
                None,
                currency,
            )
            .expect("custom")
        };
        assert!(crypto("BTC").is_same_as(&crypto("btc")));
        assert!(!crypto("BTC").is_same_as(&crypto("ETH")));
        assert!(custom("flat-paris", "EUR").is_same_as(&custom("FLAT-PARIS", "USD")));
        assert!(!custom("FLAT-PARIS", "EUR").is_same_as(&custom("FLAT-LYON", "EUR")));
        assert!(!crypto("BTC").is_same_as(&custom("BTC", "EUR")));
    }

    // AST-032 — the application's cash is never compared.
    #[test]
    fn ast_032_cash_is_never_the_same_as_another_asset() {
        let cash = |id: &str| Asset {
            id: id.to_string(),
            ..build(AssetKind::Cash, AssetClass::Cash, "EUR", None, None, "EUR").expect("cash")
        };
        assert!(!cash("system-cash-eur").is_same_as(&cash("another")));
    }
}
