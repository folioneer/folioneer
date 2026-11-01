//! Integration tests for the CSV import preview (IMP spec): a broker's export read through an
//! import mapper, each row matched to an asset and judged as manual entry would judge it.
//!
//! Exercises the full stack through the public `folioneer_lib` API:
//! `CsvImportUseCase` → `AccountImportService` / `AccountService` / `AssetService` → real
//! in-memory SQLite. No mocks — per test-rules.md Tier 3 constraint. The two fixtures are
//! invented data in the shape of a French broker's export and of an English one.

use folioneer_lib::context::account::{
    AccountError, AccountService, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteTransactionRepository,
};
use folioneer_lib::context::account::{Transaction, TransactionType};
use folioneer_lib::context::account_import::{
    AccountImportError, AccountImportService, AssetMatch, DateFormat, DecimalMark, Encoding,
    FsImportMapperStore, ImportMapperDraft, MappedColumns, MapperInfo, OperationLabel,
    OperationType, ReadingChoices, RowRejection, Separator, SqliteMapperChoiceRepository,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
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
    mappers: Arc<AccountImportService>,
    mappers_dir: tempfile::TempDir,
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
    let mappers_dir = tempfile::tempdir().expect("mappers folder");
    let mappers = Arc::new(AccountImportService::new(
        Box::new(FsImportMapperStore::new(
            mappers_dir.path().join("import-mappers"),
        )),
        Box::new(SqliteMapperChoiceRepository::new(pool.clone())),
    ));
    Ctx {
        import: CsvImportUseCase::new(
            Arc::clone(&mappers),
            account_service.clone(),
            asset_service.clone(),
            Arc::new(SqliteTransactionManager::new(pool.clone())),
        ),
        mappers,
        mappers_dir,
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

/// The French broker's mapper: `;`, Windows-1252, decimal dot, day/month/year, no ISIN.
/// "Vente Comptant" is a plausible sale label, not one read in a real export.
fn french_mapper() -> ImportMapperDraft {
    ImportMapperDraft {
        info: MapperInfo {
            provider: "Courtier".to_string(),
        },
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

/// The English broker's mapper: `,`, UTF-8, decimal dot, year-month-day, ISIN and ticker.
fn english_mapper() -> ImportMapperDraft {
    ImportMapperDraft {
        info: MapperInfo {
            provider: "Broker".to_string(),
        },
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
    mapper: &ImportMapperDraft,
) -> ImportPreview {
    ctx.import
        .preview_import(account_id, path(file), None, mapper)
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
// through its mapper: each row ready, joined, not imported or unknown, names matched with
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
    let preview = preview(&ctx, &account_id, &file, &french_mapper()).await;
    assert!(matches!(
        status(&preview, 2),
        RowStatus::Ready { asset_id: Some(id), funding: None, operation }
            if *id == acme
                && matches!(operation.figures, folioneer_lib::context::account_import::RowFigures::Purchase { fees, .. } if fees == 2_330_000)
    ));
    assert!(matches!(
        status(&preview, 3),
        RowStatus::JoinedTradeTax { purchase_line: 2, operation } if operation.date == "2024-01-03"
    ));
    assert!(
        matches!(status(&preview, 4), RowStatus::Ready { asset_id: Some(id), .. } if *id == beta)
    );
    assert!(matches!(status(&preview, 5), RowStatus::Ready { .. }));
    assert!(matches!(status(&preview, 6), RowStatus::Ready { .. }));
    assert!(matches!(status(&preview, 7), RowStatus::NotImported { .. }));
    assert!(matches!(
        status(&preview, 8),
        RowStatus::UnknownAsset { asset, .. } if asset.name.as_deref() == Some("INCONNUE SA")
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
    // IMP-030 — each row shows its total as manual entry computes it, or none without figures.
    let amount = |line: u32| {
        preview
            .rows
            .iter()
            .find(|row| row.line == line)
            .and_then(|row| row.amount)
    };
    assert_eq!(amount(2), Some(127_330_000), "125 + 1.95 + the joined 0.38");
    assert_eq!(amount(3), Some(380_000));
    assert_eq!(amount(5), Some(38_000_000), "39 less the 1 of fees");
    // IMP-067 — the unknown asset with its rows, the assets to match it to, no deposit needed.
    assert_eq!(preview.unknown_assets.len(), 1);
    assert_eq!(preview.unknown_assets[0].read, "INCONNUE SA");
    assert_eq!(preview.unknown_assets[0].lines, vec![8]);
    assert_eq!(
        preview.deposits.ready_without,
        preview.counts.ready - preview.deposits.count
    );
    assert!(!preview.all_recorded);
    let names: Vec<&str> = preview
        .matchable_assets
        .iter()
        .map(|asset| asset.name.as_str())
        .collect();
    assert!(names.contains(&"Acme Monde"), "{names:?}");
    assert_eq!(amount(7), None);
    assert!(
        amount(8).is_some(),
        "an unknown asset's row shows its amount"
    );
    assert!(matches!(
        status(&preview, 7),
        RowStatus::NotImported { date: Some(_), .. }
    ));
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
    preview(&ctx, &account_id, &file, &french_mapper()).await;
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
            folioneer_lib::shared::domain::quantity::from_surface(micro(10)),
            12_500_000,
            micro(1),
            2_330_000,
            None,
            None,
        )
        .await
        .expect("buy");
    let preview = preview(&ctx, &account_id, &file, &french_mapper()).await;
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
    let preview = preview(&ctx, &account_id, &file, &english_mapper()).await;
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
                },
                operation: Some(_),
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
                },
                ..
            }
        ),
        "CSH-031: {:?}",
        status(&preview, 5)
    );
    assert!(matches!(
        status(&preview, 6),
        RowStatus::Rejected {
            reason: RowReason::OtherCurrency,
            operation: Some(_),
        }
    ));
    assert!(matches!(
        status(&preview, 7),
        RowStatus::Rejected {
            reason: RowReason::AssetNotHeld,
            ..
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
    let preview_once = preview(&ctx, &account_id, &file, &english_mapper()).await;
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
    let archived = preview(&ctx, &account_id, &file, &english_mapper()).await;
    assert!(
        matches!(status(&archived, 2), RowStatus::Ready { .. }),
        "an archived asset is matched"
    );
    assert!(matches!(
        status(&archived, 4),
        RowStatus::Rejected {
            reason: RowReason::ManualEntry {
                refusal: AccountError::ArchivedAssetSell
            },
            ..
        }
    ));
}

// IMP-033 / IMP-033A — an unknown asset matched once takes its asset; a match made to an asset
// the portfolio lacks is refused, a saved one whose asset has not arrived is passed over.
#[tokio::test]
async fn imp_033_an_unknown_asset_is_matched_once() {
    let ctx = build_ctx().await;
    let (account_id, _, beta) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let mut mapper = french_mapper();
    mapper.asset_matches = vec![AssetMatch {
        read: "inconnue sa".to_string(),
        asset_id: beta.clone(),
    }];
    let matched = preview(&ctx, &account_id, &file, &mapper).await;
    assert!(
        matches!(status(&matched, 8), RowStatus::Ready { asset_id: Some(id), .. } if *id == beta)
    );
    mapper.asset_matches = vec![AssetMatch {
        read: "INCONNUE SA".to_string(),
        asset_id: "not-an-asset".to_string(),
    }];
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &mapper)
        .await;
    assert!(matches!(
        refused,
        Err(CsvImportError::UseCase(CsvImportTask::MatchedAssetNotFound { asset_id })) if asset_id == "not-an-asset"
    ));
    let saved = ctx
        .mappers
        .save_import_mapper(None, mapper.clone())
        .await
        .expect("saved");
    let waiting = ctx
        .import
        .preview_import(&account_id, path(&file), Some(&saved.id), &mapper)
        .await
        .expect("preview");
    assert!(matches!(
        status(&waiting, 8),
        RowStatus::UnknownAsset { .. }
    ));
}

// IMP-010 / IMP-020E / IMP-021 — an unknown account, a file that cannot be read or was not
// picked, a saved mapper no longer held and a mapper without its date column are refused.
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
        .preview_import(&account_id, path(&file), Some("gone"), &english_mapper())
        .await;
    assert!(matches!(
        gone,
        Err(CsvImportError::AccountImport(
            AccountImportError::MapperNotFound { .. }
        ))
    ));
    let mut no_date = english_mapper();
    no_date.columns.date = " ".to_string();
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &no_date)
        .await;
    assert!(matches!(
        refused,
        Err(CsvImportError::AccountImport(
            AccountImportError::MapperColumnMissing
        ))
    ));
    let mut absent = english_mapper();
    absent.columns.fees = header("Fees");
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &absent)
        .await;
    assert!(
        matches!(refused, Err(CsvImportError::AccountImport(AccountImportError::MappedColumnsNotInFile { columns })) if columns == ["Fees"])
    );
    let mut untyped = english_mapper();
    untyped.operation_types.retain(|label| label.label != "Buy");
    let preview = preview(&ctx, &account_id, &file, &untyped).await;
    assert!(matches!(
        status(&preview, 3),
        RowStatus::Rejected { reason: RowReason::Unread { rejection: RowRejection::OperationTypeMissing { label } }, .. }
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
    mapper: &ImportMapperDraft,
    add_deposits: bool,
) -> ImportPreview {
    ctx.import
        .confirm_import(account_id, path(file), None, mapper, add_deposits)
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
    let confirmed = confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
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
    let previewed = preview(&ctx, &account_id, &file, &english_mapper()).await;
    assert!(
        matches!(
            status(&previewed, 2),
            RowStatus::Ready { funding: None, .. }
        ),
        "IMP-038: the day's own deposit funds the purchase"
    );
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
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
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    ctx.asset_service
        .archive_asset(&acme)
        .await
        .expect("archived");
    let again = preview(&ctx, &account_id, &file, &english_mapper()).await;
    assert!(matches!(
        status(&again, 3),
        RowStatus::AlreadyRecorded { .. }
    ));
    let new_sale =
        format!("{HEADER}2024-02-03,Sell,FR0000000010,ACM,Acme World,1,110.00,110.00,0,EUR\n");
    let later = picked(&ctx, &new_sale, false).await;
    let refused = preview(&ctx, &account_id, &later, &english_mapper()).await;
    assert!(
        matches!(
            status(&refused, 2),
            RowStatus::Rejected {
                reason: RowReason::ManualEntry {
                    refusal: AccountError::ArchivedAssetSell
                },
                ..
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
    confirm(&ctx, &account_id, &first, &english_mapper(), true).await;
    let twice = format!(
        "{HEADER}2024-02-01,Deposit,,,,,,50.00,,EUR\n\
         2024-02-01,Deposit,,,,,,50.00,,EUR\n\
         2024-02-02,Withdrawal,,,,,,10.00,,EUR\n"
    );
    let later = picked(&ctx, &twice, false).await;
    let previewed = preview(&ctx, &account_id, &later, &english_mapper()).await;
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
    let previewed = preview(&ctx, &account_id, &file, &english_mapper()).await;
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
                reason: RowReason::AssetNotHeld,
                ..
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
    let previewed = preview(&ctx, &account_id, &file, &english_mapper()).await;
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
    let confirmed = confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
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
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    assert!(!events.drain().contains(&Event::AssetUpdated));
}

/// The file a mapper from elsewhere arrives in.
fn mapper_file(text: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp file");
    file.write_all(text.as_bytes()).expect("written");
    file
}

fn mappers_folder(ctx: &Ctx) -> std::path::PathBuf {
    ctx.mappers_dir.path().join("import-mappers")
}

// IMP-020 / IMP-020A / IMP-025 / IMP-040 — saving writes a JSON file holding what the mapper
// is; the list is what the folder holds, by provider; a change of provider keeps the file;
// deleted, it is gone.
#[tokio::test]
async fn imp_020_a_saved_mapper_is_a_file_of_the_folder() {
    let ctx = build_ctx().await;
    assert!(ctx
        .mappers
        .list_import_mappers()
        .await
        .expect("no folder")
        .is_empty());
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    assert_eq!(saved.id, "broker");
    let file = mappers_folder(&ctx).join("broker.json");
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).expect("the file")).expect("JSON");
    assert_eq!(written["info"]["provider"], "Broker");
    assert_eq!(written["separator"], "Comma");
    assert_eq!(written["columns"]["date"], "Date");
    assert!(written["operation_types"].is_array() && written["asset_matches"].is_array());
    assert!(
        written.get("id").is_none(),
        "its identity is its file's name"
    );

    let mut renamed = english_mapper();
    renamed.info.provider = "Another".to_string();
    let again = ctx
        .mappers
        .save_import_mapper(Some(saved.id.clone()), renamed)
        .await
        .expect("saved again");
    assert_eq!(again.id, "broker");
    ctx.mappers
        .save_import_mapper(None, french_mapper())
        .await
        .expect("second");
    let providers: Vec<String> = ctx
        .mappers
        .list_import_mappers()
        .await
        .expect("listed")
        .into_iter()
        .map(|mapper| mapper.info.provider)
        .collect();
    assert_eq!(providers, ["Another", "Courtier"]);

    ctx.mappers
        .delete_import_mapper("broker")
        .await
        .expect("deleted");
    assert!(!file.exists());
    assert!(matches!(
        ctx.mappers.delete_import_mapper("broker").await,
        Err(AccountImportError::MapperNotFound { .. })
    ));
    assert!(matches!(
        ctx.mappers
            .save_import_mapper(Some("broker".to_string()), english_mapper())
            .await,
        Err(AccountImportError::MapperNotFound { .. })
    ));
}

// IMP-024 / IMP-027 — a provider another mapper holds is refused when set; two files copied
// by hand with one provider are both listed, and either may be saved under it.
#[tokio::test]
async fn imp_024_a_provider_is_held_once_unless_files_were_copied_by_hand() {
    let ctx = build_ctx().await;
    let first = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let mut same = english_mapper();
    same.info.provider = " BROKER ".to_string();
    assert!(matches!(
        ctx.mappers.save_import_mapper(None, same).await,
        Err(AccountImportError::MapperProviderTaken { .. })
    ));
    std::fs::copy(
        mappers_folder(&ctx).join("broker.json"),
        mappers_folder(&ctx).join("copy.json"),
    )
    .expect("copied by hand");
    let listed = ctx.mappers.list_import_mappers().await.expect("listed");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "broker", "same provider, by identity");
    ctx.mappers
        .save_import_mapper(Some(first.id), english_mapper())
        .await
        .expect("its own provider is no conflict");
}

// IMP-020A — what the folder holds that does not read as a mapper is left out and left there.
#[tokio::test]
async fn imp_020a_a_file_that_does_not_read_is_left_out_and_left_there() {
    let ctx = build_ctx().await;
    ctx.mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    std::fs::create_dir(mappers_folder(&ctx).join("folder.json")).expect("a subfolder");
    let bad = mappers_folder(&ctx).join("bad.json");
    std::fs::write(&bad, "not a mapper").expect("written");
    let big = mappers_folder(&ctx).join("big.json");
    std::fs::write(&big, " ".repeat(1_000_001)).expect("written");
    let listed = ctx.mappers.list_import_mappers().await.expect("listed");
    assert_eq!(listed.len(), 1);
    assert!(bad.exists() && big.exists());
}

// IMP-020B — a mapper's file from elsewhere joins the list under a name of its provider, and
// a later file of the same broker reads through it; what does not read is refused with why,
// nothing written.
#[tokio::test]
async fn imp_020b_a_mapper_from_elsewhere_is_added_from_its_file() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let shared = serde_json::to_string(&english_mapper()).expect("JSON");
    let added = ctx
        .mappers
        .add_import_mapper_from(mapper_file(&shared).path())
        .await
        .expect("added");
    assert_eq!(added.id, "broker");
    assert_eq!(added.info.provider, "Broker");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let previewed = ctx
        .import
        .preview_import(&account_id, path(&file), Some(&added.id), &english_mapper())
        .await
        .expect("previewed");
    assert!(!previewed.rows.is_empty());

    assert!(matches!(
        ctx.mappers
            .add_import_mapper_from(mapper_file(&shared).path())
            .await,
        Err(AccountImportError::MapperProviderTaken { .. })
    ));
    let mut with_extra: serde_json::Value = serde_json::from_str(&shared).expect("JSON");
    with_extra["extra"] = serde_json::json!(1);
    for (bad, what) in [
        ("not json".to_string(), "not JSON"),
        (with_extra.to_string(), "a field a mapper does not have"),
        (
            r#"{"info":{"provider":"A"}}"#.to_string(),
            "fields it needs missing",
        ),
    ] {
        match ctx
            .mappers
            .add_import_mapper_from(mapper_file(&bad).path())
            .await
        {
            Err(AccountImportError::MapperFileInvalid { reason }) => {
                assert!(!reason.is_empty(), "{what}")
            }
            other => panic!("{what}: {other:?}"),
        }
    }
    let mut no_column = english_mapper();
    no_column.info.provider = "Other".to_string();
    no_column.columns.date = String::new();
    assert!(matches!(
        ctx.mappers
            .add_import_mapper_from(
                mapper_file(&serde_json::to_string(&no_column).expect("JSON")).path()
            )
            .await,
        Err(AccountImportError::MapperColumnMissing)
    ));
    assert!(matches!(
        ctx.mappers
            .add_import_mapper_from(mapper_file(&" ".repeat(1_000_001)).path())
            .await,
        Err(AccountImportError::MapperFileTooLarge)
    ));
    assert!(matches!(
        ctx.mappers
            .add_import_mapper_from(&mappers_folder(&ctx).join("none.json"))
            .await,
        Err(AccountImportError::MapperFileUnreadable)
    ));
    assert_eq!(
        ctx.mappers
            .list_import_mappers()
            .await
            .expect("listed")
            .len(),
        1
    );
}

// IMP-020C / IMP-020D — a confirmation with a saved mapper remembers it for the file's folder,
// preselected for the next file there; another folder, or a mapper gone, preselects none;
// a preview remembers nothing (IMP-017), a later confirmation replaces it.
#[tokio::test]
async fn imp_020c_the_mapper_chosen_is_remembered_for_its_folder() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let mut other = english_mapper();
    other.info.provider = "Other".to_string();
    let second = ctx
        .mappers
        .save_import_mapper(None, other)
        .await
        .expect("saved");

    let folder = tempfile::tempdir().expect("folder");
    let in_folder = |name: &str| {
        let file = folder.path().join(name);
        std::fs::write(&file, fixture("english_broker.csv")).expect("written");
        file
    };
    let first = in_folder("january.csv");
    let first_path = ctx.import.allow_file(&first).await.expect("picked");
    assert_eq!(
        ctx.import
            .remembered_import_mapper(&first_path)
            .await
            .expect("none"),
        None
    );
    ctx.import
        .preview_import(&account_id, &first_path, Some(&saved.id), &english_mapper())
        .await
        .expect("previewed");
    assert_eq!(
        ctx.import
            .remembered_import_mapper(&first_path)
            .await
            .expect("none"),
        None,
        "a preview writes nothing"
    );
    ctx.import
        .confirm_import(
            &account_id,
            &first_path,
            Some(&saved.id),
            &english_mapper(),
            true,
        )
        .await
        .expect("confirmed");

    let next = in_folder("february.csv");
    let next_path = ctx.import.allow_file(&next).await.expect("picked");
    assert_eq!(
        ctx.import
            .remembered_import_mapper(&next_path)
            .await
            .expect("read"),
        Some(saved.id.clone()),
        "another file of that folder"
    );
    ctx.import
        .confirm_import(
            &account_id,
            &next_path,
            Some(&second.id),
            &english_mapper(),
            true,
        )
        .await
        .expect("confirmed");
    assert_eq!(
        ctx.import
            .remembered_import_mapper(&next_path)
            .await
            .expect("read"),
        Some(second.id.clone()),
        "the later choice replaces it"
    );

    let elsewhere = picked(&ctx, &fixture("english_broker.csv"), false).await;
    assert_eq!(
        ctx.import
            .remembered_import_mapper(path(&elsewhere))
            .await
            .expect("read"),
        None,
        "another folder"
    );
    ctx.import.allow_file(&next).await.expect("picked");
    ctx.mappers
        .delete_import_mapper(&second.id)
        .await
        .expect("deleted");
    assert_eq!(
        ctx.import
            .remembered_import_mapper(&next_path)
            .await
            .expect("read"),
        None,
        "a mapper no longer held"
    );
    assert!(matches!(
        ctx.import.remembered_import_mapper(path(&elsewhere)).await,
        Err(CsvImportError::UseCase(CsvImportTask::FileNotPicked))
    ));
}

