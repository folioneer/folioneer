//! Reading by what a person named, and writing nothing. The transactions of the account
//! named (AGT-070): the rows a filter keeps of the account journal the window reads
//! (TXL-060), oldest first and one page at a time, each with the reference of its asset and
//! the mark of the agent session that recorded it. The price of the asset named at a date
//! (AGT-071), and its prices over a period, one page at a time (AGT-075).

use std::collections::HashMap;

use chrono::NaiveDate;

use crate::context::account::{AgentRecording, JournalFilter, TransactionType, TransactionView};
use crate::context::asset::{Asset, AssetPrice};
use crate::core::BACKEND;

use super::named_recording::{refusal_from_lookup, Refusal};
use super::orchestrator::HoldingTransactionUseCase;

/// AGT-070 — how many transactions one page of a listing holds.
pub const TRANSACTIONS_PER_PAGE: usize = 100;

/// AGT-075 — how many prices one page of a price history holds.
pub const PRICES_PER_PAGE: usize = 100;

/// Who recorded the transactions a listing keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedBy {
    /// The owner: a transaction with no mark on this computer (AGT-045).
    Owner,
    /// An agent, whatever its session.
    Agent,
    /// The agent session with this key.
    Session(String),
}

/// The transactions asked for, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionsAsked {
    /// The account's name as typed.
    pub account: String,
    /// Only the transactions of this asset, named as a recording names it.
    pub asset: Option<String>,
    /// Only the transactions from this date on.
    pub from: Option<String>,
    /// Only the transactions up to this date.
    pub to: Option<String>,
    /// Only the transactions of this kind.
    pub kind: Option<TransactionType>,
    /// Only the transactions this one recorded.
    pub recorded_by: Option<RecordedBy>,
    /// The page, from 1.
    pub page: usize,
}

/// One transaction of a listing.
#[derive(Debug, Clone)]
pub struct ListedTransaction {
    /// The transaction as a surface reads it.
    pub transaction: TransactionView,
    /// The reference of its asset; `None` for a transaction on the account's cash.
    pub asset_reference: Option<String>,
    /// The agent session that recorded it; `None` for one the owner typed (AGT-045).
    pub recorded_by: Option<AgentRecording>,
}

/// One page of the transactions of an account.
#[derive(Debug, Clone)]
pub struct ListedTransactions {
    /// The account's name as stored.
    pub account_name: String,
    /// The account's currency.
    pub currency: String,
    /// How many transactions the filter keeps, over every page.
    pub total: usize,
    /// The page given, from 1.
    pub page: usize,
    /// How many pages the filter's transactions make.
    pub pages: usize,
    /// The transactions of the page, oldest first.
    pub transactions: Vec<ListedTransaction>,
}

/// The price of an asset at a date, as read (AGT-071).
#[derive(Debug, Clone)]
pub struct PriceRead {
    /// The asset named.
    pub asset: Asset,
    /// The date the price was asked for.
    pub date: String,
    /// The price on that date or the closest earlier one; `None` when the asset has no
    /// price up to that date.
    pub price: Option<AssetPrice>,
}

/// One page of an asset's prices over a period (AGT-075).
#[derive(Debug, Clone)]
pub struct ListedPrices {
    /// The asset named.
    pub asset: Asset,
    /// How many prices the dates keep, over every page.
    pub total: usize,
    /// The page given, from 1.
    pub page: usize,
    /// How many pages the kept prices make.
    pub pages: usize,
    /// The prices of the page, oldest first.
    pub prices: Vec<AssetPrice>,
}

