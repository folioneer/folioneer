//! Integration tests for profiles (PFL spec): the default profile, a profile created, renamed
//! and removed, an account moved, names unique within a profile, no transfer across profiles.
//!
//! Exercises the full stack through the public `folioneer_lib` API — `ProfileService`,
//! `ProfileMoveUseCase`, `TransferUseCase` — over real in-memory SQLite. No mocks (test-rules.md
//! Tier 3). The portfolio is invented data.

use chrono::Datelike;
use folioneer_lib::context::account::{
    AccountError, AccountService, ProfileService, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteProfileRepository, SqliteTransactionRepository, DEFAULT_PROFILE_ID,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::shared::domain::quantity::from_surface;
use folioneer_lib::shared::infrastructure::uow::SqliteTransactionManager;
use folioneer_lib::use_cases::account_creation::AccountCreationUseCase;
use folioneer_lib::use_cases::account_summary::AccountSummaryUseCase;
use folioneer_lib::use_cases::global_performance::GlobalPerformanceUseCase;
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use folioneer_lib::use_cases::price_freshness::PriceFreshnessUseCase;
use folioneer_lib::use_cases::profile_move::ProfileMoveUseCase;
use folioneer_lib::use_cases::profile_overview::ProfileOverviewUseCase;
use folioneer_lib::use_cases::shared::price_fetch_log::SqlitePriceFetchLogRepository;
use folioneer_lib::use_cases::transfer::{TransferError, TransferFigures, TransferUseCase};
use std::sync::Arc;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

struct Ctx {
    bus: Arc<SideEffectEventBus>,
    pool: sqlx::SqlitePool,
    currencies: Arc<CurrencyService>,
    creation: AccountCreationUseCase,
    accounts: Arc<AccountService>,
    profiles: Arc<ProfileService>,
    moving: ProfileMoveUseCase,
    assets: Arc<AssetService>,
    recording: HoldingTransactionUseCase,
    transfer: TransferUseCase,
}

async fn ctx() -> Ctx {
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
    let currencies = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let profiles = Arc::new(
        ProfileService::new(Box::new(SqliteProfileRepository::new(pool.clone())))
            .with_event_bus(Arc::clone(&bus)),
    );
    Ctx {
        creation: AccountCreationUseCase::new(
            accounts.clone(),
            assets.clone(),
            Arc::new(SqliteTransactionManager::new(pool.clone())),
        ),
        profiles: profiles.clone(),
        moving: ProfileMoveUseCase::new(profiles, accounts.clone()),
        recording: HoldingTransactionUseCase::new(accounts.clone(), assets.clone()),
        transfer: TransferUseCase::new(accounts.clone(), assets.clone(), currencies.clone()),
        currencies,
        pool,
        bus,
        accounts,
        assets,
    }
}

async fn account(ctx: &Ctx, name: &str) -> String {
    ctx.accounts
        .create(name.into(), String::new(), "EUR".into(), false)
        .await
        .expect("account")
        .id
}

async fn profile_of(ctx: &Ctx, account_id: &str) -> String {
    ctx.accounts
        .get_by_id(account_id)
        .await
        .expect("read")
        .expect("account")
        .profile_id
}

// PFL-011 / PFL-013 / PFL-021 / PFL-023 — the default profile exists unnamed and holds the
// accounts, a new one included;
// it is listed first, with its count.
#[tokio::test]
async fn pfl_011_the_default_profile_holds_the_accounts() {
    let ctx = ctx().await;
    let id = account(&ctx, "PEA").await;
    assert_eq!(profile_of(&ctx, &id).await, DEFAULT_PROFILE_ID);
    ctx.profiles.create("Parent").await.expect("created");
    let listed = ctx.profiles.list().await.expect("listed");
    assert_eq!(listed.in_use, DEFAULT_PROFILE_ID);
    let seen: Vec<(Option<String>, u32)> = listed
        .profiles
        .iter()
        .map(|s| (s.profile.name.clone(), s.account_count))
        .collect();
    assert_eq!(seen, [(None, 1), (Some("Parent".into()), 0)]);
}

// PFL-020 / PFL-030 — a name is trimmed and never empty, never another profile's, case
// ignored; the default profile is renamed; an unknown one is rejected.
#[tokio::test]
async fn pfl_020_a_profile_is_named_once() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("  Parent ").await.expect("created");
    assert_eq!(parent.name.as_deref(), Some("Parent"));
    assert!(matches!(
        ctx.profiles.create("   ").await,
        Err(AccountError::ProfileNameEmpty)
    ));
    assert!(matches!(
        ctx.profiles.create("parent").await,
        Err(AccountError::ProfileNameAlreadyExists)
    ));
    let renamed = ctx
        .profiles
        .rename(DEFAULT_PROFILE_ID, "Me")
        .await
        .expect("renamed");
    assert_eq!(renamed.name.as_deref(), Some("Me"));
    assert!(matches!(
        ctx.profiles.rename(DEFAULT_PROFILE_ID, "PARENT").await,
        Err(AccountError::ProfileNameAlreadyExists)
    ));
    assert!(matches!(
        ctx.profiles.rename("nope", "Other").await,
        Err(AccountError::ProfileNotFound { .. })
    ));
}

