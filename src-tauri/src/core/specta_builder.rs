use crate::{
    context::{account, asset, currency, sync},
    core::{logger, Event},
    use_cases::{
        account_creation, account_deletion, account_details, account_performance, account_summary,
        archive_asset, asset_price_fetch, asset_web_lookup, capabilities, delete_asset,
        fee_generation, global_performance, holding_transaction, portfolio_sync, price_freshness,
        price_history_backfill, rate_history_backfill, rate_refresh, scheduled_fetch,
        update_checker,
    },
};

/// create the Specta builder for standard and generate_bindings
pub fn create_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        // ----- asset BC -----
        .typ::<asset::Asset>()
        .typ::<asset::AssetCategory>()
        .typ::<asset::AssetClass>()
        .typ::<asset::AssetPrice>()
        .typ::<asset::AssetPriceSource>()
        .typ::<asset::AssetError>()
        .typ::<asset::Exchange>()
        // ----- account BC -----
        .typ::<account::Account>()
        .typ::<account::UpdateFrequency>()
        .typ::<account::Holding>()
        .typ::<account::HoldingSnapshot>()
        .typ::<account::AccountError>()
        .typ::<account::Transaction>()
        .typ::<account::TransactionType>()
        .typ::<account::FeeSchedule>()
        .typ::<account::FeeFrequency>()
        .typ::<account::CreateFeeScheduleDTO>()
        .typ::<account::UpdateFeeScheduleDTO>()
        .typ::<account::HoldingNote>()
        .typ::<account::ThresholdDirection>()
        .typ::<account::UpsertHoldingNoteDTO>()
        .typ::<account::DeleteHoldingNoteDTO>()
        .typ::<holding_transaction::ManagementFeeDTO>()
        .typ::<holding_transaction::ManagementFeeError>()
        .typ::<holding_transaction::ManagementFeeTask>()
        .typ::<fee_generation::FeeGenerationError>()
        // ----- currency BC (FXR) -----
        .typ::<currency::CurrencyPair>()
        .typ::<currency::CurrencyRate>()
        .typ::<currency::CurrencyRateSource>()
        .typ::<currency::CurrencyPairSummary>()
        .typ::<currency::CurrencyError>()
        // ----- sync BC (SYN/CFR) -----
        .typ::<sync::SyncFolderState>()
        .typ::<sync::FolderProblem>()
        .typ::<sync::SyncStatus>()
        .typ::<sync::RosterEntry>()
        .typ::<sync::ConflictNotice>()
        .typ::<sync::ConflictNoticeKind>()
        .typ::<crate::shared::domain::RecordKind>()
        .typ::<sync::SyncFailure>()
        .typ::<sync::InconsistentHolding>()
        .typ::<sync::HoldingInconsistency>()
        .typ::<sync::SyncReport>()
        .typ::<sync::SyncError>()
        // ----- use cases -----
        .typ::<portfolio_sync::PortfolioSyncError>()
        .typ::<portfolio_sync::PortfolioSyncTask>()
        .typ::<archive_asset::ArchiveAssetTask>()
        .typ::<archive_asset::ArchiveAssetError>()
        .typ::<delete_asset::DeleteAssetTask>()
        .typ::<delete_asset::DeleteAssetError>()
        .typ::<holding_transaction::BuyHoldingDTO>()
        .typ::<holding_transaction::SellHoldingDTO>()
        .typ::<holding_transaction::CorrectTransactionDTO>()
        .typ::<holding_transaction::OpenHoldingDTO>()
        .typ::<holding_transaction::OpenHoldingError>()
        .typ::<holding_transaction::OpenHoldingTask>()
        .typ::<holding_transaction::DepositDTO>()
        .typ::<holding_transaction::WithdrawalDTO>()
        .typ::<holding_transaction::DividendDTO>()
        .typ::<holding_transaction::DividendError>()
        .typ::<holding_transaction::DividendTask>()
        .typ::<account_details::HoldingDetail>()
        .typ::<account_details::ClosedHoldingDetail>()
        .typ::<account_details::AccountDetailsResponse>()
        .typ::<account_summary::AccountSummary>()
        .typ::<account_performance::PerformanceMetric>()
        .typ::<account_performance::PerformancePeriod>()
        .typ::<account_performance::AccountPerformanceResponse>()
        .typ::<account_deletion::AccountDeletionSummary>()
        .typ::<asset_web_lookup::AssetLookupResult>()
        .typ::<asset_web_lookup::LookupMode>()
        .typ::<asset_web_lookup::WebLookupError>()
        .typ::<asset_price_fetch::FetchAllAssetPricesError>()
        .typ::<asset_price_fetch::FetchAccountAssetPricesError>()
        .typ::<asset_price_fetch::FetchPriceTask>()
        .typ::<update_checker::UpdateInfo>()
        .typ::<scheduled_fetch::ScheduledFetchConfiguration>()
        .typ::<scheduled_fetch::ScheduledFetchRun>()
        .typ::<scheduled_fetch::ScheduledFetchOutcome>()
        .typ::<scheduled_fetch::ScheduledFetchStatus>()
        .typ::<scheduled_fetch::ScheduledFetchError>()
        .typ::<rate_history_backfill::RateHistoryBackfillError>()
        .typ::<rate_refresh::RateRefreshError>()
        .typ::<price_freshness::PriceFreshness>()
        .typ::<price_freshness::PriceFreshnessError>()
        .typ::<price_history_backfill::PriceHistoryBackfillError>()
        .typ::<price_history_backfill::PriceHistoryBackfillTask>()
        .commands(tauri_specta::collect_commands![
            // ----- asset BC -----
            asset::get_assets,
            asset::get_assets_with_archived,
            asset::add_asset,
            asset::update_asset,
            asset::unarchive_asset,
            asset::block_asset_price_refresh,
            asset::unblock_asset_price_refresh,
            asset::get_supported_exchanges,
            asset::get_categories,
            asset::add_category,
            asset::update_category,
            asset::delete_category,
            asset::record_asset_price,
            asset::get_asset_prices,
            asset::update_asset_price,
            asset::delete_asset_price,
            // ----- account BC -----
            account::get_accounts,
            account_creation::add_account,
            account::update_account,
            account::delete_account,
            account::get_asset_ids_for_account,
            account::get_transactions,
            account::get_all_transactions_for_account,
            account::get_holding_snapshot_as_of,
            account::create_fee_schedule,
            account::update_fee_schedule,
            account::delete_fee_schedule,
            account::get_fee_schedule,
            account::upsert_holding_note,
            account::delete_holding_note,
            // ----- currency BC (FXR) -----
            currency::declare_currency_pair,
            currency::record_currency_rate,
            currency::update_currency_rate,
            currency::delete_currency_rate,
            currency::get_currency_pairs,
            currency::get_currency_rates,
            // ----- sync BC (SYN/CFR) -----
            sync::pause_sync,
            sync::leave_sync,
            sync::rename_sync_device,
            sync::dismiss_conflict_notice,
            // ----- use cases -----
            portfolio_sync::inspect_sync_folder,
            portfolio_sync::enable_sync,
            portfolio_sync::start_sync_over,
            portfolio_sync::change_sync_folder,
            portfolio_sync::sync_now,
            portfolio_sync::resume_sync,
            portfolio_sync::get_sync_status,
            archive_asset::archive_asset,
            delete_asset::delete_asset,
            holding_transaction::open_holding,
            holding_transaction::buy_holding,
            holding_transaction::sell_holding,
            holding_transaction::correct_transaction,
            holding_transaction::cancel_transaction,
            holding_transaction::record_deposit,
            holding_transaction::record_withdrawal,
            holding_transaction::record_dividend,
            holding_transaction::record_free_shares,
            holding_transaction::record_split,
            holding_transaction::record_management_fee,
            holding_transaction::preview_management_fee,
            holding_transaction::record_interest,
            fee_generation::apply_due_fee_deductions,
            account_details::get_account_details,
            account_summary::get_account_summaries,
            account_performance::get_account_performance,
            global_performance::get_global_performance,
            account_deletion::get_account_deletion_summary,
            asset_web_lookup::lookup_asset,
            asset_price_fetch::fetch_all_asset_prices,
            asset_price_fetch::fetch_account_asset_prices,
            update_checker::check_for_update,
            update_checker::download_update,
            update_checker::install_update,
            scheduled_fetch::configure_scheduled_fetch,
            scheduled_fetch::get_scheduled_fetch_status,
            rate_history_backfill::backfill_currency_rate_history,
            rate_refresh::refresh_currency_rates,
            price_freshness::get_price_freshness,
            price_history_backfill::backfill_holding_price_history,
            capabilities::get_capabilities,
            // ----- core -----
            logger::log_frontend
        ])
        .events(tauri_specta::collect_events![Event])
}