impl HoldingTransactionUseCase {
    /// AGT-070 — the transactions of the account named that the filter keeps, oldest first:
    /// the page asked for, with how many there are in all.
    pub async fn list_named(
        &self,
        asked: TransactionsAsked,
    ) -> Result<ListedTransactions, Refusal> {
        let from = bound(asked.from.as_deref(), "from")?;
        let to = bound(asked.to.as_deref(), "to")?;
        let account = self
            .find_account_by_name(&asked.account)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let asset_id = match &asked.asset {
            None => None,
            Some(typed) => Some(
                self.find_asset_by_name(typed)
                    .await
                    .map_err(|error| refusal_from_lookup(&error))?
                    .id,
            ),
        };
        let filter = JournalFilter {
            asset_id,
            transaction_type: asked.kind,
            ..JournalFilter::default()
        };
        let journal = self
            .account_service
            .get_account_journal(&account.id, &filter)
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "list_named: the journal could not be read");
                unreadable()
            })?;
        // Every asset but cash, archived ones too: a transaction on the account's cash has
        // no reference to give.
        let references: HashMap<String, String> = self
            .asset_service
            .get_non_cash_assets_by_name(true)
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "list_named: the assets could not be read");
                unreadable()
            })?
            .into_iter()
            .map(|asset| (asset.id, asset.reference))
            .collect();
        let kept: Vec<ListedTransaction> = journal
            .rows
            .into_iter()
            .filter(|row| {
                from.as_deref()
                    .is_none_or(|from| row.transaction.date.as_str() >= from)
                    && to
                        .as_deref()
                        .is_none_or(|to| row.transaction.date.as_str() <= to)
                    && match (&asked.recorded_by, &row.recorded_by) {
                        (None, _) => true,
                        (Some(RecordedBy::Owner), mark) => mark.is_none(),
                        (Some(RecordedBy::Agent), mark) => mark.is_some(),
                        (Some(RecordedBy::Session(key)), mark) => {
                            mark.as_ref().is_some_and(|mark| &mark.session == key)
                        }
                    }
            })
            .map(|row| ListedTransaction {
                asset_reference: references.get(&row.transaction.asset_id).cloned(),
                transaction: row.transaction,
                recorded_by: row.recorded_by,
            })
            .collect();
        let total = kept.len();
        let page = asked.page.max(1);
        Ok(ListedTransactions {
            account_name: account.name,
            currency: account.currency,
            total,
            page,
            pages: total.div_ceil(TRANSACTIONS_PER_PAGE),
            transactions: kept
                .into_iter()
                .skip((page - 1).saturating_mul(TRANSACTIONS_PER_PAGE))
                .take(TRANSACTIONS_PER_PAGE)
                .collect(),
        })
    }

    /// AGT-071 — the price the asset named has on `date`, today when none is given, or the
    /// closest earlier one. A date in the future is refused: no price is of it yet.
    pub async fn price_named(
        &self,
        asset: &str,
        date: Option<&str>,
        today: &str,
    ) -> Result<PriceRead, Refusal> {
        let date = bound(date, "date")?.unwrap_or_else(|| today.to_string());
        if date.as_str() > today {
            return Err(Refusal {
                code: "InvalidDate".to_string(),
                message: "date is in the future".to_string(),
            });
        }
        let asset = self
            .find_asset_by_name(asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let price = self
            .asset_service
            .price_as_of(&asset.id, &date)
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "price_named: the prices could not be read");
                unreadable()
            })?;
        Ok(PriceRead { asset, date, price })
    }

    /// AGT-075 — the prices of the asset named whose dates `from` and `to` keep, both
    /// included, oldest first: the page asked for, with how many there are in all.
    pub async fn prices_listed(
        &self,
        asset: &str,
        from: Option<&str>,
        to: Option<&str>,
        page: usize,
    ) -> Result<ListedPrices, Refusal> {
        let from = bound(from, "from")?;
        let to = bound(to, "to")?;
        let asset = self
            .find_asset_by_name(asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let mut kept: Vec<AssetPrice> = self
            .asset_service
            .get_asset_prices(&asset.id)
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "prices_listed: the prices could not be read");
                unreadable()
            })?
            .into_iter()
            .filter(|price| {
                from.as_deref().is_none_or(|from| price.date.as_str() >= from)
                    && to.as_deref().is_none_or(|to| price.date.as_str() <= to)
            })
            .collect();
        kept.sort_by(|a, b| a.date.cmp(&b.date));
        let total = kept.len();
        let page = page.max(1);
        Ok(ListedPrices {
            asset,
            total,
            page,
            pages: total.div_ceil(PRICES_PER_PAGE),
            prices: kept
                .into_iter()
                .skip((page - 1).saturating_mul(PRICES_PER_PAGE))
                .take(PRICES_PER_PAGE)
                .collect(),
        })
    }
}

/// A bound of the dates listed, as the journal's dates are written; one that is no date is
/// refused, naming the argument.
fn bound(typed: Option<&str>, name: &str) -> Result<Option<String>, Refusal> {
    let Some(typed) = typed else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(typed.trim(), "%Y-%m-%d")
        .map(|date| Some(date.format("%Y-%m-%d").to_string()))
        .map_err(|_| Refusal {
            code: "InvalidDate".to_string(),
            message: format!("{name} is not a date written YYYY-MM-DD"),
        })
}

fn unreadable() -> Refusal {
    Refusal {
        code: "DatabaseError".to_string(),
        message: "the portfolio could not be read".to_string(),
    }
}