// IMP-020 / IMP-020B — a new mapper never takes another's file: one renamed keeps its file
// name, and a file of its old provider takes a name of its own. A provider is written trimmed.
#[tokio::test]
async fn imp_020_a_new_mapper_never_replaces_a_file() {
    let ctx = build_ctx().await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let mut renamed = english_mapper();
    renamed.info.provider = "Another".to_string();
    ctx.mappers
        .save_import_mapper(Some(saved.id), renamed)
        .await
        .expect("renamed");

    let mut shared = english_mapper();
    shared.info.provider = "  Broker ".to_string();
    let added = ctx
        .mappers
        .add_import_mapper_from(mapper_file(&serde_json::to_string(&shared).expect("JSON")).path())
        .await
        .expect("added");
    assert_eq!(added.id, "broker-2");
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(mappers_folder(&ctx).join(name)).expect("file"))
            .expect("JSON")
    };
    assert_eq!(read("broker.json")["info"]["provider"], "Another");
    assert_eq!(read("broker-2.json")["info"]["provider"], "Broker");
}

// IMP-020 / IMP-020A / IMP-040 — a folder that cannot be read or written is told, not hidden.
#[tokio::test]
async fn imp_020_a_folder_that_fails_is_rejected() {
    let ctx = build_ctx().await;
    std::fs::write(mappers_folder(&ctx), "a file where the folder goes").expect("written");
    assert!(matches!(
        ctx.mappers.list_import_mappers().await,
        Err(AccountImportError::MapperFolderFailed)
    ));
    assert!(matches!(
        ctx.mappers.save_import_mapper(None, english_mapper()).await,
        Err(AccountImportError::MapperFolderFailed)
    ));
    assert!(matches!(
        ctx.mappers.delete_import_mapper("broker").await,
        Err(AccountImportError::MapperFolderFailed)
    ));
}

