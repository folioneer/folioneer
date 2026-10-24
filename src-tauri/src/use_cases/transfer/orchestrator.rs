use super::error::{TransferError, TransferTask};
use crate::context::account::{
    AccountError, AccountServiceContract, AgentRecording, Transaction, TransferPreview,
    TransferView,
};
use crate::context::asset::{AssetClass, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use std::sync::Arc;

/// What a transfer is recorded or corrected with (TRF-011, TRF-022).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferFigures {
    /// Business date (YYYY-MM-DD).
    pub date: String,
    /// Quantity leaving the sending account (micro-units).
    pub quantity_sent: i64,
    /// Quantity reaching the receiving account (micro-units), not above the quantity sent.
    pub quantity_received: i64,
    /// Rate from the sending account's currency to the receiving one's (micros); `None`
    /// takes the stored rate of the date (TRF-022).
    pub exchange_rate: Option<i64>,
    /// Note carried by every part.
    pub note: Option<String>,
}

/// Why an agent's transfer was not recorded and marked (AGT-049).
#[derive(Debug)]
pub enum AgentTransferError {
    /// The transfer was refused as the window's is.
    Refused(TransferError),
    /// A part could not be marked.
    NotMarked {
        /// The transfer was removed again.
        undone: bool,
    },
}

/// Cross-BC orchestrator of the transfer (TRF).
pub struct TransferUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl TransferUseCase {
    /// Creates a new TransferUseCase.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            currency_service,
        }
    }

    /// Records a transfer of `asset_id` from one account to another (TRF-010 to TRF-022):
    /// every part, the fee in kind first when there is one.
    pub async fn record_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        figures: TransferFigures,
    ) -> Result<Vec<Transaction>, TransferError> {
        self.ensure_transferable(&asset_id).await?;
        let rate = self.rate(from_account_id, to_account_id, &figures).await?;
        Ok(self
            .account_service
            .record_transfer(
                from_account_id,
                to_account_id,
                asset_id,
                figures.date,
                figures.quantity_sent,
                figures.quantity_received,
                rate,
                figures.note,
            )
            .await?)
    }

    /// AGT-045 / AGT-049 — records a transfer as `record_transfer` does and marks every part
    /// with the agent's session; when a part cannot be marked, the transfer is undone.
    pub async fn record_transfer_by_agent(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        figures: TransferFigures,
        mark: &AgentRecording,
    ) -> Result<Vec<Transaction>, AgentTransferError> {
        let parts = self
            .record_transfer(from_account_id, to_account_id, asset_id, figures)
            .await
            .map_err(AgentTransferError::Refused)?;
        for part in &parts {
            let Err(error) = self
                .account_service
                .mark_agent_recording(&part.id, mark)
                .await
            else {
                continue;
            };
            tracing::error!(target: BACKEND, transaction = %part.id, err = ?error, "agent transfer: a part not marked, undoing the transfer");
            let Some(transfer_id) = part.transfer_id.as_deref() else {
                return Err(AgentTransferError::NotMarked { undone: false });
            };
            let undone = self.account_service.cancel_transfer(transfer_id).await;
            if let Err(error) = &undone {
                tracing::error!(target: BACKEND, transfer = %transfer_id, err = ?error, "agent transfer: not marked and not undone");
            }
            return Err(AgentTransferError::NotMarked {
                undone: undone.is_ok(),
            });
        }
        Ok(parts)
    }

    /// AGT-065A — what `record_transfer` would make of the two accounts, with the same
    /// checks and the same rate, nothing written.
    pub async fn preview_transfer(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        asset_id: String,
        figures: TransferFigures,
    ) -> Result<TransferPreview, TransferError> {
        self.ensure_transferable(&asset_id).await?;
        let rate = self.rate(from_account_id, to_account_id, &figures).await?;
        Ok(self
            .account_service
            .preview_transfer(
                from_account_id,
                to_account_id,
                asset_id,
                figures.date,
                figures.quantity_sent,
                figures.quantity_received,
                rate,
                figures.note,
            )
            .await?)
    }

    /// Corrects the transfer `transfer_id` as one operation (TRF-030); a correction that
    /// names no rate takes the stored rate of its date again.
    pub async fn correct_transfer(
        &self,
        transfer_id: &str,
        figures: TransferFigures,
    ) -> Result<Vec<Transaction>, TransferError> {
        let (from, to, asset_id) = self.account_service.transfer_sides(transfer_id).await?;
        self.ensure_transferable(&asset_id).await?;
        let rate = self.rate(&from, &to, &figures).await?;
        Ok(self
            .account_service
            .correct_transfer(
                transfer_id,
                figures.date,
                figures.quantity_sent,
                figures.quantity_received,
                rate,
                figures.note,
            )
            .await?)
    }

    /// Removes every part of the transfer `transfer_id` together (TRF-040).
    pub async fn cancel_transfer(&self, transfer_id: &str) -> Result<(), TransferError> {
        Ok(self.account_service.cancel_transfer(transfer_id).await?)
    }

    /// TRF-060 — a transfer read as one from its parts.
    pub async fn get_transfer(&self, transfer_id: &str) -> Result<TransferView, TransferError> {
        Ok(self.account_service.transfer_view(transfer_id).await?)
    }

    /// TRF-061 — the rate a transfer between the two accounts on `date` takes when none is
    /// given: one within a currency, else the stored rate on or before the date, or none.
    pub async fn transfer_rate(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        date: &str,
    ) -> Result<Option<i64>, TransferError> {
        let from = self.currency_of(from_account_id).await?;
        let to = self.currency_of(to_account_id).await?;
        Transaction::validate_date(date)?;
        Ok(self
            .currency_service
            .resolve_rate_micros(&from, &to, date)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, from = %from, to = %to, err = ?e, "transfer_rate: resolve_rate failed");
                AccountError::DatabaseError
            })?)
    }

    /// TRF-010 — the asset exists, is not archived and is not a Cash Asset.
    async fn ensure_transferable(&self, asset_id: &str) -> Result<(), TransferError> {
        let asset = self
            .asset_service
            .get_asset_by_id(asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "ensure_transferable: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => Err(TransferTask::AssetNotFound.into()),
            Some(asset) if asset.class == AssetClass::Cash => {
                Err(TransferTask::TransferOfCash.into())
            }
            Some(asset) if asset.is_archived => Err(TransferTask::ArchivedAsset.into()),
            Some(_) => Ok(()),
        }
    }

    /// TRF-022 — the rate given or, with none, the most recent stored rate on or before the
    /// date from the sending account's currency to the receiving one's.
    async fn rate(
        &self,
        from_account_id: &str,
        to_account_id: &str,
        figures: &TransferFigures,
    ) -> Result<i64, TransferError> {
        let from = self.currency_of(from_account_id).await?;
        let to = self.currency_of(to_account_id).await?;
        // TRF-011 — a malformed or out-of-bounds date is named as such, not as a missing rate.
        Transaction::validate_date(&figures.date)?;
        if let Some(rate) = figures.exchange_rate {
            return Ok(rate);
        }
        let resolved = self
            .currency_service
            .resolve_rate_micros(&from, &to, &figures.date)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, from = %from, to = %to, err = ?e, "transfer rate: resolve_rate failed");
                AccountError::DatabaseError
            })?;
        resolved.ok_or_else(|| {
            AccountError::TransferRateMissing {
                from_currency: from,
                to_currency: to,
            }
            .into()
        })
    }

    async fn currency_of(&self, account_id: &str) -> Result<String, TransferError> {
        Ok(self
            .account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?
            .currency)
    }
}
