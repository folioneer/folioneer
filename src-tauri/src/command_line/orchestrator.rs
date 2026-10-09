//! Running a command (CLI-011, CLI-012): the account and asset found by what the user
//! typed, then recorded through the same use case the window calls.

use std::sync::Arc;

use serde::Serialize;

use crate::context::account::{AccountServiceContract, Transaction, TransactionType};
use crate::context::asset::{
    Asset, AssetClass, AssetError, AssetKind, AssetServiceContract, NamedAsset,
};
use crate::core::BACKEND;
pub use crate::use_cases::holding_transaction::Refusal;
use crate::use_cases::holding_transaction::{code_of, HoldingTransactionUseCase, Recording};

use super::args::Listed;

/// What a command did.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// It recorded this transaction, in this account and currency, on this asset; an
    /// opening balance of zero cost is said so (TRX-065).
    Recorded {
        transaction: Box<Transaction>,
        account_name: String,
        currency: String,
        asset_reference: String,
        zero_cost: bool,
    },
    /// It added this asset; another asset has the same reference when `reference_shared`
    /// (CLI-026).
    AssetAdded {
        asset: AssetRow,
        reference_shared: bool,
    },
    /// It listed these accounts or assets, sorted by name.
    Listed(Listing),
    /// It recorded nothing.
    Refused(Refusal),
}

/// What a list command found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    /// CLI-018 — the accounts.
    Accounts(Vec<AccountRow>),
    /// CLI-019 — the assets a command can name.
    Assets(Vec<AssetRow>),
}

/// An account as `--account` names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountRow {
    /// The account's name.
    pub name: String,
    /// The account's currency.
    pub currency: String,
}

/// An asset as `--asset` names it, with what the text output leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRow {
    /// The asset's name.
    pub name: String,
    /// The asset's reference.
    pub reference: String,
    /// The asset's kind.
    pub kind: AssetKind,
    /// The asset's class.
    pub class: AssetClass,
    /// The asset's currency.
    pub currency: String,
    /// The asset's ISIN, when it has one.
    pub isin: Option<String>,
    /// The code of the asset's exchange, when it has one: what follows `@` in `--asset`.
    pub exchange: Option<String>,
    /// Whether the asset is archived.
    pub archived: bool,
}

/// CLI-011/012 — asks the holding use case for what a command names, then records through it:
/// the same use case the window calls. An interface adapter, like the Tauri commands.
pub struct CommandRunner {
    holding: HoldingTransactionUseCase,
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
}

impl CommandRunner {
    /// Wires the runner over the account and asset services.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            holding: HoldingTransactionUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
            ),
            account_service,
            asset_service,
        }
    }

    /// CLI-026 — adds an asset through the core, which decides what the command left out.
    pub async fn add_asset(&self, named: NamedAsset) -> Outcome {
        match self.asset_service.add_named_asset(named).await {
            Ok(added) => Outcome::AssetAdded {
                asset: row_of(added.asset),
                reference_shared: added.reference_shared,
            },
            Err(AssetError::CategoryNameNotFound { name }) => Outcome::Refused(Refusal {
                code: "CategoryNotFound".to_string(),
                message: format!("no category named \"{}\"", name.escape_debug()),
            }),
            Err(error) => {
                let code = code_of(&error);
                Outcome::Refused(Refusal {
                    message: format!("refused by the rules ({code})"),
                    code,
                })
            }
        }
    }

    /// CLI-018 / CLI-019 — what a list command asks for, in the core's order.
    pub async fn list(&self, listed: Listed) -> Outcome {
        match listed {
            Listed::Accounts => match self.account_service.get_all_by_name().await {
                Ok(accounts) => Outcome::Listed(Listing::Accounts(
                    accounts
                        .into_iter()
                        .map(|account| AccountRow {
                            name: account.name,
                            currency: account.currency,
                        })
                        .collect(),
                )),
                Err(error) => {
                    tracing::error!(target: BACKEND, err = ?error, "command line: account list failed");
                    Outcome::Refused(unreadable())
                }
            },
            Listed::Assets { archived } => {
                match self
                    .asset_service
                    .get_non_cash_assets_by_name(archived)
                    .await
                {
                    Ok(assets) => {
                        Outcome::Listed(Listing::Assets(assets.into_iter().map(row_of).collect()))
                    }
                    Err(error) => {
                        tracing::error!(target: BACKEND, err = ?error, "command line: asset list failed");
                        Outcome::Refused(unreadable())
                    }
                }
            }
        }
    }

    /// Records a command through the core's recorder; `today` is the date used when the
    /// command gives none.
    pub async fn run(&self, command: Recording, today: &str) -> Outcome {
        match self.holding.record_named(command, today).await {
            Ok(recorded) => Outcome::Recorded {
                transaction: recorded.transaction,
                account_name: recorded.account_name,
                currency: recorded.currency,
                asset_reference: recorded.asset_reference,
                zero_cost: recorded.zero_cost,
            },
            Err(refusal) => Outcome::Refused(refusal),
        }
    }
}

/// An asset as a list or an addition reports it.
fn row_of(asset: Asset) -> AssetRow {
    AssetRow {
        name: asset.name,
        reference: asset.reference,
        kind: asset.kind,
        class: asset.class,
        currency: asset.currency,
        isin: asset.isin,
        exchange: asset.exchange.map(|exchange| exchange.code),
        archived: asset.is_archived,
    }
}

/// The portfolio could not be read.
fn unreadable() -> Refusal {
    Refusal {
        code: "DatabaseError".to_string(),
        message: "the portfolio could not be read".to_string(),
    }
}

/// Which way a recorded transaction reads in a sentence.
pub fn verb(transaction_type: TransactionType) -> &'static str {
    match transaction_type {
        TransactionType::OpeningBalance => "opened",
        TransactionType::Purchase => "bought",
        TransactionType::Sell => "sold",
        _ => "recorded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CLI-020 — a recorded transaction reads with the verb of its type.
    #[test]
    fn cli_020_each_type_reads_with_its_verb() {
        assert_eq!(verb(TransactionType::OpeningBalance), "opened");
        assert_eq!(verb(TransactionType::Purchase), "bought");
        assert_eq!(verb(TransactionType::Sell), "sold");
        assert_eq!(verb(TransactionType::Dividend), "recorded");
    }
}
