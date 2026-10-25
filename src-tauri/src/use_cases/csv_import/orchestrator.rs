use super::error::{CsvImportError, CsvImportTask};
use crate::context::account::{
    AccountError, AccountServiceContract, ImportJudgment, ImportStep, RecordingOp, TradeOp,
};
use crate::context::asset::{Asset, AssetServiceContract};
use crate::context::import::{
    folded, read_file, read_rows, AssetMatch, AssetRead, ColumnMappingDraft, FileRead, ImportError,
    ImportService, OperationRead, ReadingChoices, RowFigures, RowFormat, RowRead, RowRejection,
    LARGEST_FILE,
};
use crate::core::logger::BACKEND;
use serde::Serialize;
use specta::Type;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// An exchange rate of one (micro-units): one currency in this version (IMP-037).
const SAME_CURRENCY: i64 = 1_000_000;

/// Why a row is rejected (IMP-030).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "code")]
pub enum RowReason {
    /// It does not read through the mapping in use (IMP-016, IMP-022, IMP-030, IMP-031,
    /// IMP-034).
    Unread {
        /// What does not read.
        rejection: RowRejection,
    },
    /// Its asset's currency, or its own, is not the account's (IMP-037).
    OtherCurrency,
    /// A dividend of an asset the account does not hold (DIV-011).
    AssetNotHeld,
    /// Manual entry refuses it, with the reason it gives (IMP-035).
    ManualEntry {
        /// The refusal manual entry gives.
        refusal: AccountError,
    },
}

/// A row's status in the preview (IMP-030).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "status")]
pub enum RowStatus {
    /// Recorded on confirmation (#502); `funding` is the deposit a purchase the cash cannot
    /// fund needs (IMP-038).
    Ready {
        /// The operation read.
        operation: OperationRead,
        /// The asset matched; `None` for a deposit and a withdrawal.
        asset_id: Option<String>,
        /// The deposit the purchase needs, in the account's currency (micro-units).
        funding: Option<i64>,
    },
    /// Not recorded, with its reason.
    Rejected {
        /// Why.
        reason: RowReason,
    },
    /// No asset of the portfolio matches it (IMP-032).
    UnknownAsset {
        /// The asset as read.
        asset: AssetRead,
    },
    /// The account holds it already (IMP-036).
    AlreadyRecorded {
        /// The operation read.
        operation: OperationRead,
        /// The asset matched; `None` for a deposit and a withdrawal.
        asset_id: Option<String>,
    },
    /// A label the mapping leaves to record by hand (IMP-022).
    NotImported {
        /// The label read.
        label: String,
    },
    /// A trade tax added to the purchase on line `purchase_line`, of that purchase's status
    /// (IMP-034).
    JoinedTradeTax {
        /// The purchase's line number.
        purchase_line: u32,
    },
}

/// A row of the file in the preview.
#[derive(Debug, Clone, Serialize, Type)]
pub struct PreviewRow {
    /// Its line number in the file, from 1 for the header.
    pub line: u32,
    /// Its status.
    pub status: RowStatus,
}

/// How many rows hold each status; a joined trade tax counts with its purchase (IMP-030).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
pub struct PreviewCounts {
    /// Ready rows.
    pub ready: u32,
    /// Rejected rows.
    pub rejected: u32,
    /// Rows of an unknown asset.
    pub unknown_asset: u32,
    /// Rows already recorded.
    pub already_recorded: u32,
    /// Rows not imported.
    pub not_imported: u32,
}

/// IMP-030 — a file read through a mapping, each row judged; nothing written (IMP-017).
#[derive(Debug, Clone, Serialize, Type)]
pub struct ImportPreview {
    /// How the file was read.
    pub file: FileRead,
    /// Every row that is not blank, in the file's order.
    pub rows: Vec<PreviewRow>,
    /// How many rows hold each status.
    pub counts: PreviewCounts,
}

/// Reads a broker's export for an account and previews it (IMP-010 to IMP-038).
pub struct CsvImportUseCase {
    import_service: Arc<ImportService>,
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    /// IMP-010 — the files the user picked in the system dialog, the only ones read.
    picked: Mutex<HashSet<PathBuf>>,
}

/// A row on its way to the preview: its status once known, or the step it stands for.
enum Pending {
    Done(RowStatus),
    Judged {
        operation: OperationRead,
        asset_id: Option<String>,
        step: ImportStep,
    },
}

