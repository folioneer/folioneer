//! Integration tests for the CSV import preview (IMP spec): a broker's export read through a
//! column mapping, each row matched to an asset and judged as manual entry would judge it.
//!
//! Exercises the full stack through the public `folioneer_lib` API:
//! `CsvImportUseCase` → `ImportService` / `AccountService` / `AssetService` → real
//! in-memory SQLite. No mocks — per test-rules.md Tier 3 constraint. The two fixtures are
//! invented data in the shape of a French broker's export and of an English one.

use folioneer_lib::context::account::{
    AccountError, AccountService, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteTransactionRepository,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::import::{
    AssetMatch, ColumnMappingDraft, DateFormat, DecimalMark, Encoding, ImportError, ImportService,
    MappedColumns, OperationLabel, OperationType, ReadingChoices, RowRejection, Separator,
    SqliteColumnMappingRepository,
};
use folioneer_lib::use_cases::csv_import::{
    CsvImportError, CsvImportTask, CsvImportUseCase, ImportPreview, PreviewCounts, RowReason,
    RowStatus,
};
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

struct Ctx {
    import: CsvImportUseCase,
    mappings: Arc<ImportService>,
    recording: HoldingTransactionUseCase,
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
}

async fn build_ctx() -> Ctx {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("test pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let account_service = Arc::new(AccountService::new(
        Box::new(SqliteAccountRepository::new(pool.clone())),
        Box::new(SqliteHoldingRepository::new(pool.clone())),
        Box::new(SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone())),
        Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
        Box::new(SqliteAssetPriceRepository::new(pool.clone())),
    ));
    let mappings = Arc::new(ImportService::new(Box::new(
        SqliteColumnMappingRepository::new(pool.clone()),
    )));
    Ctx {
        import: CsvImportUseCase::new(
            Arc::clone(&mappings),
            account_service.clone(),
            asset_service.clone(),
        ),
        mappings,
        recording: HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone()),
        account_service,
        asset_service,
    }
}

