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
use folioneer_lib::context::account::{Transaction, TransactionType};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::import::{
    AssetMatch, ColumnMappingDraft, DateFormat, DecimalMark, Encoding, ImportError, ImportService,
    MappedColumns, OperationLabel, OperationType, ReadingChoices, RowRejection, Separator,
    SqliteColumnMappingRepository,
};
use folioneer_lib::core::event_bus::Event;
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::shared::infrastructure::uow::SqliteTransactionManager;
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
    bus: Arc<SideEffectEventBus>,
    pool: sqlx::SqlitePool,
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
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let mappings = Arc::new(ImportService::new(Box::new(
        SqliteColumnMappingRepository::new(pool.clone()),
    )));
    Ctx {
        import: CsvImportUseCase::new(
            Arc::clone(&mappings),
            account_service.clone(),
            asset_service.clone(),
            Arc::new(SqliteTransactionManager::new(pool.clone())),
        ),
        mappings,
        recording: HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone()),
        account_service,
        asset_service,
        bus,
        pool,
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

async fn transactions(ctx: &Ctx, account_id: &str) -> Vec<Transaction> {
    ctx.account_service
        .get_all_transactions_for_account(account_id)
        .await
        .expect("transactions")
}

fn deposits(transactions: &[Transaction]) -> Vec<&Transaction> {
    transactions
        .iter()
        .filter(|t| t.transaction_type == TransactionType::Deposit)
        .collect()
}

async fn confirm(
    ctx: &Ctx,
    account_id: &str,
    file: &NamedTempFile,
    mapping: &ColumnMappingDraft,
    add_deposits: bool,
) -> ImportPreview {
    ctx.import
        .confirm_import(account_id, path(file), None, mapping, add_deposits)
        .await
        .expect("confirmed")
}

/// The English fixture's portfolio: its two assets and an account with no cash.
async fn english_portfolio(ctx: &Ctx) -> String {
    asset(ctx, "Acme World", "ACM", Some("FR0000000010"), "EUR").await;
    asset(ctx, "Beta Power", "BTP", Some("FR0000000002"), "EUR").await;
    account(ctx, 0).await
}

// IMP-050 / IMP-051 — the ready rows are recorded at once, in date order, raising one
// `TransactionUpdated`; the deposit a purchase needs is added before it, on its date, marked
// as added; the rows the preview rejects are left out.
#[tokio::test]
async fn imp_050_a_confirmed_import_records_every_ready_row_at_once() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let mut events = ctx.bus.subscribe();
    let confirmed = confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    let raised = events.drain();
    let updated = raised
        .iter()
        .filter(|event| **event == Event::TransactionUpdated)
        .count();
    assert_eq!(
        updated, 1,
        "IMP-050A: one TransactionUpdated for the whole import"
    );
    assert!(
        raised.contains(&Event::AssetUpdated),
        "IMP-050A: the Cash Asset the deposit needs was created"
    );
    assert_eq!(confirmed.counts.ready, 3);
    assert!(matches!(status(&confirmed, 4), RowStatus::Rejected { .. }));
    let recorded = transactions(&ctx, &account_id).await;
    let kinds: Vec<(TransactionType, &str, bool)> = recorded
        .iter()
        .map(|t| (t.transaction_type, t.date.as_str(), t.added_deposit))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (TransactionType::Deposit, "2024-02-01", false),
            (TransactionType::Purchase, "2024-02-02", false),
            (TransactionType::Deposit, "2024-02-07", true),
            (TransactionType::Purchase, "2024-02-07", false),
        ],
        "IMP-050 / IMP-051: the ready rows in date order, the added deposit before its purchase"
    );
    let added = deposits(&recorded)
        .into_iter()
        .find(|t| t.added_deposit)
        .expect("added deposit");
    assert_eq!(
        added.total_amount,
        micro(701),
        "IMP-051: the deposit the preview showed"
    );
}

const HEADER: &str = "Date,Type,ISIN,Ticker,Name,Quantity,Price,Amount,Commission,Currency\n";