impl CsvImportUseCase {
    /// Creates a new CsvImportUseCase.
    pub fn new(
        import_service: Arc<ImportService>,
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            import_service,
            account_service,
            asset_service,
            picked: Mutex::new(HashSet::new()),
        }
    }

    /// IMP-010 — `path`, picked by the user in the system dialog, may be read; the path the
    /// other commands take, returned.
    pub async fn allow_file(&self, path: &Path) -> Result<String, CsvImportError> {
        let canonical = tokio::fs::canonicalize(path).await.map_err(unreadable)?;
        let shown = canonical
            .to_str()
            .ok_or(CsvImportTask::FileUnreadable)?
            .to_string();
        self.picked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(canonical);
        Ok(shown)
    }

    /// IMP-010 to IMP-016 — how the file at `path` reads for `account_id`, with what the user
    /// chose, the rest detected.
    pub async fn read_import_file(
        &self,
        account_id: &str,
        path: &str,
        choices: &ReadingChoices,
    ) -> Result<FileRead, CsvImportError> {
        self.account_currency(account_id).await?;
        let bytes = self.read_bytes(path).await?;
        Ok(read_file(&bytes, choices).map(|(_, read)| read)?)
    }

    /// IMP-030 — the file at `path` read through `mapping` for `account_id`, every row
    /// judged. `mapping_id` names the saved mapping it was picked from, when it was (IMP-020):
    /// its matches stand, any other is made now and must name an asset (IMP-033).
    pub async fn preview_import(
        &self,
        account_id: &str,
        path: &str,
        mapping_id: Option<&str>,
        mapping: &ColumnMappingDraft,
    ) -> Result<ImportPreview, CsvImportError> {
        let currency = self.account_currency(account_id).await?;
        mapping.columns.check()?;
        let saved: Vec<AssetMatch> = match mapping_id {
            Some(id) => {
                self.import_service
                    .get_column_mapping(id)
                    .await?
                    .asset_matches
            }
            None => Vec::new(),
        };
        let assets = self
            .asset_service
            .get_non_cash_assets_by_name(true)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "preview_import: assets");
                ImportError::DatabaseError
            })?;
        let known: HashSet<&str> = assets.iter().map(|asset| asset.id.as_str()).collect();
        if let Some(made) = mapping
            .asset_matches
            .iter()
            .find(|matched| !saved.contains(matched) && !known.contains(matched.asset_id.as_str()))
        {
            return Err(CsvImportTask::MatchedAssetNotFound {
                asset_id: made.asset_id.clone(),
            }
            .into());
        }
        let choices = ReadingChoices {
            encoding: Some(mapping.encoding),
            separator: Some(mapping.separator),
            decimal_mark: Some(mapping.decimal_mark),
            date_format: Some(mapping.date_format),
            date_column: Some(mapping.columns.date.clone()),
        };
        let bytes = self.read_bytes(path).await?;
        let (file, read) = read_file(&bytes, &choices)?;
        let format = RowFormat {
            decimal_mark: mapping.decimal_mark,
            date_format: mapping.date_format,
        };
        let rows = read_rows(&file, &mapping.columns, &mapping.operation_types, format)?;
        let pending: Vec<(u32, Pending)> = rows
            .into_iter()
            .map(|row| {
                let pending = pend(row.read, &mapping.asset_matches, &assets, &currency);
                (row.line, pending)
            })
            .collect();
        let rows = self.judge(account_id, pending).await?;
        Ok(ImportPreview {
            counts: count(&rows),
            file: read,
            rows,
        })
    }

    /// The account's currency, refused when the account is unknown (IMP-010).
    async fn account_currency(&self, account_id: &str) -> Result<String, AccountError> {
        self.account_service
            .get_by_id(account_id)
            .await?
            .map(|account| account.currency)
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })
    }

    /// IMP-035 — the steps judged in date order, the file's order within one day, then each
    /// row's status in the file's order.
    async fn judge(
        &self,
        account_id: &str,
        pending: Vec<(u32, Pending)>,
    ) -> Result<Vec<PreviewRow>, CsvImportError> {
        let mut order: Vec<usize> = pending
            .iter()
            .enumerate()
            .filter(|(_, (_, row))| matches!(row, Pending::Judged { .. }))
            .map(|(index, _)| index)
            .collect();
        let date = |index: &usize| match pending.get(*index) {
            Some((_, Pending::Judged { operation, .. })) => operation.date.clone(),
            _ => String::new(),
        };
        order.sort_by_key(date);
        let steps: Vec<ImportStep> = order
            .iter()
            .filter_map(|index| match pending.get(*index) {
                Some((_, Pending::Judged { step, .. })) => Some(step.clone()),
                _ => None,
            })
            .collect();
        let judgments = self.account_service.judge_import(account_id, steps).await?;
        let mut judged: Vec<Option<ImportJudgment>> = vec![None; pending.len()];
        for (index, judgment) in order.into_iter().zip(judgments) {
            if let Some(slot) = judged.get_mut(index) {
                *slot = Some(judgment);
            }
        }
        pending
            .into_iter()
            .zip(judged)
            .map(|((line, row), judgment)| {
                let status = match (row, judgment) {
                    (Pending::Done(status), _) => status,
                    (
                        Pending::Judged {
                            operation,
                            asset_id,
                            ..
                        },
                        Some(judgment),
                    ) => status_of(operation, asset_id, judgment),
                    (Pending::Judged { .. }, None) => {
                        tracing::error!(target: BACKEND, line, "preview_import: a step not judged");
                        return Err(CsvImportTask::UnknownError.into());
                    }
                };
                Ok(PreviewRow { line, status })
            })
            .collect()
    }

    /// IMP-010 — the file's bytes, refused when the user did not pick it, when it is larger
    /// than 10 MB or cannot be read.
    async fn read_bytes(&self, path: &str) -> Result<Vec<u8>, CsvImportError> {
        let canonical = tokio::fs::canonicalize(path).await.map_err(unreadable)?;
        let picked = self
            .picked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&canonical);
        if !picked {
            tracing::warn!(target: BACKEND, "import: a file the user did not pick");
            return Err(CsvImportTask::FileNotPicked.into());
        }
        let metadata = tokio::fs::metadata(&canonical).await.map_err(unreadable)?;
        if metadata.len() > LARGEST_FILE {
            return Err(ImportError::FileTooLarge.into());
        }
        Ok(tokio::fs::read(&canonical).await.map_err(unreadable)?)
    }
}

