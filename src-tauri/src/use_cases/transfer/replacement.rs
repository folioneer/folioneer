use super::error::TransferError;
use super::orchestrator::{TransferFigures, TransferUseCase};
use crate::context::account::{Transaction, TransactionType, TransferPreview};

/// AGT-090 — four transactions the owner typed that record a transfer the four-step way, in
/// the order the owner is shown them.
#[derive(Debug, Clone)]
pub struct FourSteps {
    /// The sale, in the sending account.
    pub sale: Transaction,
    /// The withdrawal of its proceeds, in the sending account.
    pub withdrawal: Transaction,
    /// The deposit of them, in the receiving account.
    pub deposit: Transaction,
    /// The purchase of the asset again, in the receiving account.
    pub purchase: Transaction,
}

impl FourSteps {
    /// AGT-090 — `four` read as a transfer recorded the four-step way: a sale and a
    /// withdrawal in one account, a deposit and a purchase in another, the sale and the
    /// purchase of one asset, whatever their dates and amounts. Otherwise the reason, naming
    /// what the four lack.
    pub fn of(four: Vec<Transaction>) -> Result<Self, String> {
        let mut four = four;
        let mut take = |kind: TransactionType, name: &str| {
            let at = four.iter().position(|t| t.transaction_type == kind);
            at.map(|at| four.remove(at))
                .ok_or_else(|| format!("the four transactions hold no {name}"))
        };
        let sale = take(TransactionType::Sell, "sale")?;
        let withdrawal = take(TransactionType::Withdrawal, "withdrawal")?;
        let deposit = take(TransactionType::Deposit, "deposit")?;
        let purchase = take(TransactionType::Purchase, "purchase")?;
        if withdrawal.account_id != sale.account_id {
            return Err("the withdrawal is not in the sale's account".to_string());
        }
        if deposit.account_id != purchase.account_id {
            return Err("the deposit is not in the purchase's account".to_string());
        }
        if sale.account_id == purchase.account_id {
            return Err("the sale and the purchase are in one account: a transfer moves a holding to another".to_string());
        }
        if sale.asset_id != purchase.asset_id {
            return Err("the sale and the purchase are not of one asset".to_string());
        }
        Ok(Self {
            sale,
            withdrawal,
            deposit,
            purchase,
        })
    }

    /// The ids of the four, in the order they are shown.
    pub fn ids(&self) -> Vec<String> {
        [&self.sale, &self.withdrawal, &self.deposit, &self.purchase]
            .iter()
            .map(|t| t.id.clone())
            .collect()
    }
}

/// AGT-090 — the transfer four transactions become, checked on the two accounts as they
/// would stand, nothing written.
#[derive(Debug, Clone)]
pub struct Replacement {
    /// The four it replaces.
    pub steps: FourSteps,
    /// The rate it is recorded at: the one given, or the one TRF-022 takes for its date.
    pub exchange_rate: i64,
    /// The two accounts without the four and with the transfer, and its parts.
    pub preview: TransferPreview,
}

impl Replacement {
    /// The cost the transfer moves, in the sending account's currency.
    pub fn cost(&self) -> i64 {
        cost_of(&self.preview.parts)
    }
}

/// The cost a transfer's parts move: its transfer out's.
fn cost_of(parts: &[Transaction]) -> i64 {
    parts
        .iter()
        .find(|t| t.transaction_type == TransactionType::TransferOut)
        .map_or(0, |t| t.total_amount)
}

/// Why four transactions are not replaced with a transfer before the owner is asked
/// (AGT-090).
#[derive(Debug)]
pub enum ReplacementError {
    /// They are not a transfer recorded the four-step way: the reason.
    NotATransfer(String),
    /// The transfer they become is refused as the window's is.
    Refused(TransferError),
}

impl From<TransferError> for ReplacementError {
    fn from(error: TransferError) -> Self {
        Self::Refused(error)
    }
}

impl TransferUseCase {
    /// AGT-090 — the transfer `four` become: from the sale's account to the purchase's, of
    /// their asset, on the sale's date, the sale's quantity sent and the purchase's
    /// received, at `exchange_rate` or the rate TRF-022 takes; checked as the window's
    /// transfer is on the accounts without the four, nothing written.
    pub async fn ask_replacement(
        &self,
        four: Vec<Transaction>,
        exchange_rate: Option<i64>,
    ) -> Result<Replacement, ReplacementError> {
        let steps = FourSteps::of(four).map_err(ReplacementError::NotATransfer)?;
        let (from, to, asset_id) = sides(&steps);
        self.ensure_transferable(&asset_id).await?;
        let figures = figures_of(&steps, exchange_rate);
        let exchange_rate = self.rate(&from, &to, &figures).await?;
        let preview = self
            .account_service()
            .preview_replacement(
                &steps.ids(),
                &from,
                &to,
                asset_id,
                figures.date,
                figures.quantity_sent,
                figures.quantity_received,
                exchange_rate,
            )
            .await
            .map_err(TransferError::from)?;
        Ok(Replacement {
            steps,
            exchange_rate,
            preview,
        })
    }

    /// AGT-091 — replaces the four of `asked` with its transfer in one write, at the rate
    /// the owner was shown, only while each of the four stands in full as it was shown and
    /// the transfer still moves the cost shown; `None` when not, nothing written.
    pub async fn replace(
        &self,
        asked: &Replacement,
    ) -> Result<Option<Vec<Transaction>>, TransferError> {
        let steps = &asked.steps;
        let (from, to, asset_id) = sides(steps);
        self.ensure_transferable(&asset_id).await?;
        let figures = figures_of(steps, Some(asked.exchange_rate));
        let shown = [
            steps.sale.clone(),
            steps.withdrawal.clone(),
            steps.deposit.clone(),
            steps.purchase.clone(),
        ];
        Ok(self
            .account_service()
            .replace_with_transfer(
                &shown,
                &from,
                &to,
                asset_id,
                figures.date,
                figures.quantity_sent,
                figures.quantity_received,
                asked.exchange_rate,
                asked.cost(),
            )
            .await?)
    }
}

/// The sending account, the receiving account and the asset of the transfer `steps` stand for.
fn sides(steps: &FourSteps) -> (String, String, String) {
    (
        steps.sale.account_id.clone(),
        steps.purchase.account_id.clone(),
        steps.sale.asset_id.clone(),
    )
}

/// AGT-090 — the figures of the transfer `steps` stand for, with no note.
fn figures_of(steps: &FourSteps, exchange_rate: Option<i64>) -> TransferFigures {
    TransferFigures {
        date: steps.sale.date.clone(),
        quantity_sent: steps.sale.quantity,
        quantity_received: steps.purchase.quantity,
        exchange_rate,
        note: None,
    }
}