// IMP-035A / IMP-038 — within one day the inflows come first: a deposit written after the
// purchase it funds funds it, so no deposit is added and the cash is not counted twice.
#[tokio::test]
async fn imp_035a_a_days_own_deposit_funds_its_purchases() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let rows = format!(
        "{HEADER}2024-02-01,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,1.00,EUR\n\
         2024-02-01,Deposit,,,,,,201.00,,EUR\n"
    );
    let file = picked(&ctx, &rows, false).await;
    let previewed = preview(&ctx, &account_id, &file, &english_mapping()).await;
    assert!(
        matches!(
            status(&previewed, 2),
            RowStatus::Ready { funding: None, .. }
        ),
        "IMP-038: the day's own deposit funds the purchase"
    );
    confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    let recorded = transactions(&ctx, &account_id).await;
    let kinds: Vec<(TransactionType, bool)> = recorded
        .iter()
        .map(|t| (t.transaction_type, t.added_deposit))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (TransactionType::Deposit, false),
            (TransactionType::Purchase, false),
        ],
        "IMP-035A: the deposit recorded first, none added"
    );
}

// IMP-036 / IMP-053 — a sale recorded before its asset was archived is already recorded when
// the file comes again, not refused as a new sale of an archived asset (SEL-037).
#[tokio::test]
async fn imp_036_a_recorded_sale_of_an_asset_since_archived_is_already_recorded() {
    let ctx = build_ctx().await;
    let acme = asset(&ctx, "Acme World", "ACM", Some("FR0000000010"), "EUR").await;
    let account_id = account(&ctx, micro(1000)).await;
    let rows = format!(
        "{HEADER}2024-02-01,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,0,EUR\n\
         2024-02-02,Sell,FR0000000010,ACM,Acme World,2,110.00,220.00,0,EUR\n"
    );
    let file = picked(&ctx, &rows, false).await;
    confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    ctx.asset_service
        .archive_asset(&acme)
        .await
        .expect("archived");
    let again = preview(&ctx, &account_id, &file, &english_mapping()).await;
    assert!(matches!(
        status(&again, 3),
        RowStatus::AlreadyRecorded { .. }
    ));
    let new_sale =
        format!("{HEADER}2024-02-03,Sell,FR0000000010,ACM,Acme World,1,110.00,110.00,0,EUR\n");
    let later = picked(&ctx, &new_sale, false).await;
    let refused = preview(&ctx, &account_id, &later, &english_mapping()).await;
    assert!(
        matches!(
            status(&refused, 2),
            RowStatus::Rejected {
                reason: RowReason::ManualEntry {
                    refusal: AccountError::ArchivedAssetSell
                }
            }
        ),
        "SEL-037: a new sale of an archived asset stays refused"
    );
}

// IMP-036 — two such rows match two such transactions, not one; a withdrawal too.
#[tokio::test]
async fn imp_036_each_transaction_matches_one_row() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let rows = format!(
        "{HEADER}2024-02-01,Deposit,,,,,,50.00,,EUR\n\
         2024-02-02,Withdrawal,,,,,,10.00,,EUR\n"
    );
    let first = picked(&ctx, &rows, false).await;
    confirm(&ctx, &account_id, &first, &english_mapping(), true).await;
    let twice = format!(
        "{HEADER}2024-02-01,Deposit,,,,,,50.00,,EUR\n\
         2024-02-01,Deposit,,,,,,50.00,,EUR\n\
         2024-02-02,Withdrawal,,,,,,10.00,,EUR\n"
    );
    let later = picked(&ctx, &twice, false).await;
    let previewed = preview(&ctx, &account_id, &later, &english_mapping()).await;
    assert!(matches!(
        status(&previewed, 2),
        RowStatus::AlreadyRecorded { .. }
    ));
    assert!(matches!(status(&previewed, 3), RowStatus::Ready { .. }));
    assert!(matches!(
        status(&previewed, 4),
        RowStatus::AlreadyRecorded { .. }
    ));
}