async fn asset(
    ctx: &Ctx,
    name: &str,
    reference: &str,
    isin: Option<&str>,
    currency: &str,
) -> String {
    ctx.asset_service
        .create_asset(CreateAssetDTO {
            kind: None,
            name: name.to_string(),
            reference: reference.to_string(),
            isin: isin.map(str::to_string),
            class: AssetClass::Stocks,
            currency: currency.to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("asset")
        .id
}

/// An EUR account, funded with `cash` on 2024-01-01 when it is not zero.
async fn account(ctx: &Ctx, cash: i64) -> String {
    let account = ctx
        .account_service
        .create("PEA".to_string(), String::new(), "EUR".to_string(), false)
        .await
        .expect("account");
    if cash > 0 {
        ctx.recording
            .record_deposit(&account.id, "2024-01-01".to_string(), cash, None)
            .await
            .expect("deposit");
    }
    account.id
}

fn labels(pairs: &[(&str, OperationType)]) -> Vec<OperationLabel> {
    pairs
        .iter()
        .map(|(label, transaction_type)| OperationLabel {
            label: (*label).to_string(),
            transaction_type: *transaction_type,
        })
        .collect()
}

fn header(name: &str) -> Option<String> {
    Some(name.to_string())
}

/// The French broker's mapping: `;`, Windows-1252, decimal dot, day/month/year, no ISIN.
/// "Vente Comptant" is a plausible sale label, not one read in a real export.
fn french_mapping() -> ColumnMappingDraft {
    ColumnMappingDraft {
        name: "Courtier".to_string(),
        separator: Separator::Semicolon,
        decimal_mark: DecimalMark::Dot,
        date_format: DateFormat::DayMonthYear,
        encoding: Encoding::Windows1252,
        columns: MappedColumns {
            date: "Date".to_string(),
            operation: "Opération".to_string(),
            isin: None,
            reference: None,
            name: header("libellé"),
            quantity: header("Qté"),
            unit_price: header("Prix d'éxé"),
            gross_amount: header("Montant brut"),
            fees: header("Courtage/Prélèvement"),
            net_amount: header("Montant net"),
            currency: header("Devise"),
        },
        operation_types: labels(&[
            ("Achat Comptant", OperationType::Purchase),
            ("Vente Comptant", OperationType::Sale),
            (
                "Encaissement coupons intérêt/dividende",
                OperationType::Dividend,
            ),
            ("TAXE TRANSAC FINAN", OperationType::TradeTax),
            (
                "OST de création de coupons - Détachement coupon optionnel",
                OperationType::NotImported,
            ),
        ]),
        asset_matches: Vec::new(),
    }
}

/// The English broker's mapping: `,`, UTF-8, decimal dot, year-month-day, ISIN and ticker.
fn english_mapping() -> ColumnMappingDraft {
    ColumnMappingDraft {
        name: "Broker".to_string(),
        separator: Separator::Comma,
        decimal_mark: DecimalMark::Dot,
        date_format: DateFormat::YearMonthDay,
        encoding: Encoding::Utf8,
        columns: MappedColumns {
            date: "Date".to_string(),
            operation: "Type".to_string(),
            isin: header("ISIN"),
            reference: header("Ticker"),
            name: header("Name"),
            quantity: header("Quantity"),
            unit_price: header("Price"),
            gross_amount: header("Amount"),
            fees: header("Commission"),
            net_amount: None,
            currency: header("Currency"),
        },
        operation_types: labels(&[
            ("Deposit", OperationType::Deposit),
            ("Withdrawal", OperationType::Withdrawal),
            ("Buy", OperationType::Purchase),
            ("Sell", OperationType::Sale),
            ("Dividend", OperationType::Dividend),
        ]),
        asset_matches: Vec::new(),
    }
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/import/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture")
}

/// `text` written to a temporary file; with `latin`, as the French broker writes it: Latin-1,
/// each line ended by CRLF.
fn file_of(text: &str, latin: bool) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp file");
    let bytes: Vec<u8> = if latin {
        text.replace("\r\n", "\n")
            .replace('\n', "\r\n")
            .chars()
            .map(|c| u8::try_from(u32::from(c)).expect("latin-1"))
            .collect()
    } else {
        text.as_bytes().to_vec()
    };
    file.write_all(&bytes).expect("write");
    file
}

/// `text` in a temporary file, as `file_of` writes it, picked by the user.
async fn picked(ctx: &Ctx, text: &str, latin: bool) -> NamedTempFile {
    let file = file_of(text, latin);
    ctx.import.allow_file(file.path()).await.expect("picked");
    file
}

async fn preview(
    ctx: &Ctx,
    account_id: &str,
    file: &NamedTempFile,
    mapping: &ColumnMappingDraft,
) -> ImportPreview {
    ctx.import
        .preview_import(account_id, path(file), None, mapping)
        .await
        .expect("preview")
}

fn path(file: &NamedTempFile) -> &str {
    file.path().to_str().expect("path")
}

fn status(preview: &ImportPreview, line: u32) -> &RowStatus {
    &preview
        .rows
        .iter()
        .find(|row| row.line == line)
        .expect("line")
        .status
}

/// The French fixture's portfolio: two assets named as the export names them, accents and
/// case aside.
async fn french_portfolio(ctx: &Ctx) -> (String, String, String) {
    let acme = asset(ctx, "Acme Monde", "ACM", None, "EUR").await;
    let beta = asset(ctx, "Bêta Énergie", "BET", None, "EUR").await;
    (account(ctx, micro(1_000)).await, acme, beta)
}

// IMP-010 / IMP-011 / IMP-030 / IMP-032 / IMP-034 — the French export, in Latin-1, read
// through its mapping: each row ready, joined, not imported or unknown, names matched with
// case and accents ignored, the trade tax in its purchase's fees.
#[tokio::test]
async fn imp_030_the_preview_judges_every_row_of_a_french_export() {
    let ctx = build_ctx().await;
    let (account_id, acme, beta) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let read = ctx
        .import
        .read_import_file(&account_id, path(&file), &ReadingChoices::default())
        .await
        .expect("read");
    assert_eq!(read.encoding, Encoding::Windows1252);
    assert_eq!(read.separator, Separator::Semicolon);
    assert_eq!(read.decimal_mark, DecimalMark::Dot);
    assert_eq!(read.columns.first().map(String::as_str), Some("libellé"));
    let preview = preview(&ctx, &account_id, &file, &french_mapping()).await;
    assert!(matches!(
        status(&preview, 2),
        RowStatus::Ready { asset_id: Some(id), funding: None, operation }
            if *id == acme
                && matches!(operation.figures, folioneer_lib::context::import::RowFigures::Purchase { fees, .. } if fees == 2_330_000)
    ));
    assert!(matches!(
        status(&preview, 3),
        RowStatus::JoinedTradeTax { purchase_line: 2 }
    ));
    assert!(
        matches!(status(&preview, 4), RowStatus::Ready { asset_id: Some(id), .. } if *id == beta)
    );
    assert!(matches!(status(&preview, 5), RowStatus::Ready { .. }));
    assert!(matches!(status(&preview, 6), RowStatus::Ready { .. }));
    assert!(matches!(status(&preview, 7), RowStatus::NotImported { .. }));
    assert!(matches!(
        status(&preview, 8),
        RowStatus::UnknownAsset { asset } if asset.name.as_deref() == Some("INCONNUE SA")
    ));
    assert_eq!(
        preview.counts,
        PreviewCounts {
            ready: 4,
            rejected: 0,
            unknown_asset: 1,
            already_recorded: 0,
            not_imported: 1,
        }
    );
}

// IMP-017 / IMP-036 — previewing writes nothing; a row the account already holds, its
// trade tax in its fees, is already recorded, and the rows after it are judged with it.
#[tokio::test]
async fn imp_036_a_row_already_recorded_is_shown_so() {
    let ctx = build_ctx().await;
    let (account_id, acme, _) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let before = ctx
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
        .expect("transactions")
        .len();
    preview(&ctx, &account_id, &file, &french_mapping()).await;
    let after = ctx
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
        .expect("transactions")
        .len();
    assert_eq!(before, after, "IMP-017: the preview writes nothing");
    ctx.recording
        .buy_holding(
            &account_id,
            acme,
            "2024-01-03".to_string(),
            micro(10),
            12_500_000,
            micro(1),
            2_330_000,
            None,
            None,
        )
        .await
        .expect("buy");
    let preview = preview(&ctx, &account_id, &file, &french_mapping()).await;
    assert!(matches!(
        status(&preview, 2),
        RowStatus::AlreadyRecorded { .. }
    ));
    assert!(matches!(status(&preview, 5), RowStatus::Ready { .. }));
    assert_eq!(preview.counts.already_recorded, 1);
    assert_eq!(preview.counts.ready, 3);
}

// IMP-031 / IMP-035 / IMP-037 / IMP-038 — the English export: a sale above what is held, a
// withdrawal above the cash, another currency and a dividend of an asset not held are
// rejected; a purchase the cash cannot fund stays ready with the deposit it needs.
#[tokio::test]
async fn imp_035_rows_follow_the_rules_of_manual_entry() {
    let ctx = build_ctx().await;
    asset(&ctx, "Acme World", "ACM", Some("FR0000000010"), "EUR").await;
    asset(&ctx, "Beta Power", "BTP", Some("FR0000000002"), "EUR").await;
    let account_id = account(&ctx, 0).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let preview = preview(&ctx, &account_id, &file, &english_mapping()).await;
    assert!(matches!(
        status(&preview, 2),
        RowStatus::Ready { asset_id: None, .. }
    ));
    assert!(matches!(
        status(&preview, 3),
        RowStatus::Ready { funding: None, .. }
    ));
    assert!(
        matches!(
            status(&preview, 4),
            RowStatus::Rejected {
                reason: RowReason::ManualEntry {
                    refusal: AccountError::Oversell { .. }
                }
            }
        ),
        "SEL-021: {:?}",
        status(&preview, 4)
    );
    assert!(
        matches!(
            status(&preview, 5),
            RowStatus::Rejected {
                reason: RowReason::ManualEntry {
                    refusal: AccountError::InsufficientCash { .. }
                }
            }
        ),
        "CSH-031: {:?}",
        status(&preview, 5)
    );
    assert!(matches!(
        status(&preview, 6),
        RowStatus::Rejected {
            reason: RowReason::OtherCurrency
        }
    ));
    assert!(matches!(
        status(&preview, 7),
        RowStatus::Rejected {
            reason: RowReason::AssetNotHeld
        }
    ));
    assert!(
        matches!(status(&preview, 8), RowStatus::Ready { funding: Some(funding), .. } if *funding == micro(701)),
        "IMP-038: {:?}",
        status(&preview, 8)
    );
}

// IMP-032 — the ISIN is tried before the name; two assets of one name are no match; an
// archived asset is matched, and a sale of it is refused (SEL-037).
#[tokio::test]
async fn imp_032_each_row_is_matched_to_an_asset() {
    let ctx = build_ctx().await;
    let by_isin = asset(&ctx, "Acme Holding", "AH", Some("FR0000000010"), "EUR").await;
    asset(&ctx, "Acme World", "AW1", None, "EUR").await;
    asset(&ctx, "acme world", "AW2", None, "EUR").await;
    let account_id = account(&ctx, micro(10_000)).await;
    let rows = "Date,Type,ISIN,Ticker,Name,Quantity,Price,Amount,Commission,Currency\n\
                2024-02-02,Buy,FR0000000010,,Acme World,1,10,10,0,EUR\n\
                2024-02-02,Buy,,,Acme World,1,10,10,0,EUR\n\
                2024-02-03,Sell,FR0000000010,,,1,10,10,0,EUR\n";
    let file = picked(&ctx, rows, false).await;
    let preview_once = preview(&ctx, &account_id, &file, &english_mapping()).await;
    assert!(
        matches!(status(&preview_once, 2), RowStatus::Ready { asset_id: Some(id), .. } if *id == by_isin)
    );
    assert!(matches!(
        status(&preview_once, 3),
        RowStatus::UnknownAsset { .. }
    ));
    ctx.asset_service
        .archive_asset(&by_isin)
        .await
        .expect("archive");
    let archived = preview(&ctx, &account_id, &file, &english_mapping()).await;
    assert!(
        matches!(status(&archived, 2), RowStatus::Ready { .. }),
        "an archived asset is matched"
    );
    assert!(matches!(
        status(&archived, 4),
        RowStatus::Rejected {
            reason: RowReason::ManualEntry {
                refusal: AccountError::ArchivedAssetSell
            }
        }
    ));
}

// IMP-033 — an unknown asset matched once takes its asset; a match made to an asset the
// portfolio lacks is refused, a saved one whose asset has not arrived leaves its rows unknown.
#[tokio::test]
async fn imp_033_an_unknown_asset_is_matched_once() {
    let ctx = build_ctx().await;
    let (account_id, _, beta) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let mut mapping = french_mapping();
    mapping.asset_matches = vec![AssetMatch {
        read: "inconnue sa".to_string(),
        asset_id: beta.clone(),
    }];
    let matched = preview(&ctx, &account_id, &file, &mapping).await;
    assert!(
        matches!(status(&matched, 8), RowStatus::Ready { asset_id: Some(id), .. } if *id == beta)
    );
    mapping.asset_matches = vec![AssetMatch {
        read: "INCONNUE SA".to_string(),
        asset_id: "not-an-asset".to_string(),
    }];
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &mapping)
        .await;
    assert!(matches!(
        refused,
        Err(CsvImportError::UseCase(CsvImportTask::MatchedAssetNotFound { asset_id })) if asset_id == "not-an-asset"
    ));
    let saved = ctx
        .mappings
        .save_column_mapping(None, mapping.clone())
        .await
        .expect("saved");
    let waiting = ctx
        .import
        .preview_import(&account_id, path(&file), Some(&saved.id), &mapping)
        .await
        .expect("preview");
    assert!(matches!(
        status(&waiting, 8),
        RowStatus::UnknownAsset { .. }
    ));
}

