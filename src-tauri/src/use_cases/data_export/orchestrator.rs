//! Data export orchestrator (EXP): reads the portfolio, builds the six tables and writes
//! them as CSV or JSON in the chosen folder, complete or not at all.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::error::DataExportError;
use super::format::{
    account_file_names, csv, fills, format_file, free_name, import_mapper_file, json, money,
    quantity, Cell, Table,
};
use crate::context::account::{
    Account, AccountServiceContract, RemovalLot, Transaction, TransactionType,
};
use crate::context::asset::{Asset, AssetPrice, AssetServiceContract};
use crate::context::currency::{CurrencyRate, CurrencyService};
use crate::core::cash::is_cash_asset;
use crate::core::logger::BACKEND;

/// EXP-010 — the export's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ExportFormat {
    /// A folder of CSV files (EXP-011).
    Csv,
    /// One JSON file (EXP-012).
    Json,
}

/// EXP-016 — where the export went and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ExportReport {
    /// The folder or the file written.
    pub written_to: String,
    /// How many accounts it holds.
    pub accounts: u32,
    /// How many assets it holds.
    pub assets: u32,
    /// How many transactions it holds.
    pub transactions: u32,
    /// How many prices it holds.
    pub prices: u32,
}

const ACCOUNT_COLUMNS: &[&str] = &[
    "id",
    "name",
    "bank",
    "currency",
    "country",
    "wrapper",
    "holder",
    "management_fees_enabled",
    "cost_basis_method",
];
const ASSET_COLUMNS: &[&str] = &[
    "id",
    "name",
    "reference",
    "isin",
    "kind",
    "class",
    "currency",
    "category",
    "risk_level",
    "exchange",
    "interest_bearing",
    "price_typed_by_hand",
    "archived",
];
const TRANSACTION_COLUMNS: &[&str] = &[
    "id",
    "account_id",
    "account_name",
    "account_currency",
    "date",
    "type",
    "asset_id",
    "asset_name",
    "asset_reference",
    "asset_isin",
    "quantity",
    "unit_price",
    "exchange_rate",
    "fees",
    "total_amount",
    "realized_pnl",
    "split_factor",
    "dividend_amount",
    "dividend_currency",
    "write_off_reason",
    "transfer_id",
    "added_deposit",
    "note",
    "entered_at",
];
const PRICE_COLUMNS: &[&str] = &[
    "asset_id",
    "asset_reference",
    "date",
    "price",
    "currency",
    "source",
];

/// EXP-026 — the lots' columns.
const LOT_COLUMNS: &[&str] = &[
    "transaction_id",
    "account_id",
    "asset_id",
    "acquired_on",
    "quantity_taken",
    "cost_taken",
    "free",
];

/// EXP-027 — the exchange rates' columns.
const RATE_COLUMNS: &[&str] = &["from_currency", "to_currency", "date", "rate", "source"];

/// The moment a transaction entered before that moment was kept holds (TRX-036).
const UNKNOWN_MOMENT: &str = "1970-01-01T00:00:00Z";
/// A rate of one, in micro-units: between two currencies, a dividend entered in the account's
/// currency (DIV-029, DIV-043).
const RATE_ONE: i64 = 1_000_000;

/// What was read, the links between its parts already held (EXP-022).
struct Snapshot {
    accounts: Vec<Account>,
    assets: Vec<Asset>,
    transactions: Vec<Transaction>,
    prices: Vec<AssetPrice>,
    /// EXP-027 — every recorded exchange rate.
    rates: Vec<CurrencyRate>,
    /// EXP-026 — the lots each removal took, with its account's identifier.
    lots: Vec<(String, RemovalLot)>,
}

/// EXP — reads every account, asset, transaction, price and currency rate, and writes them.
pub struct DataExportUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl DataExportUseCase {
    /// The use case over the account, asset and currency services.
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

