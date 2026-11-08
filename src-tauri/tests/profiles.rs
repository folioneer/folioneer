//! Integration tests for profiles (PFL spec): the default profile, a profile created, renamed
//! and removed, an account moved, names unique within a profile, no transfer across profiles.
//!
//! Exercises the full stack through the public `folioneer_lib` API — `ProfileService`,
//! `ProfileMoveUseCase`, `TransferUseCase` — over real in-memory SQLite. No mocks (test-rules.md
//! Tier 3). The portfolio is invented data.

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
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use folioneer_lib::use_cases::profile_move::ProfileMoveUseCase;
use folioneer_lib::use_cases::transfer::{TransferFigures, TransferUseCase};
use std::sync::Arc;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

struct Ctx {
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
    let profiles = Arc::new(ProfileService::new(Box::new(SqliteProfileRepository::new(
        pool.clone(),
    ))));
    Ctx {
        profiles: profiles.clone(),
        moving: ProfileMoveUseCase::new(profiles, accounts.clone()),
        recording: HoldingTransactionUseCase::new(accounts.clone(), assets.clone()),
        transfer: TransferUseCase::new(accounts.clone(), assets.clone(), currencies),
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

// PFL-011 / PFL-013 / PFL-023 — the default profile exists unnamed and holds the accounts;
// it is listed first, with its count.
#[tokio::test]
async fn pfl_011_the_default_profile_holds_the_accounts() {
    let ctx = ctx().await;
    let id = account(&ctx, "PEA").await;
    assert_eq!(profile_of(&ctx, &id).await, DEFAULT_PROFILE_ID);
    ctx.profiles.create("Parent").await.expect("created");
    let listed = ctx.profiles.list().await.expect("listed");
    let seen: Vec<(Option<String>, u32)> = listed
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
    assert!(ctx
        .transfer
        .record_transfer(&from, &elsewhere, acme, figures())
        .await
        .is_err());
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
