//! Integration tests for the data export (EXP spec): a portfolio written as CSV or JSON, and
//! an account's file read back through the account import into an empty portfolio.
//!
//! Exercises the full stack through the public `folioneer_lib` API — `DataExportUseCase`,
//! `CsvImportUseCase`, the account and asset services — over real in-memory SQLite. No mocks
//! (test-rules.md Tier 3). The portfolio is invented data.

use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
};
use folioneer_lib::context::account_import::{
    AccountImportService, FsImportMapperStore, ImportMapperDraft, SqliteMapperChoiceRepository,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::cash::is_cash_asset;
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::shared::domain::quantity::from_surface;
use folioneer_lib::shared::infrastructure::uow::SqliteTransactionManager;
use folioneer_lib::use_cases::csv_import::CsvImportUseCase;
use folioneer_lib::use_cases::data_export::{DataExportError, DataExportUseCase, ExportFormat};
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use std::path::Path;
use std::sync::Arc;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

/// One portfolio over its own in-memory database.
struct Portfolio {
    accounts: Arc<AccountService>,
    assets: Arc<AssetService>,
    recording: HoldingTransactionUseCase,
    import: CsvImportUseCase,
    export: DataExportUseCase,
    currencies: Arc<CurrencyService>,
    _mappers: tempfile::TempDir,
}

async fn portfolio() -> Portfolio {
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
    let accounts = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let assets = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let mappers_dir = tempfile::tempdir().expect("mappers folder");
    let mappers = Arc::new(AccountImportService::new(
        Box::new(FsImportMapperStore::new(mappers_dir.path().join("m"))),
        Box::new(SqliteMapperChoiceRepository::new(pool.clone())),
    ));
    let currencies = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    Portfolio {
        recording: HoldingTransactionUseCase::new(accounts.clone(), assets.clone()),
        import: CsvImportUseCase::new(
            mappers,
            accounts.clone(),
            assets.clone(),
            Arc::new(SqliteTransactionManager::new(pool.clone())),
        ),
        export: DataExportUseCase::new(accounts.clone(), assets.clone(), currencies.clone()),
        currencies,
        accounts,
        assets,
        _mappers: mappers_dir,
    }
}

async fn asset(p: &Portfolio, name: &str, reference: &str, isin: &str) -> String {
    p.assets
        .create_asset(CreateAssetDTO {
            kind: None,
            name: name.to_string(),
            reference: reference.to_string(),
            isin: Some(isin.to_string()),
            class: AssetClass::Stocks,
            currency: "EUR".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("asset")
        .id
}

async fn account(p: &Portfolio, name: &str) -> String {
    p.accounts
        .create(name.to_string(), String::new(), "EUR".to_string(), false)
        .await
        .expect("account")
        .id
}

async fn buy(
    p: &Portfolio,
    account: &str,
    asset: &str,
    date: &str,
    quantity: i64,
    price: i64,
    fees: i64,
) {
    p.recording
        .buy_holding(
            account,
            asset.to_string(),
            date.to_string(),
            from_surface(micro(quantity)),
            micro(price),
            micro(1),
            fees,
            None,
            None,
        )
        .await
        .expect("buy");
}

/// The source portfolio: two accounts of the five types the import reads, and a price.
async fn source() -> (Portfolio, String, String, String) {
    let p = portfolio().await;
    let acme = asset(&p, "Acme World", "ACM", "FR0000000010").await;
    let beta = asset(&p, "Beta Power", "BTP", "FR0000000002").await;
    let pea = account(&p, "PEA").await;
    let cto = account(&p, "CTO").await;
    let r = &p.recording;
    r.record_deposit(
        &pea,
        "2024-01-02".into(),
        micro(10_000),
        Some("first, \"main\"".into()),
    )
    .await
    .expect("deposit");
    buy(&p, &pea, &acme, "2024-02-01", 10, 100, 2_500_000).await;
    buy(&p, &pea, &beta, "2024-02-15", 5, 50, 0).await;
    r.sell_holding(
        &pea,
        acme.clone(),
        "2024-03-01".into(),
        from_surface(micro(4)),
        micro(120),
        micro(1),
        1_000_000,
        None,
        None,
    )
    .await
    .expect("sell");
    r.record_dividend(
        &pea,
        acme.clone(),
        "2024-04-01".into(),
        micro(30),
        micro(1),
        None,
    )
    .await
    .expect("dividend");
    r.record_withdrawal(&pea, "2024-05-01".into(), micro(500), None)
        .await
        .expect("withdrawal");
    r.record_deposit(&cto, "2024-01-05".into(), micro(1_000), None)
        .await
        .expect("deposit");
    buy(&p, &cto, &beta, "2024-01-10", 3, 40, 0).await;
    p.assets
        .record_asset_price(&acme, "2024-03-01", 120.0)
        .await
        .expect("price");
    (p, pea, acme, beta)
}

/// What an account holds: each holding's quantity, average price and realized P&L, by
/// asset reference, the cash included.
async fn figures(p: &Portfolio, account_id: &str) -> Vec<(String, i64, i64, i64)> {
    let mut rows = Vec::new();
    for holding in p
        .accounts
        .get_holdings_for_account(account_id)
        .await
        .expect("holdings")
    {
        let reference = if is_cash_asset(&holding.asset_id) {
            "cash".to_string()
        } else {
            p.assets
                .get_asset_by_id(&holding.asset_id)
                .await
                .expect("asset")
                .expect("held asset")
                .reference
        };
        rows.push((
            reference,
            holding.quantity,
            holding.average_price,
            holding.total_realized_pnl,
        ));
    }
    rows.sort();
    rows
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("file")
}

// EXP-011 / EXP-050 / EXP-051 — the CSV folder holds the four tables, its format, one file
// per account and the mapper; an account's file, read through that mapper into an empty
// portfolio holding the same assets, gives the same holdings, cash and realized P&L.
#[tokio::test]
async fn exp_050_an_account_comes_back_through_the_import() {
    let (original, pea, _, _) = source().await;
    let out = tempfile::tempdir().expect("folder");
    let report = original
        .export
        .export(ExportFormat::Csv, out.path())
        .await
        .expect("exported");
    assert_eq!(
        (
            report.accounts,
            report.assets,
            report.transactions,
            report.prices
        ),
        (2, 2, 8, 1)
    );
    let folder = Path::new(&report.written_to);
    for file in [
        "accounts.csv",
        "assets.csv",
        "transactions.csv",
        "prices.csv",
        "format.json",
        "transactions-PEA.csv",
        "transactions-CTO.csv",
        "import-mapper.json",
    ] {
        assert!(folder.join(file).is_file(), "{file}");
    }
    // EXP-030 — the note keeps its comma and its quotes, quoted as the standard writes it.
    assert!(read(&folder.join("transactions.csv")).contains("\"first, \"\"main\"\"\""));

    let restored = portfolio().await;
    asset(&restored, "Acme World", "ACM", "FR0000000010").await;
    asset(&restored, "Beta Power", "BTP", "FR0000000002").await;
    let target = account(&restored, "PEA").await;
    let mapper = ImportMapperDraft::from_file(read(&folder.join("import-mapper.json")).as_bytes())
        .expect("mapper");
    let shown = restored
        .import
        .allow_file(&folder.join("transactions-PEA.csv"))
        .await
        .expect("picked");
    restored
        .import
        .confirm_import(&target, &shown, None, &mapper, false)
        .await
        .expect("imported");

    assert_eq!(
        figures(&restored, &target).await,
        figures(&original, &pea).await
    );
}

// EXP-012 / EXP-032 — the JSON file holds the four tables, its header first, a figure as
// text, a flag as a boolean.
#[tokio::test]
async fn exp_032_the_json_file_holds_every_table() {
    let (original, _, _, _) = source().await;
    let out = tempfile::tempdir().expect("folder");
    let report = original
        .export
        .export(ExportFormat::Json, out.path())
        .await
        .expect("exported");
    assert!(report.written_to.ends_with(".json"));
    let value: serde_json::Value =
        serde_json::from_str(&read(Path::new(&report.written_to))).expect("JSON");
    assert_eq!(value["format"], "folioneer-export");
    assert_eq!(value["version"], 2);
    assert_eq!(value["accounts"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["transactions"].as_array().map(Vec::len), Some(8));
    let deposit = &value["transactions"][0];
    assert_eq!(deposit["account_name"], "CTO");
    assert_eq!(deposit["type"], "Deposit");
    assert_eq!(deposit["total_amount"], "1000");
    assert_eq!(deposit["asset_id"], serde_json::Value::Null);
    assert_eq!(deposit["added_deposit"], false);
}

// EXP-014 — a second export of the day takes " (2)": nothing is overwritten.
#[tokio::test]
async fn exp_014_a_second_export_takes_the_next_name() {
    let (original, _, _, _) = source().await;
    let out = tempfile::tempdir().expect("folder");
    let first = original
        .export
        .export(ExportFormat::Json, out.path())
        .await
        .expect("first");
    let second = original
        .export
        .export(ExportFormat::Json, out.path())
        .await
        .expect("second");
    assert_ne!(first.written_to, second.written_to);
    assert!(
        second.written_to.ends_with(" (2).json"),
        "{}",
        second.written_to
    );
}

// EXP-015 — a folder that is not there is refused; one that cannot be written leaves nothing.
#[tokio::test]
async fn exp_015_a_failed_export_leaves_nothing() {
    let (original, _, _, _) = source().await;
    let out = tempfile::tempdir().expect("folder");
    let missing = original
        .export
        .export(ExportFormat::Csv, &out.path().join("missing"))
        .await
        .unwrap_err();
    assert_eq!(missing, DataExportError::FolderUnavailable);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out.path(), std::fs::Permissions::from_mode(0o555))
            .expect("read only");
        let refused = original.export.export(ExportFormat::Csv, out.path()).await;
        std::fs::set_permissions(out.path(), std::fs::Permissions::from_mode(0o755))
            .expect("writable again");
        assert!(matches!(refused, Err(DataExportError::WriteFailed)));
        assert_eq!(std::fs::read_dir(out.path()).expect("listed").count(), 0);
    }
}

// EXP-013 / EXP-020 / EXP-031 — each type fills its own figures as written in the file; a
// dividend names the currency it was entered in; an archived asset and its price are in it;
// the CSV headers are the documented columns; the name carries today's date.
#[tokio::test]
async fn exp_031_each_type_writes_its_own_figures() {
    use folioneer_lib::context::account::WriteOffReason;
    let p = portfolio().await;
    let acme = asset(&p, "Acme World", "ACM", "FR0000000010").await;
    let globex = p
        .assets
        .create_asset(CreateAssetDTO {
            kind: None,
            name: "Globex".into(),
            reference: "GBX".into(),
            isin: Some("US0378331005".into()),
            class: AssetClass::Stocks,
            currency: "USD".into(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.into(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("asset")
        .id;
    let old = asset(&p, "Old Co", "OLD", "FR0000120073").await;
    p.assets
        .record_asset_price(&old, "2024-01-02", 5.0)
        .await
        .expect("price");
    p.assets.archive_asset(&old).await.expect("archived");
    let main = account(&p, "Main").await;
    let r = &p.recording;
    r.record_deposit(&main, "2024-01-02".into(), micro(10_000), None)
        .await
        .expect("deposit");
    r.open_holding(
        &main,
        acme.clone(),
        "2024-01-03".into(),
        from_surface(micro(10)),
        micro(1_000),
    )
    .await
    .expect("opening balance");
    r.record_split(&main, acme.clone(), "2024-02-01".into(), micro(2), None)
        .await
        .expect("split");
    r.record_write_off(
        &main,
        acme.clone(),
        "2024-03-01".into(),
        from_surface(micro(1)),
        WriteOffReason::NetworkFee,
        None,
    )
    .await
    .expect("write-off");
    r.buy_holding(
        &main,
        globex.clone(),
        "2024-04-01".into(),
        from_surface(micro(2)),
        micro(100),
        900_000,
        0,
        None,
        None,
    )
    .await
    .expect("buy");
    r.record_dividend(
        &main,
        globex.clone(),
        "2024-05-01".into(),
        micro(10),
        900_000,
        None,
    )
    .await
    .expect("dividend in the asset's currency");
    r.record_dividend(
        &main,
        globex.clone(),
        "2024-06-01".into(),
        micro(9),
        micro(1),
        None,
    )
    .await
    .expect("dividend in the account's currency");

    let out = tempfile::tempdir().expect("folder");
    let report = p
        .export
        .export(ExportFormat::Json, out.path())
        .await
        .expect("exported");
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    assert!(report
        .written_to
        .ends_with(&format!("folioneer-export-{today}.json")));
    let value: serde_json::Value =
        serde_json::from_str(&read(Path::new(&report.written_to))).expect("JSON");
    let rows = value["transactions"].as_array().expect("rows");
    let of = |kind: &str| {
        rows.iter()
            .filter(|row| row["type"] == kind)
            .collect::<Vec<_>>()
    };
    let opening = of("OpeningBalance")[0];
    assert_eq!(
        (
            &opening["quantity"],
            &opening["unit_price"],
            &opening["total_amount"],
            &opening["fees"]
        ),
        (
            &"10".into(),
            &"100".into(),
            &"1000".into(),
            &serde_json::Value::Null
        )
    );
    let split = of("Split")[0];
    assert_eq!(
        (&split["quantity"], &split["split_factor"]),
        (&"0".into(), &"2".into())
    );
    let write_off = of("WriteOff")[0];
    assert_eq!(write_off["write_off_reason"], "NetworkFee");
    assert!(write_off["realized_pnl"].is_string());
    // A write-off has no proceeds: its cost is minus its realized P&L, not a total.
    assert!(write_off["total_amount"].is_null());
    let dividends = of("Dividend");
    assert_eq!(
        (
            &dividends[0]["dividend_currency"],
            &dividends[0]["dividend_amount"]
        ),
        (&"USD".into(), &"10".into())
    );
    assert_eq!(dividends[1]["dividend_currency"], "EUR");
    let assets = value["assets"].as_array().expect("assets");
    let archived = assets
        .iter()
        .find(|a| a["reference"] == "OLD")
        .expect("archived asset");
    assert_eq!(archived["archived"], true);
    assert_eq!(archived["risk_level"], 3);
    assert!(value["prices"]
        .as_array()
        .expect("prices")
        .iter()
        .any(|price| price["asset_reference"] == "OLD" && price["price"] == "5"));

    let csv = p
        .export
        .export(ExportFormat::Csv, out.path())
        .await
        .expect("exported");
    let header = |file: &str| {
        read(&Path::new(&csv.written_to).join(file))
            .lines()
            .next()
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        header("accounts.csv"),
        "id,name,bank,currency,country,wrapper,management_fees_enabled,cost_basis_method"
    );
    assert_eq!(
        header("prices.csv"),
        "asset_id,asset_reference,date,price,currency,source"
    );
    assert!(header("transactions.csv")
        .starts_with("id,account_id,account_name,account_currency,date,type,"));
}

// EXP-027 / EXP-033 — every recorded exchange rate is in the export, by its currencies then
// date, with its source; the format is version 2.
#[tokio::test]
async fn exp_027_the_exchange_rates_are_exported() {
    let p = portfolio().await;
    for (from, date, rate) in [
        ("USD", "2024-02-01", 900_000),
        ("USD", "2024-01-01", 910_000),
        ("GBP", "2024-01-01", 1_150_000),
    ] {
        p.currencies
            .record_currency_rate(from.into(), "EUR".into(), date.into(), rate)
            .await
            .expect("rate");
    }
    let out = tempfile::tempdir().expect("folder");
    let report = p
        .export
        .export(ExportFormat::Json, out.path())
        .await
        .expect("exported");
    let value: serde_json::Value =
        serde_json::from_str(&read(Path::new(&report.written_to))).expect("JSON");
    assert_eq!(value["version"], 2);
    let rates: Vec<(String, String, String, String)> = value["rates"]
        .as_array()
        .expect("rates")
        .iter()
        .map(|r| {
            (
                r["from_currency"].as_str().unwrap_or_default().to_string(),
                r["date"].as_str().unwrap_or_default().to_string(),
                r["rate"].as_str().unwrap_or_default().to_string(),
                r["source"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        rates,
        [
            (
                "GBP".into(),
                "2024-01-01".into(),
                "1.15".into(),
                "Manual".into()
            ),
            (
                "USD".into(),
                "2024-01-01".into(),
                "0.91".into(),
                "Manual".into()
            ),
            (
                "USD".into(),
                "2024-02-01".into(),
                "0.9".into(),
                "Manual".into()
            ),
        ]
    );
    let csv = p
        .export
        .export(ExportFormat::Csv, out.path())
        .await
        .expect("exported");
    let header = read(&Path::new(&csv.written_to).join("rates.csv"))
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    assert_eq!(header, "from_currency,to_currency,date,rate,source");
}

// EXP-026 / EXP-033 — a sale of an account under FIFO names the lots it took, oldest first:
// one taken whole, one in part; an account under the weighted average has none.
#[tokio::test]
async fn exp_026_the_lots_each_sale_took_are_exported() {
    let p = portfolio().await;
    let acme = asset(&p, "Acme World", "ACM", "FR0000000010").await;
    let fifo = p
        .accounts
        .create_classified(
            "Depot".into(),
            String::new(),
            "EUR".into(),
            false,
            Some("DE".into()),
            None,
        )
        .await
        .expect("account")
        .id;
    let average = account(&p, "Compte").await;
    let r = &p.recording;
    for id in [&fifo, &average] {
        r.record_deposit(id, "2024-01-02".into(), micro(100_000), None)
            .await
            .expect("deposit");
        for (date, price) in [("2024-02-01", 100), ("2024-03-01", 200)] {
            r.buy_holding(
                id,
                acme.clone(),
                date.into(),
                from_surface(micro(10)),
                micro(price),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .expect("buy");
        }
        r.sell_holding(
            id,
            acme.clone(),
            "2024-04-01".into(),
            from_surface(micro(15)),
            micro(200),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .expect("sale");
    }
    let out = tempfile::tempdir().expect("folder");
    let report = p
        .export
        .export(ExportFormat::Json, out.path())
        .await
        .expect("exported");
    let value: serde_json::Value =
        serde_json::from_str(&read(Path::new(&report.written_to))).expect("JSON");
    let lots: Vec<(String, String, String, String, bool)> = value["lots"]
        .as_array()
        .expect("lots")
        .iter()
        .map(|lot| {
            (
                lot["account_id"].as_str().unwrap_or_default().to_string(),
                lot["acquired_on"].as_str().unwrap_or_default().to_string(),
                lot["quantity_taken"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                lot["cost_taken"].as_str().unwrap_or_default().to_string(),
                lot["free"].as_bool().unwrap_or(true),
            )
        })
        .collect();
    assert_eq!(
        lots,
        [
            (
                fifo.clone(),
                "2024-02-01".into(),
                "10".into(),
                "1000".into(),
                false
            ),
            (
                fifo.clone(),
                "2024-03-01".into(),
                "5".into(),
                "1000".into(),
                false
            ),
        ]
    );
    let sale = value["transactions"]
        .as_array()
        .expect("transactions")
        .iter()
        .find(|t| t["type"] == "Sell" && t["account_id"] == fifo.as_str())
        .expect("sale");
    assert_eq!(value["lots"][0]["transaction_id"], sale["id"]);
}