    /// EXP-010 to EXP-016 — writes the export into `folder` and says where and how much.
    pub async fn export(
        &self,
        format: ExportFormat,
        folder: &Path,
    ) -> Result<ExportReport, DataExportError> {
        if !tokio::fs::metadata(folder)
            .await
            .is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(DataExportError::FolderUnavailable);
        }
        let snapshot = self.read().await?;
        let today = chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let exported_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let tables = tables(&snapshot);
        let stem = format!("folioneer-export-{today}");
        let written_to = match format {
            ExportFormat::Csv => {
                let files = csv_files(&tables, &snapshot, &exported_at, &today)?;
                write_folder(folder, &stem, files).await?
            }
            ExportFormat::Json => {
                write_file(folder, &stem, json(&tables, &exported_at).into_bytes()).await?
            }
        };
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Ok(ExportReport {
            written_to: written_to.to_string_lossy().into_owned(),
            accounts: count(snapshot.accounts.len()),
            assets: count(snapshot.assets.len()),
            transactions: count(snapshot.transactions.len()),
            prices: count(snapshot.prices.len()),
        })
    }

    /// EXP-020 / EXP-022 — the accounts and their transactions, then the assets and their
    /// prices; a transaction whose asset was not read, and a transfer recorded while the
    /// export reads with a side missing, is left out, so every link the export holds is
    /// whole. The reads are not one database transaction: the links hold through these
    /// filters.
    async fn read(&self) -> Result<Snapshot, DataExportError> {
        let started = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.6fZ")
            .to_string();
        let database = |e: &dyn std::fmt::Debug, what: &str| {
            tracing::error!(target: BACKEND, err = ?e, what, "data export: read failed");
            DataExportError::DatabaseError
        };
        let accounts = self
            .account_service
            .get_all()
            .await
            .map_err(|e| database(&e, "accounts"))?;
        let mut transactions = Vec::new();
        let mut lots = Vec::new();
        for account in &accounts {
            let own = self
                .account_service
                .get_all_transactions_for_account(&account.id)
                .await
                .map_err(|e| database(&e, "transactions"))?;
            // EXP-026 — the lots each removal took, replayed over the account's whole history.
            lots.extend(
                Account::removal_lots_in(&own, account.cost_basis_method)
                    .into_iter()
                    .map(|lot| (account.id.clone(), lot)),
            );
            transactions.extend(own);
        }
        let assets = self
            .asset_service
            .get_non_cash_assets()
            .await
            .map_err(|e| database(&e, "assets"))?;
        let mut prices = Vec::new();
        for asset in &assets {
            prices.extend(
                self.asset_service
                    .get_asset_prices(&asset.id)
                    .await
                    .map_err(|e| database(&e, "prices"))?,
            );
        }
        let mut rates = Vec::new();
        let pairs = self
            .currency_service
            .list_currency_pairs()
            .await
            .map_err(|e| database(&e, "currency pairs"))?;
        for pair in pairs {
            rates.extend(
                self.currency_service
                    .list_currency_rates(pair.from_currency, pair.to_currency)
                    .await
                    .map_err(|e| database(&e, "rates"))?,
            );
        }
        let transactions = whole_links(transactions, &assets, &started);
        // EXP-022 — only the lots of a removal the export holds.
        let kept: HashSet<&str> = transactions.iter().map(|t| t.id.as_str()).collect();
        let lots: Vec<(String, RemovalLot)> = lots
            .into_iter()
            .filter(|(_, lot)| kept.contains(lot.transaction_id.as_str()))
            .collect();
        Ok(Snapshot {
            accounts,
            assets,
            transactions,
            prices,
            rates,
            lots,
        })
    }
}