// IMP-035A — the order runs over dates whatever the file's, a sale is no inflow, and a
// dividend on the day of a first purchase comes before it and is rejected (DIV-011).
#[tokio::test]
async fn imp_035a_the_order_of_a_days_rows() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let rows = format!(
        "{HEADER}2024-02-03,Buy,FR0000000010,ACM,Acme World,1,100.00,100.00,0,EUR\n\
         2024-02-03,Dividend,FR0000000002,,,,,3.00,,EUR\n\
         2024-02-01,Deposit,,,,,,100.00,,EUR\n\
         2024-02-02,Buy,FR0000000002,BTP,Beta Power,1,100.00,100.00,0,EUR\n\
         2024-02-02,Sell,FR0000000002,BTP,Beta Power,1,100.00,100.00,0,EUR\n\
         2024-02-04,Dividend,FR0000000010,,,,,1.00,,EUR\n\
         2024-02-05,Buy,FR0000000002,BTP,Beta Power,1,100.00,100.00,0,EUR\n\
         2024-02-05,Dividend,FR0000000002,,,,,1.00,,EUR\n"
    );
    let file = picked(&ctx, &rows, false).await;
    let previewed = preview(&ctx, &account_id, &file, &english_mapping()).await;
    assert!(
        matches!(
            status(&previewed, 2),
            RowStatus::Ready { funding: None, .. }
        ),
        "the sale of the day before funds it, the file's order aside"
    );
    assert!(
        matches!(
            status(&previewed, 5),
            RowStatus::Ready { funding: None, .. }
        ),
        "the day's first purchase, funded by the earlier deposit"
    );
    assert!(
        matches!(
            status(&previewed, 9),
            RowStatus::Rejected {
                reason: RowReason::AssetNotHeld
            }
        ),
        "IMP-035A: the dividend comes before the day's first purchase"
    );
}

// IMP-038 — the rows after a funded purchase are judged as if its deposit were added, and a
// day's own deposit leaves only what it does not cover to add.
#[tokio::test]
async fn imp_038_each_funding_counts_what_came_before() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let rows = format!(
        "{HEADER}2024-02-01,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,0,EUR\n\
         2024-02-01,Deposit,,,,,,50.00,,EUR\n\
         2024-02-02,Buy,FR0000000010,ACM,Acme World,1,100.00,100.00,0,EUR\n"
    );
    let file = picked(&ctx, &rows, false).await;
    let previewed = preview(&ctx, &account_id, &file, &english_mapping()).await;
    let funding = |line| match status(&previewed, line) {
        RowStatus::Ready { funding, .. } => *funding,
        other => panic!("line {line}: {other:?}"),
    };
    assert_eq!(
        funding(2),
        Some(micro(150)),
        "what the day's deposit leaves"
    );
    assert_eq!(funding(4), Some(micro(100)), "nothing left of the first");
    let confirmed = confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    assert_eq!(confirmed.counts.ready, 3);
    let recorded = transactions(&ctx, &account_id).await;
    assert_eq!(deposits(&recorded).len(), 3, "the file's and the two added");
}

// IMP-050A — the Cash Asset already there, no `AssetUpdated` is raised.
#[tokio::test]
async fn imp_050a_no_asset_event_when_the_cash_asset_exists() {
    let ctx = build_ctx().await;
    asset(&ctx, "Acme World", "ACM", Some("FR0000000010"), "EUR").await;
    let account_id = account(&ctx, micro(1000)).await;
    let rows =
        format!("{HEADER}2024-02-01,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,0,EUR\n");
    let file = picked(&ctx, &rows, false).await;
    let mut events = ctx.bus.subscribe();
    confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    assert!(!events.drain().contains(&Event::AssetUpdated));
}

// IMP-010 — only the file last picked is read.
#[tokio::test]
async fn imp_010_only_the_file_last_picked_is_read() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let earlier = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let _last = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let refused = ctx
        .import
        .preview_import(&account_id, path(&earlier), None, &english_mapping())
        .await;
    assert!(matches!(
        refused,
        Err(CsvImportError::UseCase(CsvImportTask::FileNotPicked))
    ));
}