/// IMP-010 — a file that cannot be read, its cause logged.
fn unreadable(e: std::io::Error) -> CsvImportTask {
    tracing::warn!(target: BACKEND, err = %e, "import: the file cannot be read");
    CsvImportTask::FileUnreadable
}

/// A row read: its status when matching or the currency settles it (IMP-032, IMP-037),
/// otherwise the step the account judges.
fn pend(read: RowRead, matches: &[AssetMatch], assets: &[Asset], currency: &str) -> Pending {
    let operation = match read {
        RowRead::Operation(operation) => operation,
        RowRead::JoinedTradeTax { purchase_line } => {
            return Pending::Done(RowStatus::JoinedTradeTax { purchase_line })
        }
        RowRead::NotImported { label } => return Pending::Done(RowStatus::NotImported { label }),
        RowRead::Rejected(rejection) => {
            return Pending::Done(RowStatus::Rejected {
                reason: RowReason::Unread { rejection },
            })
        }
    };
    let other_currency = Pending::Done(RowStatus::Rejected {
        reason: RowReason::OtherCurrency,
    });
    if operation
        .currency
        .as_deref()
        .is_some_and(|own| !own.eq_ignore_ascii_case(currency))
    {
        return other_currency;
    }
    let date = operation.date.clone();
    let cash = |op: RecordingOp| (op, false);
    let (step, asset_id) = match &operation.figures {
        RowFigures::Deposit { amount } => (
            cash(RecordingOp::Deposit {
                date,
                amount: *amount,
                note: None,
            }),
            None,
        ),
        RowFigures::Withdrawal { amount } => (
            cash(RecordingOp::Withdrawal {
                date,
                amount: *amount,
                note: None,
            }),
            None,
        ),
        figures => {
            let Some(asset) = matched(&operation.asset, matches, assets) else {
                return Pending::Done(RowStatus::UnknownAsset {
                    asset: operation.asset.clone(),
                });
            };
            if !asset.currency.eq_ignore_ascii_case(currency) {
                return other_currency;
            }
            let trade = |quantity: i64, unit_price: i64, fees: i64| TradeOp {
                asset_id: asset.id.clone(),
                date: date.clone(),
                quantity,
                unit_price,
                exchange_rate: SAME_CURRENCY,
                fees,
                total_amount: None,
                note: None,
            };
            let step = match *figures {
                RowFigures::Purchase {
                    quantity,
                    unit_price,
                    fees,
                } => (RecordingOp::Buy(trade(quantity, unit_price, fees)), false),
                RowFigures::Sale {
                    quantity,
                    unit_price,
                    fees,
                } => {
                    // SEL-037 — an archived asset is not sold.
                    if asset.is_archived {
                        return Pending::Done(RowStatus::Rejected {
                            reason: RowReason::ManualEntry {
                                refusal: AccountError::ArchivedAssetSell,
                            },
                        });
                    }
                    (RecordingOp::Sell(trade(quantity, unit_price, fees)), false)
                }
                RowFigures::Dividend { amount } => (
                    RecordingOp::Dividend {
                        asset_id: asset.id.clone(),
                        date: date.clone(),
                        amount,
                        exchange_rate: SAME_CURRENCY,
                        note: None,
                    },
                    true,
                ),
                // Deposits and withdrawals are matched above; a trade tax is joined or
                // rejected when the rows are read (IMP-034).
                _ => {
                    return Pending::Done(RowStatus::Rejected {
                        reason: RowReason::Unread {
                            rejection: RowRejection::TradeTaxUnpaired,
                        },
                    })
                }
            };
            (step, Some(asset.id.clone()))
        }
    };
    let (op, needs_held) = step;
    Pending::Judged {
        operation,
        asset_id,
        step: ImportStep { op, needs_held },
    }
}