/// EXP-022 — the transactions whose links are whole: on the cash or on an asset read, and
/// a transfer with both its sides — its fee in kind with them — or with the one side its
/// deleted account left (TRF-041), recorded before the export began reading (`started`);
/// a side alone that was recorded since is a transfer being written, and is left out.
fn whole_links(
    transactions: Vec<Transaction>,
    assets: &[Asset],
    started: &str,
) -> Vec<Transaction> {
    let asset_ids: HashSet<&str> = assets.iter().map(|asset| asset.id.as_str()).collect();
    // Per transfer: its out side, its in side, and whether every part predates the read.
    let mut sides: HashMap<String, (bool, bool, bool)> = HashMap::new();
    for transaction in &transactions {
        if let Some(transfer_id) = &transaction.transfer_id {
            let side = sides
                .entry(transfer_id.clone())
                .or_insert((false, false, true));
            match transaction.transaction_type {
                TransactionType::TransferOut => side.0 = true,
                TransactionType::TransferIn => side.1 = true,
                _ => {}
            }
            side.2 &= transaction.created_at.as_str() < started;
        }
    }
    transactions
        .into_iter()
        .filter(|transaction| {
            is_cash_asset(&transaction.asset_id)
                || asset_ids.contains(transaction.asset_id.as_str())
        })
        .filter(|transaction| {
            transaction.transfer_id.as_ref().is_none_or(|transfer_id| {
                sides
                    .get(transfer_id)
                    .is_some_and(|&(out, into, before)| (out && into) || before)
            })
        })
        .collect()
}

fn text(value: impl Into<String>) -> Cell {
    Cell::Text(Some(value.into()))
}

fn optional_text(value: Option<impl Into<String>>) -> Cell {
    Cell::Text(value.map(Into::into))
}

fn figure(value: Option<String>) -> Cell {
    Cell::Figure(value)
}

/// EXP-031 / EXP-033 — the four tables, each in its order.
fn tables(snapshot: &Snapshot) -> Vec<Table> {
    let mut accounts: Vec<&Account> = snapshot.accounts.iter().collect();
    accounts.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    let mut assets: Vec<&Asset> = snapshot.assets.iter().collect();
    assets.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    let ordered = ordered_transactions(snapshot);
    vec![
        Table {
            name: "accounts",
            columns: ACCOUNT_COLUMNS,
            rows: accounts
                .iter()
                .map(|account| account_row(account))
                .collect(),
        },
        Table {
            name: "assets",
            columns: ASSET_COLUMNS,
            rows: assets.iter().map(|asset| asset_row(asset)).collect(),
        },
        Table {
            name: "transactions",
            columns: TRANSACTION_COLUMNS,
            rows: {
                let assets: HashMap<&str, &Asset> = snapshot
                    .assets
                    .iter()
                    .map(|asset| (asset.id.as_str(), asset))
                    .collect();
                ordered
                    .iter()
                    .map(|(account, transaction)| {
                        transaction_row(
                            account,
                            transaction,
                            assets.get(transaction.asset_id.as_str()).copied(),
                        )
                    })
                    .collect()
            },
        },
        Table {
            name: "prices",
            columns: PRICE_COLUMNS,
            rows: ordered_prices(snapshot)
                .into_iter()
                .map(|(asset, price)| price_row(asset, price))
                .collect(),
        },
        Table {
            name: "lots",
            columns: LOT_COLUMNS,
            rows: {
                // EXP-033 — in the order of their transactions, then oldest lot first.
                let order: HashMap<&str, usize> = ordered
                    .iter()
                    .enumerate()
                    .map(|(index, (_, transaction))| (transaction.id.as_str(), index))
                    .collect();
                let mut lots: Vec<&(String, RemovalLot)> = snapshot.lots.iter().collect();
                lots.sort_by_key(|(_, lot)| {
                    order
                        .get(lot.transaction_id.as_str())
                        .copied()
                        .unwrap_or(usize::MAX)
                });
                lots.into_iter()
                    .map(|(account_id, lot)| lot_row(account_id, lot))
                    .collect()
            },
        },
        Table {
            name: "rates",
            columns: RATE_COLUMNS,
            rows: {
                // EXP-033 — by their two currencies, then date.
                let mut rates: Vec<&CurrencyRate> = snapshot.rates.iter().collect();
                rates.sort_by(|a, b| {
                    (&a.from_currency, &a.to_currency, &a.date).cmp(&(
                        &b.from_currency,
                        &b.to_currency,
                        &b.date,
                    ))
                });
                rates.into_iter().map(rate_row).collect()
            },
        },
    ]
}

