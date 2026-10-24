use serde::Serialize;
use specta::Type;

use super::transaction::{Transaction, TransactionType};

/// A transfer read as one from its parts (TRF-060): it is not stored on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct TransferView {
    /// Identifies the operation.
    pub transfer_id: String,
    /// The sending account: the transfer out's.
    pub from_account_id: String,
    /// The receiving account: the transfer in's.
    pub to_account_id: String,
    /// The asset moved.
    pub asset_id: String,
    /// The day of the transfer: the transfer in's (TRF-032).
    pub date: String,
    /// What left the sending account: the transfer out's quantity plus its fee in kind's
    /// (micro-units).
    pub quantity_sent: i64,
    /// What reached the receiving account: the transfer in's quantity (micro-units).
    pub quantity_received: i64,
    /// The cost the transfer out moved, in the sending account's currency (micro-units).
    pub cost: i64,
    /// The rate used between the two accounts' currencies: the transfer in's (micros).
    pub exchange_rate: i64,
    /// The transfer in's note.
    pub note: Option<String>,
}

impl TransferView {
    /// TRF-060 — the transfer `transfer_id` read from its parts, in every account holding
    /// one; none when it has lost a side (TRF-041).
    pub fn from_parts(transfer_id: &str, parts: &[Transaction]) -> Option<Self> {
        let of = |kind: TransactionType| {
            parts.iter().find(|part| {
                part.transaction_type == kind && part.transfer_id.as_deref() == Some(transfer_id)
            })
        };
        let sent = of(TransactionType::TransferOut)?;
        let received = of(TransactionType::TransferIn)?;
        let fee = of(TransactionType::WriteOff).map_or(0, |fee| fee.quantity);
        Some(Self {
            transfer_id: transfer_id.to_string(),
            from_account_id: sent.account_id.clone(),
            to_account_id: received.account_id.clone(),
            asset_id: sent.asset_id.clone(),
            date: received.date.clone(),
            quantity_sent: sent.quantity.saturating_add(fee),
            quantity_received: received.quantity,
            cost: sent.total_amount,
            exchange_rate: received.exchange_rate,
            note: received.note.clone(),
        })
    }
}