// IMP-020B — a folder is not a mapper's file.
#[tokio::test]
async fn imp_020b_a_folder_picked_is_refused() {
    let ctx = build_ctx().await;
    let dir = tempfile::tempdir().expect("dir");
    assert!(matches!(
        ctx.mappers.add_import_mapper_from(dir.path()).await,
        Err(AccountImportError::MapperFileUnreadable)
    ));
}

// IMP-020E / IMP-025 — a held file that no longer reads is refused when picked or saved over.
#[tokio::test]
async fn imp_020e_a_mapper_that_no_longer_reads_is_refused() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    std::fs::write(mappers_folder(&ctx).join("broker.json"), "broken").expect("written");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    assert!(matches!(
        ctx.import
            .preview_import(&account_id, path(&file), Some(&saved.id), &english_mapper())
            .await,
        Err(CsvImportError::AccountImport(
            AccountImportError::MapperFileInvalid { .. }
        ))
    ));
    assert!(matches!(
        ctx.mappers
            .save_import_mapper(Some(saved.id), english_mapper())
            .await,
        Err(AccountImportError::MapperFileInvalid { .. })
    ));
}

// IMP-033A — of two matches for one text, the first whose asset is held applies.
#[tokio::test]
async fn imp_033a_the_first_match_whose_asset_is_held_applies() {
    let ctx = build_ctx().await;
    let (account_id, _, beta) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let mut mapper = french_mapper();
    mapper.asset_matches = vec![
        AssetMatch {
            read: "inconnue sa".to_string(),
            asset_id: "not-an-asset".to_string(),
        },
        AssetMatch {
            read: "inconnue sa".to_string(),
            asset_id: beta.clone(),
        },
    ];
    let saved = ctx
        .mappers
        .save_import_mapper(None, mapper.clone())
        .await
        .expect("saved");
    let preview = ctx
        .import
        .preview_import(&account_id, path(&file), Some(&saved.id), &mapper)
        .await
        .expect("preview");
    assert!(
        matches!(status(&preview, 8), RowStatus::Ready { asset_id: Some(id), .. } if *id == beta)
    );
}