/// EXP-026 — one lot a removal took.
fn lot_row(account_id: &str, lot: &RemovalLot) -> Vec<Cell> {
    vec![
        text(&lot.transaction_id),
        text(account_id),
        text(&lot.asset_id),
        text(&lot.acquired_on),
        figure(Some(quantity(&lot.asset_id, lot.quantity))),
        figure(Some(money(lot.cost))),
        Cell::Flag(lot.free),
    ]
}

/// EXP-027 — one recorded exchange rate.
fn rate_row(rate: &CurrencyRate) -> Vec<Cell> {
    vec![
        text(&rate.from_currency),
        text(&rate.to_currency),
        text(&rate.date),
        figure(Some(money(rate.rate))),
        text(rate.source.to_string()),
    ]
}

fn account_row(account: &Account) -> Vec<Cell> {
    vec![
        text(&account.id),
        text(&account.name),
        text(&account.bank_name),
        text(&account.currency),
        optional_text(account.country.as_deref()),
        optional_text(account.wrapper.as_deref()),
        optional_text(account.holder.as_deref()),
        Cell::Flag(account.management_fees_enabled),
        text(account.cost_basis_method.as_str()),
    ]
}

fn asset_row(asset: &Asset) -> Vec<Cell> {
    vec![
        text(&asset.id),
        text(&asset.name),
        text(&asset.reference),
        optional_text(asset.isin.as_deref()),
        text(asset.kind.to_string()),
        text(asset.class.to_string()),
        text(&asset.currency),
        text(&asset.category.name),
        Cell::Number(i64::from(asset.risk_level)),
        optional_text(
            asset
                .exchange
                .as_ref()
                .map(|exchange| exchange.code.as_str()),
        ),
        Cell::Flag(asset.interest_bearing),
        Cell::Flag(asset.price_refresh_blocked),
        Cell::Flag(asset.is_archived),
    ]
}

/// EXP-033 — transactions by account name, account, date, the moment entered (an unknown
/// one first), then identifier: the replay's order (TRX-036).
fn ordered_transactions(snapshot: &Snapshot) -> Vec<(&Account, &Transaction)> {
    let accounts: HashMap<&str, &Account> = snapshot
        .accounts
        .iter()
        .map(|account| (account.id.as_str(), account))
        .collect();
    let mut rows: Vec<(&Account, &Transaction)> = snapshot
        .transactions
        .iter()
        .filter_map(|transaction| {
            accounts
                .get(transaction.account_id.as_str())
                .map(|account| (*account, transaction))
        })
        .collect();
    rows.sort_by(|(a, x), (b, y)| {
        (&a.name, &a.id, &x.date, &x.created_at, &x.id).cmp(&(
            &b.name,
            &b.id,
            &y.date,
            &y.created_at,
            &y.id,
        ))
    });
    rows
}

/// EXP-033 — prices by asset reference, asset, then date.
fn ordered_prices(snapshot: &Snapshot) -> Vec<(&Asset, &AssetPrice)> {
    let assets: HashMap<&str, &Asset> = snapshot
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect();
    let mut rows: Vec<(&Asset, &AssetPrice)> = snapshot
        .prices
        .iter()
        .filter_map(|price| {
            assets
                .get(price.asset_id.as_str())
                .map(|asset| (*asset, price))
        })
        .collect();
    rows.sort_by(|(a, x), (b, y)| {
        (&a.reference, &a.id, &x.date).cmp(&(&b.reference, &b.id, &y.date))
    });
    rows
}

