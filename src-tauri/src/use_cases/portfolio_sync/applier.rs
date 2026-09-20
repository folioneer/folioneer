//! `ServiceChangeApplier` — the `ChangeApplier` port over the account, asset, and currency
//! services (ADR-004, CFR-017): routes each synced record kind to the service that owns it,
//! reads the state it holds, and writes prevailing changes through the services' apply
//! entry points, which run no entry guards. The cash asset a change refers to is seeded
//! first (SYN-027/CFR-033). Every call rides the apply transaction's connection (SYN-065).

use std::sync::Arc;

use sqlx::SqliteConnection;

use crate::context::account::AccountService;
use crate::context::asset::AssetService;
use crate::context::currency::CurrencyService;
use crate::context::sync::{Change, ChangeApplier, SyncError};
use crate::core::cash::is_cash_asset;
use crate::core::logger::BACKEND;
use crate::shared::domain::{Operation, Rank, RecordKind, SyncedChild, SyncedRecord};

/// Writes synced records through the owning bounded contexts' services.
pub struct ServiceChangeApplier {
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
}

/// The currency of a system cash asset id (`system-cash-<ccy>`), when `asset_id` is one.
fn cash_currency(asset_id: &str) -> Option<String> {
    is_cash_asset(asset_id)
        .then(|| asset_id.rsplit('-').next().map(str::to_uppercase))
        .flatten()
}