// PFL-031 / PFL-022 — an account moves with its history; to its own profile it changes
// nothing; two profiles may each hold an account of one name, never one profile.
#[tokio::test]
async fn pfl_031_an_account_moves_and_names_are_per_profile() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    let mine = account(&ctx, "PEA").await;
    ctx.recording
        .record_deposit(&mine, "2024-01-02".into(), micro(1_000), None)
        .await
        .expect("deposit");
    ctx.moving
        .move_account(&mine, &parent)
        .await
        .expect("moved");
    assert_eq!(profile_of(&ctx, &mine).await, parent);
    assert_eq!(
        ctx.accounts
            .get_all_transactions_for_account(&mine)
            .await
            .expect("history")
            .len(),
        1
    );
    ctx.moving
        .move_account(&mine, &parent)
        .await
        .expect("in place");
    // PFL-022 — the default profile can now hold its own "PEA".
    let other = account(&ctx, "PEA").await;
    assert!(matches!(
        ctx.moving.move_account(&other, &parent).await,
        Err(AccountError::NameAlreadyExists)
    ));
    assert!(matches!(
        ctx.moving.move_account(&other, "nope").await,
        Err(AccountError::ProfileNotFound { .. })
    ));
}

// PFL-033 — a transfer links two accounts of one profile: recording one across profiles is
// rejected, and so is moving an account a transfer links to another profile's.
#[tokio::test]
async fn pfl_033_no_transfer_across_profiles() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    let acme = ctx
        .assets
        .create_asset(CreateAssetDTO {
            kind: None,
            name: "Acme".into(),
            reference: "ACM".into(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "EUR".into(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.into(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("asset")
        .id;
    let from = account(&ctx, "Depot").await;
    let to = account(&ctx, "Depot 2").await;
    ctx.recording
        .open_holding(
            &from,
            acme.clone(),
            "2024-01-02".into(),
            from_surface(micro(10)),
            micro(100),
        )
        .await
        .expect("opening balance");
    let figures = || TransferFigures {
        date: "2024-02-01".into(),
        quantity_sent: from_surface(micro(5)),
        quantity_received: from_surface(micro(5)),
        exchange_rate: None,
        note: None,
    };
    ctx.transfer
        .record_transfer(&from, &to, acme.clone(), figures())
        .await
        .expect("transfer within one profile");
    assert!(matches!(
        ctx.moving.move_account(&to, &parent).await,
        Err(AccountError::TransferAcrossProfiles)
    ));
    let elsewhere = account(&ctx, "Elsewhere").await;
    ctx.moving
        .move_account(&elsewhere, &parent)
        .await
        .expect("moved");
    assert!(matches!(
        ctx.transfer
            .record_transfer(&from, &elsewhere, acme, figures())
            .await,
        Err(TransferError::Account(AccountError::TransferAcrossProfiles))
    ));
}

// PFL-040 — only an empty profile is removed; never the default one; an unknown one is
// rejected.
#[tokio::test]
async fn pfl_040_only_an_empty_profile_is_removed() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    let id = account(&ctx, "PEA").await;
    ctx.moving.move_account(&id, &parent).await.expect("moved");
    assert!(matches!(
        ctx.profiles.remove(&parent).await,
        Err(AccountError::ProfileNotEmpty)
    ));
    assert!(matches!(
        ctx.profiles.remove(DEFAULT_PROFILE_ID).await,
        Err(AccountError::DefaultProfileKept)
    ));
    ctx.moving
        .move_account(&id, DEFAULT_PROFILE_ID)
        .await
        .expect("moved back");
    ctx.profiles.remove(&parent).await.expect("removed");
    assert!(matches!(
        ctx.profiles.remove(&parent).await,
        Err(AccountError::ProfileNotFound { .. })
    ));
}

// PFL-012 / PFL-032 — the profile in use is the default one until another is chosen;
// choosing an unknown one is rejected, choosing the one in use changes nothing.
#[tokio::test]
async fn pfl_032_the_profile_in_use_is_chosen() {
    let ctx = ctx().await;
    assert_eq!(
        ctx.profiles.in_use().await.expect("read"),
        DEFAULT_PROFILE_ID
    );
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    ctx.profiles.choose(&parent).await.expect("chosen");
    ctx.profiles.choose(&parent).await.expect("chosen again");
    assert_eq!(ctx.profiles.list().await.expect("listed").in_use, parent);
    assert!(matches!(
        ctx.profiles.choose("nope").await,
        Err(AccountError::ProfileNotFound { .. })
    ));
    assert_eq!(ctx.profiles.in_use().await.expect("read"), parent);
}

// PFL-014 — a profile in use that is removed gives way to the default profile.
#[tokio::test]
async fn pfl_014_a_profile_in_use_removed_gives_way_to_the_default() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    ctx.profiles.choose(&parent).await.expect("chosen");
    ctx.profiles.remove(&parent).await.expect("removed");
    assert_eq!(
        ctx.profiles.in_use().await.expect("read"),
        DEFAULT_PROFILE_ID
    );
}

// PFL-021 — a new account goes into the profile in use, or the one given; an unknown one
// given is rejected and creates nothing.
#[tokio::test]
async fn pfl_021_a_new_account_goes_into_the_profile_in_use() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    ctx.profiles.choose(&parent).await.expect("chosen");
    let create = |name: &'static str, profile: Option<String>| {
        ctx.creation.create_with_method(
            name.into(),
            String::new(),
            "EUR".into(),
            false,
            None,
            None,
            None,
            profile,
        )
    };
    let in_use = create("PEA", None).await.expect("created");
    assert_eq!(in_use.profile_id, parent);
    let given = create("PEA", Some(DEFAULT_PROFILE_ID.into()))
        .await
        .expect("PFL-022: the same name in another profile");
    assert_eq!(given.profile_id, DEFAULT_PROFILE_ID);
    assert!(matches!(
        create("Other", Some("nope".into())).await,
        Err(AccountError::ProfileNotFound { .. })
    ));
    assert_eq!(ctx.accounts.get_all().await.expect("read").len(), 2);
}