// IMP-022 — a mapper's file giving one label two types reads with the first.
#[tokio::test]
async fn imp_022_a_label_given_two_types_reads_with_the_first() {
    let ctx = build_ctx().await;
    let (account_id, _, _) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let mut mapper = french_mapper();
    let before = preview(&ctx, &account_id, &file, &mapper).await;
    let mut first = mapper.operation_types[0].clone();
    first.transaction_type = OperationType::NotImported;
    mapper.operation_types.insert(0, first);
    let after = preview(&ctx, &account_id, &file, &mapper).await;
    assert!(after.counts.not_imported > before.counts.not_imported);
}

// IMP-020D — a memory that cannot be read preselects none.
#[tokio::test]
async fn imp_020d_a_memory_that_cannot_be_read_preselects_none() {
    let ctx = build_ctx().await;
    sqlx::query("DROP TABLE import_mapper_choices")
        .execute(&ctx.pool)
        .await
        .expect("dropped");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    assert_eq!(
        ctx.import
            .remembered_import_mapper(path(&file))
            .await
            .expect("read"),
        None
    );
}

// IMP-021 — every column the file lacks is named, each once.
#[tokio::test]
async fn imp_021_every_column_the_file_lacks_is_named() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let mut absent = english_mapper();
    absent.columns.fees = header("Charges");
    absent.columns.quantity = header("Qté");
    absent.columns.currency = header("Qté");
    let refused = ctx
        .import
        .preview_import(&account_id, path(&file), None, &absent)
        .await;
    assert!(
        matches!(refused, Err(CsvImportError::AccountImport(AccountImportError::MappedColumnsNotInFile { columns })) if columns == ["Qté", "Charges"])
    );
}