// IMP-035 / IMP-037 — a row is refused as manual entry would refuse it: a future date, a sale
// of nothing held, a sale leaving a later transaction short; an asset in another currency.
#[tokio::test]
async fn imp_035_a_row_is_judged_as_if_typed() {
    let ctx = build_ctx().await;
    asset(&ctx, "Dollar Fund", "USF", Some("US0378331005"), "USD").await;
    let account_id = english_portfolio(&ctx).await;
    let first = format!(
        "{HEADER}2024-03-01,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,0,EUR\n\
         2024-03-02,Sell,FR0000000010,ACM,Acme World,2,100.00,200.00,0,EUR\n"
    );
    let file = picked(&ctx, &first, false).await;
    confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    let rows = format!(
        "{HEADER}2099-01-01,Deposit,,,,,,10.00,,EUR\n\
         2024-02-01,Sell,FR0000000002,BTP,Beta Power,1,100.00,100.00,0,EUR\n\
         2024-03-01,Sell,FR0000000010,ACM,Acme World,1,100.00,100.00,0,EUR\n\
         2024-02-02,Buy,US0378331005,USF,Dollar Fund,1,100.00,100.00,0,EUR\n"
    );
    let later = picked(&ctx, &rows, false).await;
    let previewed = preview(&ctx, &account_id, &later, &english_mapping()).await;
    for (line, what) in [
        (2, "TRX-020: a future date"),
        (3, "SEL-012: nothing held"),
        (4, "SEL-032: a later sale left short"),
    ] {
        assert!(
            matches!(
                status(&previewed, line),
                RowStatus::Rejected {
                    reason: RowReason::ManualEntry { .. }
                }
            ),
            "{what}: {:?}",
            status(&previewed, line)
        );
    }
    assert!(matches!(
        status(&previewed, 5),
        RowStatus::Rejected {
            reason: RowReason::OtherCurrency
        }
    ));
}

// IMP-036A — a later file's deposit row of an added deposit's date and amount is already
// recorded, so the cash is not counted twice; the added mark stays.
#[tokio::test]
async fn imp_036a_a_later_files_deposit_row_matches_an_added_deposit() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let purchase =
        format!("{HEADER}2024-02-01,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,1.00,EUR\n");
    let first = picked(&ctx, &purchase, false).await;
    confirm(&ctx, &account_id, &first, &english_mapping(), true).await;
    let deposit = format!("{HEADER}2024-02-01,Deposit,,,,,,201.00,,EUR\n");
    let later = picked(&ctx, &deposit, false).await;
    let confirmed = confirm(&ctx, &account_id, &later, &english_mapping(), true).await;
    assert!(matches!(
        status(&confirmed, 2),
        RowStatus::AlreadyRecorded { .. }
    ));
    let recorded = transactions(&ctx, &account_id).await;
    let found = deposits(&recorded);
    assert_eq!(found.len(), 1, "IMP-036A: the cash is not counted twice");
    assert!(found[0].added_deposit, "IMP-036A: the added mark stays");
}

// IMP-050 — a write failing on the last row records none of them, and announces nothing.
#[tokio::test]
async fn imp_050_a_failed_write_records_nothing() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    sqlx::query(
        "CREATE TRIGGER refuse_the_last_row BEFORE INSERT ON transactions \
         WHEN NEW.date = '2024-02-02' BEGIN SELECT RAISE(ABORT, 'refused'); END",
    )
    .execute(&ctx.pool)
    .await
    .expect("trigger");
    let rows = format!(
        "{HEADER}2024-02-01,Deposit,,,,,,500.00,,EUR\n\
         2024-02-02,Buy,FR0000000010,ACM,Acme World,2,100.00,200.00,1.00,EUR\n"
    );
    let file = picked(&ctx, &rows, false).await;
    let mut events = ctx.bus.subscribe();
    let failed = ctx
        .import
        .confirm_import(&account_id, path(&file), None, &english_mapping(), true)
        .await;
    assert!(failed.is_err(), "IMP-050: the failed write is told");
    assert!(
        transactions(&ctx, &account_id).await.is_empty(),
        "IMP-050: all of them or none"
    );
    assert!(
        events.drain().is_empty(),
        "IMP-050A: nothing recorded, nothing announced"
    );
}