/// EXP-031 — one transaction, each type filling its own figures (`fills`), the others empty.
fn transaction_row(
    account: &Account,
    transaction: &Transaction,
    asset: Option<&Asset>,
) -> Vec<Cell> {
    use TransactionType::*;
    let kind = transaction.transaction_type;
    let filled = fills(kind);
    let on_cash = is_cash_asset(&transaction.asset_id);
    let fill = |fills: bool, value: i64| -> Option<String> { fills.then(|| money(value)) };
    let is_transfer = matches!(kind, TransferOut | TransferIn);
    let quantity_text = match kind {
        Split | Dividend => "0".to_string(),
        _ => quantity(&transaction.asset_id, transaction.quantity),
    };
    let dividend_currency = (kind == Dividend).then(|| {
        let asset_currency =
            asset.map_or(account.currency.as_str(), |asset| asset.currency.as_str());
        if asset_currency != account.currency && transaction.exchange_rate == RATE_ONE {
            account.currency.clone()
        } else {
            asset_currency.to_string()
        }
    });
    let entered_at =
        (transaction.created_at != UNKNOWN_MOMENT).then(|| transaction.created_at.clone());
    vec![
        text(&transaction.id),
        text(&account.id),
        text(&account.name),
        text(&account.currency),
        text(&transaction.date),
        text(kind.to_string()),
        Cell::Text((!on_cash).then(|| transaction.asset_id.clone())),
        Cell::Text(asset.map(|asset| asset.name.clone())),
        Cell::Text(asset.map(|asset| asset.reference.clone())),
        Cell::Text(asset.and_then(|asset| asset.isin.clone())),
        figure(Some(quantity_text)),
        figure(fill(filled.unit_price, transaction.unit_price)),
        figure(fill(filled.exchange_rate, transaction.exchange_rate)),
        figure(fill(filled.fees, transaction.fees)),
        figure(fill(filled.total_amount, transaction.total_amount)),
        figure(
            transaction
                .realized_pnl
                .filter(|_| filled.realized_pnl)
                .map(money),
        ),
        figure(
            transaction
                .split_factor
                .filter(|_| kind == Split)
                .map(money),
        ),
        figure(
            transaction
                .dividend_amount
                .filter(|_| kind == Dividend)
                .map(money),
        ),
        Cell::Text(dividend_currency),
        Cell::Text(
            transaction
                .write_off_reason
                .map(|reason| reason.to_string()),
        ),
        Cell::Text(
            transaction
                .transfer_id
                .clone()
                .filter(|_| is_transfer || kind == WriteOff),
        ),
        Cell::Flag(transaction.added_deposit),
        Cell::Text(transaction.note.clone()),
        Cell::Text(entered_at),
    ]
}

fn price_row(asset: &Asset, price: &AssetPrice) -> Vec<Cell> {
    vec![
        text(&asset.id),
        text(&asset.reference),
        text(&price.date),
        figure(Some(money(price.price))),
        text(&asset.currency),
        text(price.source.to_string()),
    ]
}

/// EXP-011 — the files of the CSV folder, by name.
fn csv_files(
    tables: &[Table],
    snapshot: &Snapshot,
    exported_at: &str,
    today: &str,
) -> Result<Vec<(String, Vec<u8>)>, DataExportError> {
    let mut files: Vec<(String, Vec<u8>)> = tables
        .iter()
        .map(|table| (format!("{}.csv", table.name), csv(table).into_bytes()))
        .collect();
    files.push(("format.json".into(), format_file(exported_at).into_bytes()));
    let names = account_file_names(
        &snapshot
            .accounts
            .iter()
            .map(|account| (account.id.clone(), account.name.clone()))
            .collect::<Vec<_>>(),
    );
    if let Some(transactions) = tables.iter().find(|table| table.name == "transactions") {
        for account in &snapshot.accounts {
            let rows: Vec<Vec<Cell>> = transactions
                .rows
                .iter()
                .filter(|row| matches!(row.get(1), Some(Cell::Text(Some(id))) if *id == account.id))
                .cloned()
                .collect();
            if rows.is_empty() {
                continue;
            }
            let Some(name) = names.get(&account.id) else {
                continue;
            };
            let table = Table {
                name: "transactions",
                columns: TRANSACTION_COLUMNS,
                rows,
            };
            files.push((name.clone(), csv(&table).into_bytes()));
        }
    }
    let mapper = import_mapper_file(today).map_err(|e| {
        tracing::error!(target: BACKEND, err = ?e, "data export: mapper not written");
        DataExportError::WriteFailed
    })?;
    files.push(("import-mapper.json".into(), mapper));
    Ok(files)
}

fn write_failed(e: std::io::Error) -> DataExportError {
    tracing::error!(target: BACKEND, err = ?e, "data export: write failed");
    DataExportError::WriteFailed
}