/// Two accounts named "PEA", the default profile's holding 1 000 and Parent's 300, with
/// Parent the profile in use; returns Parent's account.
async fn two_profiles_parent_in_use(ctx: &Ctx) -> String {
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    let mine = account(ctx, "PEA").await;
    ctx.recording
        .record_deposit(&mine, "2024-01-02".into(), micro(1_000), None)
        .await
        .expect("deposit");
    ctx.profiles.choose(&parent).await.expect("chosen");
    let theirs = ctx
        .creation
        .create("PEA".into(), String::new(), "EUR".into(), false)
        .await
        .expect("created in the profile in use")
        .id;
    ctx.recording
        .record_deposit(&theirs, "2024-01-02".into(), micro(300), None)
        .await
        .expect("deposit");
    theirs
}

// PFL-050 — the account summaries, their total and the global performance read the profile
// in use's accounts only.
#[tokio::test]
async fn pfl_050_figures_read_the_profile_in_use() {
    let ctx = ctx().await;
    let theirs = two_profiles_parent_in_use(&ctx).await;
    let summaries = AccountSummaryUseCase::new(
        ctx.accounts.clone(),
        ctx.assets.clone(),
        ctx.currencies.clone(),
    )
    .get_account_summaries()
    .await
    .expect("summaries");
    let ids: Vec<&str> = summaries.summaries.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, [theirs.as_str()]);
    assert_eq!(summaries.total.total_global_value, micro(300));

    let performance = GlobalPerformanceUseCase::new(
        ctx.accounts.clone(),
        ctx.assets.clone(),
        ctx.currencies.clone(),
    )
    .get_global_performance(None, None)
    .await
    .expect("performance");
    assert_eq!(
        performance.yearly.last().map(|period| period.end_value),
        Some(micro(300))
    );
}