// IMP-051B — left out by the user's choice, no deposit is added, and the purchase the cash
// cannot fund is rejected as manual entry rejects it (CSH-041).
#[tokio::test]
async fn imp_051b_the_added_deposits_can_be_left_out() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let confirmed = confirm(&ctx, &account_id, &file, &english_mapping(), false).await;
    assert!(matches!(
        status(&confirmed, 8),
        RowStatus::Rejected {
            reason: RowReason::ManualEntry {
                refusal: AccountError::InsufficientCash { .. }
            }
        }
    ));
    let recorded = transactions(&ctx, &account_id).await;
    assert_eq!(recorded.len(), 2, "the deposit and the first purchase only");
    assert!(recorded.iter().all(|t| !t.added_deposit));
}

// IMP-053 — importing the same file twice records nothing new: every row the first import
// recorded is already recorded, and no event is raised.
#[tokio::test]
async fn imp_053_the_same_file_imported_twice_records_nothing_new() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    let once = transactions(&ctx, &account_id).await;
    let mut events = ctx.bus.subscribe();
    let again = confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    assert_eq!(transactions(&ctx, &account_id).await.len(), once.len());
    assert_eq!(again.counts.ready, 0);
    assert_eq!(again.counts.already_recorded, 3);
    assert!(
        events.drain().is_empty(),
        "nothing recorded, nothing announced"
    );
    let french = build_ctx().await;
    let (account_id, _, _) = french_portfolio(&french).await;
    let file = picked(&french, &fixture("french_broker.csv"), true).await;
    let first = confirm(&french, &account_id, &file, &french_mapping(), true).await;
    let again = confirm(&french, &account_id, &file, &french_mapping(), true).await;
    assert_eq!(again.counts.ready, 0, "IMP-053: {:?}", again.rows);
    assert_eq!(again.counts.already_recorded, first.counts.ready);
}

// IMP-052 — an unknown asset created from its row before confirming takes its rows, which
// are then recorded.
#[tokio::test]
async fn imp_052_an_unknown_asset_created_from_its_row_is_recorded() {
    let ctx = build_ctx().await;
    let (account_id, _, _) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let preview = preview(&ctx, &account_id, &file, &french_mapping()).await;
    let RowStatus::UnknownAsset { asset: read } = status(&preview, 8) else {
        panic!("line 8 is unknown");
    };
    let name = read.name.clone().expect("the name read");
    let created = asset(&ctx, &name, "INC", None, "EUR").await;
    let confirmed = confirm(&ctx, &account_id, &file, &french_mapping(), true).await;
    assert!(
        matches!(status(&confirmed, 8), RowStatus::Ready { asset_id: Some(id), .. } if *id == created)
    );
    assert!(transactions(&ctx, &account_id)
        .await
        .iter()
        .any(|t| t.asset_id == created && t.transaction_type == TransactionType::Purchase));
}

// IMP-051A — a corrected added deposit stays marked as added.
#[tokio::test]
async fn imp_051a_a_corrected_added_deposit_stays_added() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    confirm(&ctx, &account_id, &file, &english_mapping(), true).await;
    let added = deposits(&transactions(&ctx, &account_id).await)
        .into_iter()
        .find(|t| t.added_deposit)
        .cloned()
        .expect("added deposit");
    ctx.account_service
        .correct_transaction(
            &account_id,
            &added.id,
            added.date.clone(),
            micro(800), // a deposit's amount travels as its quantity
            added.unit_price,
            added.exchange_rate,
            0,
            None,
            Some("topped up".to_string()),
        )
        .await
        .expect("corrected");
    let corrected = transactions(&ctx, &account_id)
        .await
        .into_iter()
        .find(|t| t.id == added.id)
        .expect("still there");
    assert!(corrected.added_deposit);
    assert_eq!(corrected.total_amount, micro(800));
}