/// EXP-014 — the names the chosen folder holds now, read without blocking.
async fn names_in(folder: &Path) -> std::io::Result<HashSet<String>> {
    let mut names = HashSet::new();
    let mut entries = tokio::fs::read_dir(folder).await?;
    while let Some(entry) = entries.next_entry().await? {
        names.insert(entry.file_name().to_string_lossy().into_owned());
    }
    Ok(names)
}

/// A temporary name in `folder` no other export can take: hidden, and unique.
fn partial_in(folder: &Path, stem: &str) -> PathBuf {
    folder.join(format!(".{stem}.partial-{}", uuid::Uuid::new_v4()))
}

/// EXP-014 / EXP-015 — gives what was written at `partial` the first free name, never over
/// what is there: a file is linked to its name, which refuses one that exists; a folder is
/// renamed once the name is checked free again. Tried until a name is taken.
async fn place(
    folder: &Path,
    stem: &str,
    extension: Option<&str>,
    partial: &Path,
) -> std::io::Result<PathBuf> {
    for _ in 0..100 {
        let taken = names_in(folder).await?;
        let name = free_name(stem, extension, |name| taken.contains(name));
        let target = folder.join(&name);
        if extension.is_some() {
            match tokio::fs::hard_link(partial, &target).await {
                Ok(()) => {
                    let _ = tokio::fs::remove_file(partial).await;
                    return Ok(target);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                // A drive without links (a FAT USB key): renamed once checked free.
                Err(_) => {}
            }
        }
        if tokio::fs::symlink_metadata(&target).await.is_ok() {
            continue;
        }
        tokio::fs::rename(partial, &target).await?;
        return Ok(target);
    }
    Err(std::io::Error::other("no free name"))
}

/// Writes `bytes` to a new file at `path`, refusing one that exists and never following a
/// link, then flushes it to the disk.
async fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(bytes).await?;
    file.sync_all().await
}

/// EXP-015 — the folder is written under a hidden temporary name, then named; a failure
/// removes what this export wrote, and only that.
async fn write_folder(
    folder: &Path,
    stem: &str,
    files: Vec<(String, Vec<u8>)>,
) -> Result<PathBuf, DataExportError> {
    let partial = partial_in(folder, stem);
    tokio::fs::create_dir(&partial)
        .await
        .map_err(write_failed)?;
    let written = async {
        for (file, bytes) in &files {
            write_new(&partial.join(file), bytes).await?;
        }
        place(folder, stem, None, &partial).await
    }
    .await;
    match written {
        Ok(path) => Ok(path),
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&partial).await;
            Err(write_failed(e))
        }
    }
}