// PFL-051 / PFL-022 — the account list and a lookup by name read the profile in use's
// accounts: one name in two profiles is not ambiguous.
#[tokio::test]
async fn pfl_051_lists_and_lookups_read_the_profile_in_use() {
    let ctx = ctx().await;
    let theirs = two_profiles_parent_in_use(&ctx).await;
    let listed: Vec<String> = ctx
        .accounts
        .get_all_by_name()
        .await
        .expect("listed")
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(listed, std::slice::from_ref(&theirs));
    let found = ctx
        .recording
        .find_account_by_name("pea")
        .await
        .expect("found in the profile in use");
    assert_eq!(found.id, theirs);
    assert_eq!(
        ctx.accounts.get_all().await.expect("every account").len(),
        2
    );
}

async fn stock(ctx: &Ctx, name: &str) -> String {
    ctx.assets
        .create_asset(CreateAssetDTO {
            kind: None,
            name: name.into(),
            reference: name.to_uppercase(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "EUR".into(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.into(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("asset")
        .id
}

// PFL-050 / MKT-200 — the newest price date is that of the assets the profile in use's
// accounts hold.
#[tokio::test]
async fn pfl_050_the_newest_price_date_reads_the_profile_in_use() {
    let ctx = ctx().await;
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    let mine = account(&ctx, "Depot").await;
    let theirs = account(&ctx, "Their depot").await;
    ctx.moving
        .move_account(&theirs, &parent)
        .await
        .expect("moved");
    for (account_id, name, date) in [
        (&mine, "Acme", "2025-06-01"),
        (&theirs, "Bolt", "2025-03-01"),
    ] {
        let asset = stock(&ctx, name).await;
        ctx.recording
            .open_holding(
                account_id,
                asset.clone(),
                "2024-01-02".into(),
                from_surface(micro(1)),
                micro(10),
            )
            .await
            .expect("opening balance");
        ctx.assets
            .record_asset_price_in_micros(&asset, date, micro(12))
            .await
            .expect("price");
    }
    let freshness = PriceFreshnessUseCase::new(
        ctx.accounts.clone(),
        ctx.assets.clone(),
        Arc::new(SqlitePriceFetchLogRepository::new(ctx.pool.clone())),
    );
    let read = |label: &'static str| {
        let freshness = &freshness;
        async move { freshness.read().await.expect(label).newest_price_date }
    };
    assert_eq!(read("default in use").await.as_deref(), Some("2025-06-01"));
    ctx.profiles.choose(&parent).await.expect("chosen");
    assert_eq!(read("parent in use").await.as_deref(), Some("2025-03-01"));
}

// PFL-034 / PFL-032A — creating, renaming, choosing and removing a profile are signalled by
// `ProfileUpdated`, choosing the profile in use again by nothing; a move by `AccountUpdated`.
#[tokio::test]
async fn pfl_034_profile_changes_are_signalled() {
    use folioneer_lib::core::Event;
    let ctx = ctx().await;
    let mut rx = ctx.bus.subscribe();
    let parent = ctx.profiles.create("Parent").await.expect("created").id;
    assert_eq!(rx.drain(), [Event::ProfileUpdated]);
    ctx.profiles.rename(&parent, "Mum").await.expect("renamed");
    assert_eq!(rx.drain(), [Event::ProfileUpdated]);
    ctx.profiles.choose(&parent).await.expect("chosen");
    assert_eq!(rx.drain(), [Event::ProfileUpdated]);
    ctx.profiles.choose(&parent).await.expect("chosen again");
    assert_eq!(rx.drain(), []);
    let id = account(&ctx, "PEA").await;
    rx.drain();
    ctx.moving.move_account(&id, &parent).await.expect("moved");
    assert!(rx.drain().contains(&Event::AccountUpdated));
    let other = ctx.profiles.create("Other").await.expect("created").id;
    rx.drain();
    ctx.profiles.remove(&other).await.expect("removed");
    assert_eq!(rx.drain(), [Event::ProfileUpdated]);
}

// PFL-055 — every profile with its own value, computed within it; a profile without an
// account is worth 0 with no performance; no row adds profiles together.
#[tokio::test]
async fn pfl_055_every_profile_has_its_own_figures() {
    let ctx = ctx().await;
    let theirs = two_profiles_parent_in_use(&ctx).await;
    let son = ctx.profiles.create("Son").await.expect("created").id;
    let overview = ProfileOverviewUseCase::new(
        ctx.profiles.clone(),
        AccountSummaryUseCase::new(
            ctx.accounts.clone(),
            ctx.assets.clone(),
            ctx.currencies.clone(),
        ),
        GlobalPerformanceUseCase::new(
            ctx.accounts.clone(),
            ctx.assets.clone(),
            ctx.currencies.clone(),
        ),
    )
    .read()
    .await
    .expect("overview");
    let parent = ctx
        .accounts
        .get_by_id(&theirs)
        .await
        .expect("read")
        .expect("account")
        .profile_id;
    assert_eq!(overview.in_use, parent);
    assert_eq!(overview.currency, "EUR");
    let rows: Vec<(String, u32, i64)> = overview
        .rows
        .iter()
        .map(|row| (row.profile.id.clone(), row.account_count, row.value))
        .collect();
    assert_eq!(
        rows,
        [
            (DEFAULT_PROFILE_ID.to_string(), 1, micro(1_000)),
            (parent, 1, micro(300)),
            (son, 0, 0),
        ]
    );
    assert_eq!(overview.rows[2].ytd_performance_pct, None);
}

// PFL-055 / PRF-034 — a profile whose accounts started this year has a year-to-date
// percentage: the current month row's, as an account's (ACC-024).
#[tokio::test]
async fn pfl_055_a_profile_started_this_year_has_a_year_to_date_percentage() {
    let ctx = ctx().await;
    let today = chrono::Local::now().date_naive();
    let start = today.with_day(1).expect("first of the month");
    let id = account(&ctx, "PEA").await;
    ctx.recording
        .record_deposit(
            &id,
            start.format("%Y-%m-%d").to_string(),
            micro(1_000),
            None,
        )
        .await
        .expect("deposit");
    let overview = ProfileOverviewUseCase::new(
        ctx.profiles.clone(),
        AccountSummaryUseCase::new(
            ctx.accounts.clone(),
            ctx.assets.clone(),
            ctx.currencies.clone(),
        ),
        GlobalPerformanceUseCase::new(
            ctx.accounts.clone(),
            ctx.assets.clone(),
            ctx.currencies.clone(),
        ),
    )
    .read()
    .await
    .expect("overview");
    assert_eq!(overview.rows[0].ytd_performance_pct, Some(0));
}