// IMP-020A — the list sorts by provider, case and accents ignored.
#[tokio::test]
async fn imp_020a_the_list_sorts_case_and_accents_ignored() {
    let ctx = build_ctx().await;
    for provider in ["zeta", "Élan", "alpha"] {
        let mut mapper = english_mapper();
        mapper.info.provider = provider.to_string();
        ctx.mappers
            .save_import_mapper(None, mapper)
            .await
            .expect("saved");
    }
    let providers: Vec<String> = ctx
        .mappers
        .list_import_mappers()
        .await
        .expect("listed")
        .into_iter()
        .map(|mapper| mapper.info.provider)
        .collect();
    assert_eq!(providers, ["alpha", "Élan", "zeta"]);
}

// IMP-020B / IMP-022 / IMP-033 — a file's operation types and asset matches are kept, whether
// or not the asset is held.
#[tokio::test]
async fn imp_020b_operation_types_and_matches_are_kept() {
    let ctx = build_ctx().await;
    let mut shared = english_mapper();
    shared.asset_matches = vec![AssetMatch {
        read: "Somewhere".to_string(),
        asset_id: "not-here".to_string(),
    }];
    assert!(!shared.operation_types.is_empty());
    let text = serde_json::to_string(&shared).expect("JSON");
    let added = ctx
        .mappers
        .add_import_mapper_from(mapper_file(&text).path())
        .await
        .expect("added");
    let read = ctx
        .mappers
        .get_import_mapper(&added.id)
        .await
        .expect("read");
    assert_eq!(read.asset_matches, shared.asset_matches);
    assert_eq!(read.operation_types, shared.operation_types);
}