impl ServiceChangeApplier {
    /// Creates the applier over the three services that own synced records.
    pub fn new(
        account_service: Arc<AccountService>,
        asset_service: Arc<AssetService>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            currency_service,
        }
    }

    /// SYN-027/CFR-033 — seeds the cash asset (and cash category) `currency` needs before a
    /// change referring to it is written.
    async fn ensure_cash_asset(
        &self,
        conn: &mut SqliteConnection,
        currency: Option<String>,
    ) -> Result<(), SyncError> {
        if let Some(currency) = currency {
            self.asset_service
                .ensure_cash_asset(conn, &currency)
                .await
                .map_err(|error| SyncError::database("apply: cash asset not seeded", error))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ChangeApplier for ServiceChangeApplier {
    async fn live_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>, SyncError> {
        match kind {
            RecordKind::Account
            | RecordKind::Transaction
            | RecordKind::HoldingNote
            | RecordKind::FeeSchedule
            | RecordKind::FeeCatchUpPosition => self
                .account_service
                .synced_record(conn, kind, identity)
                .await
                .map_err(|error| SyncError::database("apply: account record not read", error)),
            RecordKind::Category | RecordKind::Asset | RecordKind::AssetPrice => self
                .asset_service
                .synced_record(conn, kind, identity)
                .await
                .map_err(|error| SyncError::database("apply: asset record not read", error)),
            RecordKind::CurrencyPair | RecordKind::CurrencyRate => self
                .currency_service
                .synced_record(conn, kind, identity)
                .await
                .map_err(|error| SyncError::database("apply: currency record not read", error)),
        }
    }

    async fn children_of_account(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
    ) -> Result<Vec<SyncedChild>, SyncError> {
        self.account_service
            .synced_children(conn, account_id)
            .await
            .map_err(|error| SyncError::database("apply: account children not read", error))
    }

    async fn clashing_name(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
        name: &str,
    ) -> Result<Option<Rank>, SyncError> {
        match kind {
            RecordKind::Account => self
                .account_service
                .clashing_account_name(conn, identity, name)
                .await
                .map_err(|error| SyncError::database("apply: account names not read", error)),
            RecordKind::Category => self
                .asset_service
                .clashing_category_name(conn, identity, name)
                .await
                .map_err(|error| SyncError::database("apply: category names not read", error)),
            _ => Ok(None),
        }
    }

    async fn write(&self, conn: &mut SqliteConnection, change: &Change) -> Result<(), SyncError> {
        let kind = change.record_kind;
        // reviewer-security FP: identity always equals the content's canonical identity —
        // `check_received_change` rejected every other change at read time.
        let identity = change.record_identity.as_str();
        if change.operation == Operation::Removed {
            return match kind {
                RecordKind::Account
                | RecordKind::Transaction
                | RecordKind::HoldingNote
                | RecordKind::FeeSchedule
                | RecordKind::FeeCatchUpPosition => self
                    .account_service
                    .apply_removal(conn, kind, identity)
                    .await
                    .map_err(|error| {
                        SyncError::database("apply: account removal not written", error)
                    }),
                RecordKind::Category | RecordKind::Asset | RecordKind::AssetPrice => self
                    .asset_service
                    .apply_removal(conn, kind, identity)
                    .await
                    .map_err(|error| {
                        SyncError::database("apply: asset removal not written", error)
                    }),
                RecordKind::CurrencyPair | RecordKind::CurrencyRate => self
                    .currency_service
                    .apply_removal(conn, kind, identity)
                    .await
                    .map_err(|error| {
                        SyncError::database("apply: currency removal not written", error)
                    }),
            };
        }
        let Some(content) = change.content.as_deref() else {
            tracing::error!(target: BACKEND, identity, "apply: a creation or update carries no content");
            return Err(SyncError::DatabaseError);
        };
        let rank = change.rank();
        match kind {
            RecordKind::Account => {
                self.ensure_cash_asset(conn, change.content_field("currency"))
                    .await?;
                self.account_service
                    .apply_account(conn, content, rank)
                    .await
            }
            RecordKind::Transaction => {
                let cash = change
                    .content_field("asset_id")
                    .and_then(|id| cash_currency(&id));
                self.ensure_cash_asset(conn, cash).await?;
                self.account_service
                    .apply_transaction(conn, content, rank)
                    .await
            }
            RecordKind::HoldingNote => {
                self.account_service
                    .apply_holding_note(conn, content, rank)
                    .await
            }
            RecordKind::FeeSchedule => {
                self.account_service
                    .apply_fee_schedule(conn, content, rank)
                    .await
            }
            RecordKind::FeeCatchUpPosition => {
                self.account_service
                    .apply_catch_up_position(conn, content, rank)
                    .await
            }
            RecordKind::Category => {
                return self
                    .asset_service
                    .apply_category(conn, content, rank)
                    .await
                    .map_err(|error| SyncError::database("apply: category not written", error));
            }
            RecordKind::Asset => {
                return self
                    .asset_service
                    .apply_asset(conn, content, rank)
                    .await
                    .map_err(|error| SyncError::database("apply: asset not written", error));
            }
            RecordKind::AssetPrice => {
                return self
                    .asset_service
                    .apply_asset_price(conn, content, rank)
                    .await
                    .map_err(|error| SyncError::database("apply: asset price not written", error));
            }
            RecordKind::CurrencyPair => {
                return self
                    .currency_service
                    .apply_currency_pair(conn, content, rank)
                    .await
                    .map_err(|error| {
                        SyncError::database("apply: currency pair not written", error)
                    });
            }
            RecordKind::CurrencyRate => {
                return self
                    .currency_service
                    .apply_currency_rate(conn, content, rank)
                    .await
                    .map_err(|error| {
                        SyncError::database("apply: currency rate not written", error)
                    });
            }
        }
        .map_err(|error| SyncError::database("apply: account record not written", error))
    }

    async fn discard_observations(&self, conn: &mut SqliteConnection) -> Result<(), SyncError> {
        self.asset_service
            .discard_asset_prices(conn)
            .await
            .map_err(|error| SyncError::database("apply: asset prices not discarded", error))?;
        self.currency_service
            .discard_pairs_and_rates(conn)
            .await
            .map_err(|error| {
                SyncError::database("apply: currency pairs and rates not discarded", error)
            })
    }
}