/// EXP-015 — the file is written under a hidden temporary name, then named; a failure
/// removes what this export wrote, and only that.
async fn write_file(folder: &Path, stem: &str, bytes: Vec<u8>) -> Result<PathBuf, DataExportError> {
    let partial = partial_in(folder, stem);
    let written = async {
        write_new(&partial, &bytes).await?;
        place(folder, stem, Some("json"), &partial).await
    }
    .await;
    match written {
        Ok(path) => Ok(path),
        Err(e) => {
            let _ = tokio::fs::remove_file(&partial).await;
            Err(write_failed(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EXP-015 — an account that cannot be read refuses the export with the database error,
    // and nothing is written.
    #[tokio::test]
    async fn an_unreadable_portfolio_is_refused_and_nothing_written() {
        use crate::context::account::{AccountError, MockAccountServiceContract};
        use crate::context::asset::MockAssetServiceContract;
        let mut accounts = MockAccountServiceContract::new();
        accounts
            .expect_get_all()
            .returning(|| Err(AccountError::DatabaseError));
        let export = DataExportUseCase::new(
            Arc::new(accounts),
            Arc::new(MockAssetServiceContract::new()),
            Arc::new(crate::context::currency::CurrencyService::new(
                Box::new(crate::context::currency::MockCurrencyPairRepository::new()),
                Box::new(crate::context::currency::MockCurrencyRateRepository::new()),
            )),
        );
        let out = tempfile::tempdir().expect("folder");
        let refused = export.export(ExportFormat::Json, out.path()).await;
        assert!(matches!(refused, Err(DataExportError::DatabaseError)));
        assert_eq!(std::fs::read_dir(out.path()).expect("listed").count(), 0);
    }
    use crate::context::account::WriteOffReason;
    use crate::shared::domain::quantity::from_surface;

    fn side(kind: TransactionType, transfer_id: &str) -> Transaction {
        Transaction::new_transfer_side(
            "acc".into(),
            "asset".into(),
            kind,
            "2024-01-01".into(),
            from_surface(1_000_000),
            1_000_000,
            1_000_000,
            None,
            transfer_id.into(),
        )
        .expect("side")
    }

    fn asset() -> Asset {
        Asset::restore(
            "asset".into(),
            crate::context::asset::AssetKind::Listed,
            "Acme".into(),
            crate::context::asset::AssetClass::Stocks,
            crate::context::asset::AssetCategory::default(),
            "EUR".into(),
            3,
            "ACM".into(),
            None,
            false,
            None,
            false,
            false,
        )
    }

    // EXP-022 — a transfer is kept with both sides and its fee in kind; one being written,
    // a side missing, is left out; a transaction on an asset not read is left out; the cash
    // is always kept.
    #[test]
    fn a_transfer_and_its_fee_are_whole_or_absent() {
        let mut fee = Transaction::write_off(
            "acc".into(),
            "asset".into(),
            "2024-01-01".into(),
            from_surface(1_000_000),
            WriteOffReason::NetworkFee,
            None,
        )
        .expect("write-off")
        .with_transfer_id(Some("t1".into()));
        fee.total_amount = 0;
        let kept = whole_links(
            vec![
                side(TransactionType::TransferOut, "t1"),
                side(TransactionType::TransferIn, "t1"),
                fee,
                side(TransactionType::TransferOut, "t2"),
                Transaction::new_deposit(
                    "acc".into(),
                    "system-cash-eur".into(),
                    "2024-01-01".into(),
                    1_000_000,
                    None,
                )
                .expect("deposit"),
            ],
            &[asset()],
            "2000-01-01T00:00:00.000000Z",
        );
        let ids: Vec<Option<&str>> = kept.iter().map(|t| t.transfer_id.as_deref()).collect();
        assert_eq!(ids, [Some("t1"), Some("t1"), Some("t1"), None]);
        let later = "9999-01-01T00:00:00.000000Z";
        assert!(whole_links(vec![side(TransactionType::TransferOut, "t3")], &[], later).is_empty());
        // TRF-041 — the side a deleted account left, recorded before the read, stays.
        let orphan = whole_links(
            vec![side(TransactionType::TransferOut, "t4")],
            &[asset()],
            later,
        );
        assert_eq!(orphan.len(), 1);
    }

    // EXP-023 / EXP-033 — a transaction entered before its moment was kept sorts first on its
    // day, as the replay does, and its moment is written empty.
    #[test]
    fn an_unknown_moment_sorts_first_and_is_written_empty() {
        let mut account =
            Account::new("Main".into(), String::new(), "EUR".into(), false).expect("account");
        account.id = "acc".into();
        let deposit = |amount: i64| {
            Transaction::new_deposit(
                "acc".into(),
                "system-cash-eur".into(),
                "2024-01-01".into(),
                amount,
                None,
            )
            .expect("deposit")
        };
        let recent = deposit(2_000_000);
        let mut older = deposit(1_000_000);
        older.created_at = UNKNOWN_MOMENT.into();
        let snapshot = Snapshot {
            accounts: vec![account],
            assets: Vec::new(),
            transactions: vec![recent, older],
            prices: Vec::new(),
            rates: Vec::new(),
            lots: Vec::new(),
        };
        let table = &tables(&snapshot)[2];
        let total = |row: &Vec<Cell>| row[14].clone();
        assert_eq!(total(&table.rows[0]), Cell::Figure(Some("1".into())));
        assert_eq!(table.rows[0][23], Cell::Text(None));
        assert_eq!(total(&table.rows[1]), Cell::Figure(Some("2".into())));
    }
}