// IMP-024 / IMP-025 — a change of provider onto one another mapper holds is refused, the file
// unchanged.
#[tokio::test]
async fn imp_025_a_provider_another_holds_is_refused() {
    let ctx = build_ctx().await;
    ctx.mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let second = ctx
        .mappers
        .save_import_mapper(None, french_mapper())
        .await
        .expect("saved");
    let mut taken = french_mapper();
    taken.info.provider = "broker".to_string();
    assert!(matches!(
        ctx.mappers
            .save_import_mapper(Some(second.id.clone()), taken)
            .await,
        Err(AccountImportError::MapperProviderTaken { .. })
    ));
    let read = ctx
        .mappers
        .get_import_mapper(&second.id)
        .await
        .expect("read");
    assert_eq!(read.info.provider, french_mapper().info.provider);
}

// IMP-040 — deleting a mapper changes nothing already recorded.
#[tokio::test]
async fn imp_040_a_deleted_mapper_leaves_the_records() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    let before = transactions(&ctx, &account_id).await.len();
    assert!(before > 0);
    ctx.mappers
        .delete_import_mapper(&saved.id)
        .await
        .expect("deleted");
    assert_eq!(transactions(&ctx, &account_id).await.len(), before);
}

// IMP-020C — a confirmation with no saved mapper remembers nothing.
#[tokio::test]
async fn imp_020c_no_saved_mapper_nothing_remembered() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    let remembered: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM import_mapper_choices")
        .fetch_one(&ctx.pool)
        .await
        .expect("counted");
    assert_eq!(remembered, 0);
}

// IMP-020D — a mappers folder that cannot be read preselects none.
#[tokio::test]
async fn imp_020d_a_mappers_folder_that_cannot_be_read_preselects_none() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    ctx.import
        .confirm_import(
            &account_id,
            path(&file),
            Some(&saved.id),
            &english_mapper(),
            true,
        )
        .await
        .expect("confirmed");
    std::fs::remove_dir_all(mappers_folder(&ctx)).expect("removed");
    std::fs::write(mappers_folder(&ctx), "a file where the folder goes").expect("written");
    assert_eq!(
        ctx.import
            .remembered_import_mapper(path(&file))
            .await
            .expect("read"),
        None
    );
}

// IMP-020C — an import stands when remembering fails.
#[tokio::test]
async fn imp_020c_an_import_stands_when_remembering_fails() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    sqlx::query("DROP TABLE import_mapper_choices")
        .execute(&ctx.pool)
        .await
        .expect("dropped");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    let preview = ctx
        .import
        .confirm_import(
            &account_id,
            path(&file),
            Some(&saved.id),
            &english_mapper(),
            true,
        )
        .await
        .expect("the import stands");
    assert!(preview.counts.ready > 0);
}