/// IMP-032 / IMP-033 — the asset a row names: the mapping's matches first, then its ISIN,
/// its reference and its name, the first matching exactly one asset.
fn matched<'a>(read: &AssetRead, matches: &[AssetMatch], assets: &'a [Asset]) -> Option<&'a Asset> {
    let by_code = |code: &str| code.trim().to_lowercase();
    // A match is compared as the field it was read from is (IMP-033).
    let fields = [
        read.isin.as_deref().map(by_code),
        read.reference.as_deref().map(by_code),
    ];
    let name = read.name.as_deref().map(folded);
    let remembered = matches
        .iter()
        .filter(|matched| {
            fields.contains(&Some(by_code(&matched.read)))
                || name.as_ref() == Some(&folded(&matched.read))
        })
        .find_map(|matched| assets.iter().find(|asset| asset.id == matched.asset_id));
    if remembered.is_some() {
        return remembered;
    }
    let unique = |is: &dyn Fn(&Asset) -> bool| {
        let mut found = assets.iter().filter(|asset| is(asset));
        match (found.next(), found.next()) {
            (Some(asset), None) => Some(asset),
            _ => None,
        }
    };
    let by_isin = |isin: &String| {
        unique(&|asset: &Asset| {
            asset
                .isin
                .as_deref()
                .is_some_and(|own| by_code(own) == by_code(isin))
        })
    };
    let by_reference = |reference: &String| {
        unique(&|asset: &Asset| by_code(&asset.reference) == by_code(reference))
    };
    let by_name = |name: &String| unique(&|asset: &Asset| folded(&asset.name) == folded(name));
    read.isin
        .as_ref()
        .and_then(by_isin)
        .or_else(|| read.reference.as_ref().and_then(by_reference))
        .or_else(|| read.name.as_ref().and_then(by_name))
}

/// The status a judgment gives a row (IMP-035, IMP-036, IMP-038).
fn status_of(
    operation: OperationRead,
    asset_id: Option<String>,
    judgment: ImportJudgment,
) -> RowStatus {
    match judgment {
        ImportJudgment::Ready { funding } => RowStatus::Ready {
            operation,
            asset_id,
            funding,
        },
        ImportJudgment::AlreadyRecorded => RowStatus::AlreadyRecorded {
            operation,
            asset_id,
        },
        ImportJudgment::NotHeld => RowStatus::Rejected {
            reason: RowReason::AssetNotHeld,
        },
        ImportJudgment::Refused(refusal) => RowStatus::Rejected {
            reason: RowReason::ManualEntry { refusal },
        },
    }
}

/// IMP-030 — the rows of each status; a joined trade tax counts with its purchase.
fn count(rows: &[PreviewRow]) -> PreviewCounts {
    rows.iter()
        .fold(PreviewCounts::default(), |mut counts, row| {
            match row.status {
                RowStatus::Ready { .. } => counts.ready += 1,
                RowStatus::Rejected { .. } => counts.rejected += 1,
                RowStatus::UnknownAsset { .. } => counts.unknown_asset += 1,
                RowStatus::AlreadyRecorded { .. } => counts.already_recorded += 1,
                RowStatus::NotImported { .. } => counts.not_imported += 1,
                RowStatus::JoinedTradeTax { .. } => {}
            }
            counts
        })
}