// IMP-010 / IMP-020 / IMP-021 — an unknown account, a file that cannot be read or was not
// picked, a saved mapping no longer held and a mapping without its date column are refused.
#[tokio::test]
async fn imp_010_what_cannot_be_read_is_refused() {
    let ctx = build_ctx().await;
    let account_id = account(&ctx, 0).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let unknown = ctx
        .import
        .read_import_file("nope", path(&file), &ReadingChoices::default())
        .await;
    assert!(
        matches!(unknown, Err(CsvImportError::Account(AccountError::AccountNotFound { account_id })) if account_id == "nope")
    );
    let missing = ctx
        .import
        .read_import_file(&account_id, "/no/such/file.csv", &ReadingChoices::default())
        .await;
    assert!(matches!(
        missing,
        Err(CsvImportError::UseCase(CsvImportTask::FileUnreadable))
    ));
    let unpicked = file_of(&fixture("english_broker.csv"), false);
    let refused = ctx
        .import
        .read_import_file(&account_id, path(&unpicked), &ReadingChoices::default())
        .await;
    assert!(
        matches!(
            refused,
            Err(CsvImportError::UseCase(CsvImportTask::FileNotPicked))
        ),
        "IMP-010: only a file the user picked is read"
    );
    let gone = ctx
        .import
        .preview_import(&account_id, path(&file), Some("gone"), &english_mapping())
        .await;
    assert!(matches!(
        gone,
        Err(CsvImportError::Import(ImportError::MappingNotFound { .. }))
    ));
    let mut no_date = english_mapping();
    no_date.columns.date = " ".to_string();
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &no_date)
        .await;
    assert!(matches!(
        refused,
        Err(CsvImportError::Import(ImportError::MappingColumnMissing))
    ));
    let mut absent = english_mapping();
    absent.columns.fees = header("Fees");
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &absent)
        .await;
    assert!(
        matches!(refused, Err(CsvImportError::Import(ImportError::MappedColumnNotInFile { column })) if column == "Fees")
    );
    let mut untyped = english_mapping();
    untyped.operation_types.retain(|label| label.label != "Buy");
    let preview = preview(&ctx, &account_id, &file, &untyped).await;
    assert!(matches!(
        status(&preview, 3),
        RowStatus::Rejected { reason: RowReason::Unread { rejection: RowRejection::OperationTypeMissing { label } } }
            if label == "Buy"
    ));
}