// IMP-020D — a remembered mapper whose file no longer reads, for any reason, preselects none.
#[tokio::test]
async fn imp_020d_a_mapper_that_no_longer_reads_preselects_none() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let saved = ctx
        .mappers
        .save_import_mapper(None, english_mapper())
        .await
        .expect("saved");
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    ctx.import
        .confirm_import(
            &account_id,
            path(&file),
            Some(&saved.id),
            &english_mapper(),
            true,
        )
        .await
        .expect("confirmed");
    let mut no_date = english_mapper();
    no_date.columns.date = String::new();
    for broken in [
        "not a mapper".to_string(),
        " ".repeat(1_000_001),
        serde_json::to_string(&no_date).expect("JSON"),
    ] {
        std::fs::write(mappers_folder(&ctx).join("broker.json"), broken).expect("written");
        assert_eq!(
            ctx.import
                .remembered_import_mapper(path(&file))
                .await
                .expect("read"),
            None
        );
    }
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
        .preview_import(&account_id, path(&earlier), None, &english_mapper())
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
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    let rows = format!(
        "{HEADER}2099-01-01,Deposit,,,,,,10.00,,EUR\n\
         2024-02-01,Sell,FR0000000002,BTP,Beta Power,1,100.00,100.00,0,EUR\n\
         2024-03-01,Sell,FR0000000010,ACM,Acme World,1,100.00,100.00,0,EUR\n\
         2024-02-02,Buy,US0378331005,USF,Dollar Fund,1,100.00,100.00,0,EUR\n"
    );
    let later = picked(&ctx, &rows, false).await;
    let previewed = preview(&ctx, &account_id, &later, &english_mapper()).await;
    for (line, what) in [
        (2, "TRX-020: a future date"),
        (3, "SEL-012: nothing held"),
        (4, "SEL-032: a later sale left short"),
    ] {
        assert!(
            matches!(
                status(&previewed, line),
                RowStatus::Rejected {
                    reason: RowReason::ManualEntry { .. },
                    ..
                }
            ),
            "{what}: {:?}",
            status(&previewed, line)
        );
    }
    assert!(matches!(
        status(&previewed, 5),
        RowStatus::Rejected {
            reason: RowReason::OtherCurrency,
            ..
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
    confirm(&ctx, &account_id, &first, &english_mapper(), true).await;
    let deposit = format!("{HEADER}2024-02-01,Deposit,,,,,,201.00,,EUR\n");
    let later = picked(&ctx, &deposit, false).await;
    let confirmed = confirm(&ctx, &account_id, &later, &english_mapper(), true).await;
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
        .confirm_import(&account_id, path(&file), None, &english_mapper(), true)
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
    let confirmed = confirm(&ctx, &account_id, &file, &english_mapper(), false).await;
    assert!(matches!(
        status(&confirmed, 8),
        RowStatus::Rejected {
            reason: RowReason::ManualEntry {
                refusal: AccountError::InsufficientCash { .. }
            },
            ..
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
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    let once = transactions(&ctx, &account_id).await;
    let mut events = ctx.bus.subscribe();
    let again = confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
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
    let first = confirm(&french, &account_id, &file, &french_mapper(), true).await;
    let again = confirm(&french, &account_id, &file, &french_mapper(), true).await;
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
    let preview = preview(&ctx, &account_id, &file, &french_mapper()).await;
    let RowStatus::UnknownAsset { asset: read, .. } = status(&preview, 8) else {
        panic!("line 8 is unknown");
    };
    let name = read.name.clone().expect("the name read");
    let created = asset(&ctx, &name, "INC", None, "EUR").await;
    let confirmed = confirm(&ctx, &account_id, &file, &french_mapper(), true).await;
    assert!(
        matches!(status(&confirmed, 8), RowStatus::Ready { asset_id: Some(id), .. } if *id == created)
    );
    assert!(transactions(&ctx, &account_id)
        .await
        .iter()
        .any(|t| t.asset_id == created && t.transaction_type == TransactionType::Purchase));
}

// IMP-052A — a created asset takes its rows through a match, whatever its name; in another
// currency, they are rejected through that match.
#[tokio::test]
async fn imp_052a_a_created_asset_takes_its_rows_through_a_match() {
    let ctx = build_ctx().await;
    let (account_id, _, _) = french_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("french_broker.csv"), true).await;
    let mut mapper = french_mapper();
    let created = asset(&ctx, "Renamed by the user", "RNM", None, "EUR").await;
    mapper.asset_matches = vec![AssetMatch {
        read: "INCONNUE SA".to_string(),
        asset_id: created.clone(),
    }];
    let matched = preview(&ctx, &account_id, &file, &mapper).await;
    assert!(
        matches!(status(&matched, 8), RowStatus::Ready { asset_id: Some(id), .. } if *id == created)
    );
    assert!(matched.unknown_assets.is_empty());
    let dollars = asset(&ctx, "Other currency", "USDO", None, "USD").await;
    mapper.asset_matches = vec![AssetMatch {
        read: "INCONNUE SA".to_string(),
        asset_id: dollars,
    }];
    let rejected = preview(&ctx, &account_id, &file, &mapper).await;
    assert!(
        matches!(status(&rejected, 8), RowStatus::Rejected { .. }),
        "{:?}",
        status(&rejected, 8)
    );
}

const JUDGED: &str = "Date,Type,ISIN,Ticker,Name,Quantity,Price,Amount,Commission,Currency
2025-01-06,Deposit,,,,,,500,,EUR
2025-01-07,Buy,FR0000000010,ACM,Acme World,10,100,,0,EUR
2025-01-08,Dividend,FR0000000010,ACM,Acme World,,,25,,EUR
2025-01-09,Withdrawal,,,,,,10,,EUR
2025-01-10,Sell,FR0000000010,ACM,Acme World,99,10,,0,EUR
2025-01-11,Buy,FR0000000099,ZZZ,Societe Nouvelle,1,10,,0,EUR
2025-01-12,Buy,FR0000000099,,Another Name,1,10,,0,EUR
2025-01-13,Buy,,ZZZ2,Societe Nouvelle,1,10,,0,EUR
2025-01-14,Buy,,,societe nouvelle,1,10,,0,EUR
2025-01-15,Buy,,,SOCIÉTÉ NOUVELLE,1,10,,0,EUR
2025-01-16,Buy,,,,1,10,,0,EUR
";

// IMP-030 / IMP-038 / IMP-067 / IMP-068 — what the preview adds up for the window: each row's
// amount, the deposits needed, and the unknown assets each with its rows under its first read.
#[tokio::test]
async fn imp_067_the_preview_groups_unknown_assets_and_sums_the_deposits() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, JUDGED, false).await;
    let judged = preview(&ctx, &account_id, &file, &english_mapper()).await;
    let amount = |line: u32| {
        judged
            .rows
            .iter()
            .find(|row| row.line == line)
            .and_then(|row| row.amount)
    };
    assert_eq!(amount(2), Some(micro(500)), "a deposit");
    assert_eq!(amount(4), Some(micro(25)), "a dividend");
    assert_eq!(amount(5), Some(micro(10)), "a withdrawal");
    assert!(
        matches!(status(&judged, 6), RowStatus::Rejected { .. }),
        "{:?}",
        status(&judged, 6)
    );
    assert_eq!(
        amount(6),
        Some(micro(990)),
        "a rejected sale keeps its total"
    );

    assert_eq!(judged.deposits.count, 1, "{:?}", status(&judged, 3));
    assert_eq!(judged.deposits.total, micro(500));
    assert_eq!(
        judged.deposits.ready_without,
        judged.counts.ready - 1,
        "without the deposit, one row less"
    );

    let groups: Vec<(&str, &[u32])> = judged
        .unknown_assets
        .iter()
        .map(|group| (group.read.as_str(), group.lines.as_slice()))
        .collect();
    assert_eq!(
        groups,
        vec![
            ("FR0000000099", &[7, 8][..]),
            ("ZZZ2", &[9][..]),
            ("societe nouvelle", &[10, 11][..]),
        ],
        "the ISIN, else the reference, else the name, accents and case aside; no group for line 12"
    );

    // IMP-033 — a match is compared against what each row read, accents and case aside.
    let mut mapper = english_mapper();
    let acme = judged.matchable_assets[0].id.clone();
    mapper.asset_matches = vec![
        AssetMatch {
            read: "fr0000000099".to_string(),
            asset_id: acme.clone(),
        },
        AssetMatch {
            read: "zzz2".to_string(),
            asset_id: acme.clone(),
        },
        AssetMatch {
            read: "Société Nouvelle".to_string(),
            asset_id: acme,
        },
    ];
    let matched = preview(&ctx, &account_id, &file, &mapper).await;
    for line in [7, 8, 9, 10, 11] {
        assert!(
            !matches!(status(&matched, line), RowStatus::UnknownAsset { .. }),
            "line {line}"
        );
    }
}

// IMP-032 / IMP-067 — an unknown asset can be matched to an archived asset, never to a Cash
// Asset; the choices come by name.
#[tokio::test]
async fn imp_067_the_assets_to_match_hold_the_archived_and_no_cash() {
    let ctx = build_ctx().await;
    let beta = asset(&ctx, "beta", "BET", None, "EUR").await;
    asset(&ctx, "Alpha", "ALP", None, "EUR").await;
    ctx.asset_service
        .archive_asset(&beta)
        .await
        .expect("archived");
    let account_id = account(&ctx, micro(100)).await;
    let file = picked(&ctx, "Date,Type,ISIN,Ticker,Name,Quantity,Price,Amount,Commission,Currency\n2025-01-06,Deposit,,,,,,500,,EUR\n", false).await;
    let judged = preview(&ctx, &account_id, &file, &english_mapper()).await;
    let names: Vec<&str> = judged
        .matchable_assets
        .iter()
        .map(|asset| asset.name.as_str())
        .collect();
    assert_eq!(names, vec!["Alpha", "beta"]);
    assert!(
        ctx.asset_service
            .get_all_assets()
            .await
            .expect("assets")
            .iter()
            .any(|asset| asset.class == AssetClass::Cash),
        "the account's Cash Asset exists and is left out"
    );
}

// IMP-053 / IMP-066 — a file whose every row is recorded says so.
#[tokio::test]
async fn imp_066_a_file_recorded_whole_is_all_recorded() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let text = "Date,Type,ISIN,Ticker,Name,Quantity,Price,Amount,Commission,Currency\n2025-01-06,Deposit,,,,,,500,,EUR\n";
    let file = picked(&ctx, text, false).await;
    let first = preview(&ctx, &account_id, &file, &english_mapper()).await;
    assert!(!first.all_recorded);
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
    let again = preview(&ctx, &account_id, &file, &english_mapper()).await;
    assert!(again.all_recorded, "{:?}", again.counts);
}

// IMP-051A — a corrected added deposit stays marked as added.
#[tokio::test]
async fn imp_051a_a_corrected_added_deposit_stays_added() {
    let ctx = build_ctx().await;
    let account_id = english_portfolio(&ctx).await;
    let file = picked(&ctx, &fixture("english_broker.csv"), false).await;
    confirm(&ctx, &account_id, &file, &english_mapper(), true).await;
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
