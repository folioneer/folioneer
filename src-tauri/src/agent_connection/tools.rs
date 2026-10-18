//! Running the tools an agent may call (AGT-046), in the running application: each reads or
//! writes through the queries and commands the window uses.

use std::sync::Arc;

use serde_json::Value;

use crate::context::account::{
    AccountError, AccountServiceContract, AgentRecording, Transaction, TransactionType,
};
use crate::context::asset::{
    Asset, AssetClass, AssetCreationDefaults, AssetError, AssetKind, AssetServiceContract,
    NamedAsset,
};
use crate::context::currency::CurrencyService;
use crate::core::BACKEND;
use crate::use_cases::account_details::AccountDetailsUseCase;
use crate::use_cases::account_summary::AccountSummaryUseCase;
use crate::use_cases::holding_transaction::{
    code_of, decimal_to_micro, CashAmount, Change, Correction, DividendAmount, FreeShares,
    HoldingTransactionUseCase, PriceAt, RecordedBy, Recording, StockSplit, Target, Trade,
    TradeAmount, TransactionsAsked, TypedTransaction,
};

use super::connections::{
    AgentConnections, ChangeAnswer, Granted, LastRecording, RemovalAnswer, TransactionShown,
    MAX_RECORDINGS,
};
use super::{definitions, wire};

/// Why a tool call is refused: a stable code and a reason for the agent to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// A stable code for the reason.
    pub code: String,
    /// The reason; it names only what the agent sent (AGT-042).
    pub message: String,
}

/// An account as a tool names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct AccountRow {
    name: String,
    currency: String,
}

/// An asset as a tool names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct AssetRow {
    name: String,
    reference: String,
    kind: AssetKind,
    class: AssetClass,
    currency: String,
    isin: Option<String>,
    exchange: Option<String>,
    archived: bool,
}

impl From<Asset> for AssetRow {
    fn from(asset: Asset) -> Self {
        Self {
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
}

/// AGT-052 — a session's last recording as the window names it.
pub fn last_recording(
    (transaction, asset): (crate::context::account::Transaction, String),
) -> LastRecording {
    LastRecording {
        kind: transaction.transaction_type,
        asset,
        date: transaction.date,
    }
}

/// Runs the tools in the running application, through the queries the window uses.
pub struct AgentTools {
    recorder: Arc<HoldingTransactionUseCase>,
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    summaries: AccountSummaryUseCase,
    details: AccountDetailsUseCase,
    /// Where a removal request waits for the owner's answer (AGT-080).
    connections: Arc<AgentConnections>,
}

impl AgentTools {
    /// Wires the tools over the application's services.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
        connections: Arc<AgentConnections>,
    ) -> Self {
        Self {
            recorder: Arc::new(HoldingTransactionUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
            )),
            summaries: AccountSummaryUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
                Arc::clone(&currency_service),
            ),
            details: AccountDetailsUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
                currency_service,
            ),
            account_service,
            asset_service,
            connections,
        }
    }

    /// AGT-040 / AGT-044 — runs `tool` for the session `granted` and returns its result, or
    /// why it is refused. A refusal names only what the agent sent (AGT-042).
    pub async fn call(
        &self,
        granted: &Granted,
        tool: &str,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        let result = self.run(granted, tool, arguments).await;
        // AGT-052 — a read that was answered counts as one.
        if result.is_ok() && definitions::only_reads(tool) {
            granted.count_read();
        }
        result
    }

    async fn run(
        &self,
        granted: &Granted,
        tool: &str,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        match tool {
            "correct_recording" => self.correct(granted, arguments).await,
            "cancel_recording" => self.cancel(granted, arguments).await,
            "request_removal" => self.request_removal(granted, arguments).await,
            "request_change" => self.request_change(granted, arguments).await,
            "preview_recording" => self.preview(granted, arguments).await,
            "record_opening_balance"
            | "record_purchase"
            | "record_sale"
            | "record_split"
            | "record_deposit"
            | "record_withdrawal"
            | "record_dividend"
            | "record_free_shares" => {
                let recording =
                    previewable(tool, arguments).unwrap_or_else(|| unreachable_recording(tool))?;
                self.record(granted, recording).await
            }
            "record_price" => self.record_price(granted, arguments).await,
            "add_asset" => self.add_asset(granted, arguments).await,
            "portfolio_summary" => self
                .summaries
                .get_account_summaries()
                .await
                .map_err(|error| unreadable("portfolio_summary", &error))
                .and_then(|summaries| as_json(&summaries)),
            "list_accounts" => {
                let accounts = self
                    .account_service
                    .get_all_by_name()
                    .await
                    .map_err(|error| unreadable("list_accounts", &error))?;
                let rows: Vec<AccountRow> = accounts
                    .into_iter()
                    .map(|account| AccountRow {
                        name: account.name,
                        currency: account.currency,
                    })
                    .collect();
                as_json(&rows)
            }
            "list_assets" => {
                let archived = arguments
                    .get("archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let assets = self
                    .asset_service
                    .get_non_cash_assets_by_name(archived)
                    .await
                    .map_err(|error| unreadable("list_assets", &error))?;
                let rows: Vec<AssetRow> = assets.into_iter().map(AssetRow::from).collect();
                as_json(&rows)
            }
            "list_transactions" => self.list_transactions(granted, arguments).await,
            "get_price" => self.get_price(arguments).await,
            "list_holdings" => {
                let account = text(arguments, "account")?;
                // AGT-043 — a date that is not text is refused, never read as today.
                let as_of = optional_text(arguments, "as_of").map_err(|_| Refusal {
                    code: "InvalidDate".to_string(),
                    message: "as_of is not a past date written YYYY-MM-DD".to_string(),
                })?;
                let account_id = self.account_named(account).await?;
                self.details
                    .get_account_details(&account_id, as_of.as_deref())
                    .await
                    .map_err(|error| match error {
                        AccountError::InvalidDate | AccountError::DateInFuture => Refusal {
                            code: "InvalidDate".to_string(),
                            message: "as_of is not a past date written YYYY-MM-DD".to_string(),
                        },
                        other => unreadable("list_holdings", &other),
                    })
                    .and_then(|details| as_json(&details))
            }
            _ => Err(Refusal {
                code: "UnknownTool".to_string(),
                message: "no such tool".to_string(),
            }),
        }
    }

    /// AGT-044 / AGT-045 — records through the rules of the window, marked with the agent
    /// and its session. The refusals are the recording's own (CLI-011, CLI-012).
    async fn record(&self, granted: &Granted, recording: Recording) -> Result<Value, Refusal> {
        if !granted.may_record_one_more() {
            return Err(session_limit_reached());
        }
        let today = today();
        let mark = AgentRecording {
            agent: granted.client.clone(),
            session: granted.key.clone(),
            session_started_at: granted.started_at.clone(),
        };
        // AGT-049 — once started, the recording and its mark run to their end, whatever
        // becomes of the connection that asked: a disconnect or a closing application
        // never leaves a transaction between its two writes.
        let recorder = Arc::clone(&self.recorder);
        let outcome = tokio::spawn(async move {
            recorder
                .record_named_by_agent(recording, &today, &mark)
                .await
        })
        .await
        .map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent recording: the task ended early");
            portfolio_unreadable()
        })?;
        let recorded = outcome.map_err(|refusal| {
            // A transaction that stayed without its mark was recorded all the same: it
            // counts against the session's limit (AGT-047).
            if refusal.code == "RecordedNotMarked" {
                granted.count_recording();
            }
            Refusal {
                code: refusal.code,
                message: refusal.message,
            }
        })?;
        granted.count_marked_transaction();
        self.tell_recordings(granted).await;
        tracing::info!(target: BACKEND, session = granted.session_id, transaction = %recorded.transaction.id, kind = %recorded.transaction.transaction_type, "agent recording");
        as_json(&serde_json::json!({
            "status": "recorded",
            "transaction": recorded.transaction,
            "account": recorded.account_name,
            "currency": recorded.currency,
            "asset": recorded.asset_reference,
            "zero_cost": recorded.zero_cost,
        }))
    }

    /// AGT-062 — records a price with the source `Agent`. It counts against the session's
    /// limit like a transaction (AGT-047).
    async fn record_price(&self, granted: &Granted, arguments: &Value) -> Result<Value, Refusal> {
        let typed = price_of(arguments)?;
        if !granted.may_record_one_more() {
            return Err(session_limit_reached());
        }
        // As a recording (AGT-049, AGT-086): once started, the write runs to its end.
        let recorder = Arc::clone(&self.recorder);
        let recorded =
            tokio::spawn(
                async move { recorder.record_price_named_by_agent(typed, &today()).await },
            )
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "agent price: the task ended early");
                portfolio_unreadable()
            })?
            .map_err(|refusal| Refusal {
                code: refusal.code,
                message: refusal.message,
            })?;
        granted.count_recording();
        tracing::info!(target: BACKEND, session = granted.session_id, asset = %recorded.price.asset_id, date = %recorded.price.date, "agent recorded a price");
        as_json(&serde_json::json!({
            "status": "recorded",
            "price": recorded.price,
            "asset": recorded.asset_reference,
            "currency": recorded.currency,
        }))
    }

    /// AGT-070 — the transactions of an account, one page at a time, each with who recorded
    /// it: the owner, an agent, or this session.
    async fn list_transactions(
        &self,
        granted: &Granted,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        // A date that is not text is refused, never read as no bound (AGT-043).
        let date = |name: &str| {
            optional_text(arguments, name).map_err(|_| Refusal {
                code: "InvalidDate".to_string(),
                message: format!("{name} is not a date written YYYY-MM-DD"),
            })
        };
        let asked = TransactionsAsked {
            account: text(arguments, "account")?.to_string(),
            asset: optional_text(arguments, "asset")?,
            from: date("from")?,
            to: date("to")?,
            kind: transaction_type(arguments)?,
            recorded_by: recorded_by(arguments, &granted.key)?,
            page: page(arguments)?,
        };
        let listed = self
            .recorder
            .list_named(asked)
            .await
            .map_err(|refusal| Refusal {
                code: refusal.code,
                message: refusal.message,
            })?;
        let rows: Vec<Value> = listed
            .transactions
            .into_iter()
            .map(|row| {
                let transaction = row.transaction;
                // The words `recorded_by` filters with: no mark on this computer is the
                // owner's, this session's mark is `session`, any other `agent`.
                let recorded_by = match row.recorded_by {
                    None => serde_json::json!({ "by": "owner" }),
                    Some(mark) => serde_json::json!({
                        "by": if mark.session == granted.key { "session" } else { "agent" },
                        "agent": mark.agent,
                        "session_started_at": mark.session_started_at,
                    }),
                };
                serde_json::json!({
                    "id": transaction.id,
                    "date": transaction.date,
                    "transaction_type": transaction.transaction_type,
                    "asset": row.asset_reference,
                    "quantity": transaction.quantity,
                    "unit_price": transaction.unit_price,
                    "total_amount": transaction.total_amount,
                    "fees": transaction.fees,
                    "exchange_rate": transaction.exchange_rate,
                    "note": transaction.note,
                    "recorded_by": recorded_by,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "status": "listed",
            "account": listed.account_name,
            "currency": listed.currency,
            "total": listed.total,
            "page": listed.page,
            "pages": listed.pages,
            "transactions": rows,
        }))
    }

    /// AGT-071 — the price an asset has at a date, or the closest earlier one, with its own
    /// date and its source; `no_price` when it has none up to that date.
    async fn get_price(&self, arguments: &Value) -> Result<Value, Refusal> {
        let asset = text(arguments, "asset")?;
        // A date that is not text is refused, never read as today (AGT-043).
        let date = optional_text(arguments, "date").map_err(|_| Refusal {
            code: "InvalidDate".to_string(),
            message: "date is not a date written YYYY-MM-DD".to_string(),
        })?;
        let read = self
            .recorder
            .price_named(asset, date.as_deref(), &today())
            .await
            .map_err(|refusal| Refusal {
                code: refusal.code,
                message: refusal.message,
            })?;
        Ok(serde_json::json!({
            "status": if read.price.is_some() { "priced" } else { "no_price" },
            "asset": read.asset.reference,
            "currency": read.asset.currency,
            "date": read.date,
            // Its own date, the price of one unit and its source: what the rule lists.
            "price": read.price.map(|price| serde_json::json!({
                "date": price.date,
                "price": price.price,
                "source": price.source,
            })),
        }))
    }

    /// AGT-064 — adds an asset through the rules of the window's form, as the command line
    /// does (CLI-026): the core decides what is left out. It takes a place of the session's
    /// limit (AGT-047) and the window is told.
    async fn add_asset(&self, granted: &Granted, arguments: &Value) -> Result<Value, Refusal> {
        let kind = asset_kind(arguments)?;
        let named = NamedAsset {
            name: kept_text(arguments, "name", ASSET_NAME_LENGTH)?,
            reference: kept_text(arguments, "reference", ASSET_REFERENCE_LENGTH)?,
            kind,
            class: asset_class(arguments, kind)?,
            currency: text(arguments, "currency")?.to_string(),
            isin: optional_text(arguments, "isin")?,
            exchange_code: optional_text(arguments, "exchange")?,
            risk_level: None,
            category_name: None,
        };
        if !granted.may_record_one_more() {
            return Err(session_limit_reached());
        }
        // As a recording (AGT-049, AGT-086): once started, the addition runs to its end.
        let assets = Arc::clone(&self.asset_service);
        let added = tokio::spawn(async move { assets.add_named_asset(named).await })
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "agent adding an asset: the task ended early");
                portfolio_unreadable()
            })?
            .map_err(|error| {
                let code = code_of(&error);
                if matches!(error, AssetError::DatabaseError) {
                    tracing::error!(target: BACKEND, session = granted.session_id, "agent adding an asset: the portfolio could not be read or written");
                }
                Refusal {
                    message: match error {
                        AssetError::AssetAlreadyExists { .. } => {
                            "the portfolio already has this asset".to_string()
                        }
                        AssetError::DatabaseError => {
                            "the portfolio could not be read or written".to_string()
                        }
                        _ => format!("refused by the rules ({code})"),
                    },
                    code,
                }
            })?;
        granted.count_recording();
        granted.count_asset(added.asset.name.clone(), added.asset.reference.clone());
        tracing::info!(target: BACKEND, session = granted.session_id, asset = %added.asset.id, "agent added an asset");
        as_json(&serde_json::json!({
            "status": "added",
            "asset": AssetRow::from(added.asset),
            "reference_shared": added.reference_shared,
        }))
    }

    /// AGT-052 — reads what the session recorded and that still exists, for the window. A
    /// read that fails leaves what the window was last told.
    async fn tell_recordings(&self, granted: &Granted) {
        match self.recorder.recorded_in_session(&granted.key).await {
            Ok(recordings) => {
                granted.recordings_are(recordings.count, recordings.last.map(last_recording));
            }
            Err(error) => {
                tracing::warn!(target: BACKEND, session = granted.session_id, err = ?error, "agent session: its recordings could not be counted");
            }
        }
    }

    /// AGT-050 — the session cancels a transaction it recorded itself.
    async fn cancel(&self, granted: &Granted, arguments: &Value) -> Result<Value, Refusal> {
        let transaction_id = text(arguments, "transaction")?.to_string();
        let recorder = Arc::clone(&self.recorder);
        let session = granted.key.clone();
        // As a recording (AGT-049): once started, the cancelling runs to its end.
        let cancelled = tokio::spawn(async move {
            recorder
                .cancel_recorded_by_agent(&transaction_id, &session)
                .await
        })
        .await
        .map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent cancelling: the task ended early");
            portfolio_unreadable()
        })?
        .map_err(|refusal| Refusal {
            code: refusal.code,
            message: refusal.message,
        })?;
        self.tell_recordings(granted).await;
        tracing::info!(target: BACKEND, session = granted.session_id, transaction = %cancelled.id, "agent cancelled its recording");
        as_json(&serde_json::json!({ "status": "cancelled", "transaction": cancelled.id }))
    }

    /// AGT-080 — the session asks the owner to remove a transaction the owner typed, and
    /// waits for the answer.
    async fn request_removal(
        &self,
        granted: &Granted,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        let transaction_id = text(arguments, "transaction")?;
        let typed = self
            .recorder
            .typed_by_owner(transaction_id)
            .await
            .map_err(|refusal| Refusal {
                code: refusal.code,
                message: refusal.message,
            })?;
        let shown = shown(&typed, &typed.transaction);
        tracing::info!(target: BACKEND, session = granted.session_id, transaction = %typed.transaction.id, "agent asks to remove a transaction the owner typed");
        match self.connections.ask_removal(granted, shown).await {
            RemovalAnswer::Removed => as_json(
                &serde_json::json!({ "status": "removed", "transaction": typed.transaction.id }),
            ),
            RemovalAnswer::Refused => Err(Refusal {
                code: "RemovalRefused".to_string(),
                message: "the owner did not remove this transaction".to_string(),
            }),
            RemovalAnswer::NotRemoved { code, message } => Err(Refusal { code, message }),
        }
    }

    /// AGT-087 — the session asks the owner to change a transaction the owner typed, and
    /// waits for the answer. What the window's correction would refuse is refused first.
    async fn request_change(&self, granted: &Granted, arguments: &Value) -> Result<Value, Refusal> {
        let transaction_id = text(arguments, "transaction")?;
        let change = Change {
            correction: Correction {
                date: optional_text(arguments, "date")?,
                quantity: figure(arguments, "quantity")?,
                price: figure(arguments, "price")?,
                total: figure(arguments, "total")?,
                fees: figure(arguments, "fees")?,
                rate: figure(arguments, "rate")?,
                note: note(arguments)?,
            },
            amount: figure(arguments, "amount")?,
            new: optional_whole(arguments, "new")?,
            old: optional_whole(arguments, "old")?,
        };
        let asked = self
            .recorder
            .ask_change_of_typed(transaction_id, change)
            .await
            .map_err(|refusal| {
                tracing::info!(target: BACKEND, session = granted.session_id, code = %refusal.code, "agent's change refused before asking the owner");
                Refusal {
                    code: refusal.code,
                    message: refusal.message,
                }
            })?;
        let id = asked.typed.transaction.id.clone();
        let as_it_is = shown(&asked.typed, &asked.typed.transaction);
        let becomes = shown(&asked.typed, &asked.becomes);
        tracing::info!(target: BACKEND, session = granted.session_id, transaction = %id, "agent asks to change a transaction the owner typed");
        match self
            .connections
            .ask_change(granted, as_it_is, becomes, asked.fields)
            .await
        {
            ChangeAnswer::Changed(changed) => {
                as_json(&serde_json::json!({ "status": "changed", "transaction": changed }))
            }
            ChangeAnswer::Refused => Err(Refusal {
                code: "ChangeRefused".to_string(),
                message: "the owner did not change this transaction".to_string(),
            }),
            ChangeAnswer::NotChanged { code, message } => Err(Refusal { code, message }),
        }
    }

    /// AGT-072 — what a recording would do, nothing written: the recording tool's arguments
    /// read as that tool reads them, then the refusal the real call would give, or the
    /// transaction with no id and the account without it and with it.
    async fn preview(&self, granted: &Granted, arguments: &Value) -> Result<Value, Refusal> {
        let tool = text(arguments, "tool")?;
        let not_previewed = || Refusal {
            code: "InvalidArguments".to_string(),
            message: format!("preview_recording previews {}", PREVIEWED.join(", ")),
        };
        if !PREVIEWED.contains(&tool) {
            return Err(not_previewed());
        }
        let inner = arguments.get("arguments").filter(|inner| inner.is_object());
        let Some(inner) = inner else {
            return Err(not_previewed());
        };
        let refused = |refusal: crate::use_cases::holding_transaction::Refusal| Refusal {
            code: refusal.code,
            message: refusal.message,
        };
        if tool == "record_price" {
            let previewed = self
                .recorder
                .preview_price_named(price_of(inner)?, &today())
                .await
                .map_err(refused)?;
            return as_json(&serde_json::json!({
                "status": "previewed",
                "price": previewed.price.price,
                "asset": previewed.price.asset_reference,
                "currency": previewed.price.currency,
                "replaces": previewed.replaced.map(|replaced| replaced.price),
            }));
        }
        let (previewed, keeps_its_id) = if tool == "correct_recording" {
            let (transaction_id, correction) = correction_of(inner)?;
            let previewed = self
                .recorder
                .preview_correction_by_agent(&transaction_id, &granted.key, correction)
                .await
                .map_err(refused)?;
            (previewed, true)
        } else {
            let recording = previewable(tool, inner).ok_or_else(not_previewed)??;
            let previewed = self
                .recorder
                .preview_named(recording, &today())
                .await
                .map_err(refused)?;
            (previewed, false)
        };
        let mut transaction =
            serde_json::to_value(&previewed.transaction).map_err(|_| portfolio_unreadable())?;
        if !keeps_its_id {
            if let Some(id) = transaction.get_mut("id") {
                *id = Value::Null;
            }
        }
        as_json(&serde_json::json!({
            "status": "previewed",
            "transaction": transaction,
            "account": previewed.account_name,
            "currency": previewed.currency,
            "date": previewed.date,
            "before": previewed.before,
            "after": previewed.after,
        }))
    }

    /// AGT-050 — the session corrects a transaction it recorded itself.
    async fn correct(&self, granted: &Granted, arguments: &Value) -> Result<Value, Refusal> {
        let (transaction_id, correction) = correction_of(arguments)?;
        let recorder = Arc::clone(&self.recorder);
        let session = granted.key.clone();
        let corrected = tokio::spawn(async move {
            recorder
                .correct_recorded_by_agent(&transaction_id, &session, correction)
                .await
        })
        .await
        .map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent correcting: the task ended early");
            portfolio_unreadable()
        })?
        .map_err(|refusal| Refusal {
            code: refusal.code,
            message: refusal.message,
        })?;
        self.tell_recordings(granted).await;
        tracing::info!(target: BACKEND, session = granted.session_id, transaction = %corrected.id, "agent corrected its recording");
        as_json(&serde_json::json!({ "status": "corrected", "transaction": corrected }))
    }

    /// AGT-043 — the one account with this name, case ignored; the refusal names only what
    /// was sent (AGT-042).
    async fn account_named(&self, typed: &str) -> Result<String, Refusal> {
        let accounts = self
            .account_service
            .get_all_by_name()
            .await
            .map_err(|error| unreadable("list_holdings", &error))?;
        let mut matches = accounts
            .iter()
            .filter(|account| account.name.to_lowercase() == typed.to_lowercase());
        let found = matches.next().ok_or_else(|| Refusal {
            code: "AccountNotFound".to_string(),
            message: format!("no account named \"{}\"", typed.escape_debug()),
        })?;
        if matches.next().is_some() {
            return Err(Refusal {
                code: "AccountAmbiguous".to_string(),
                message: format!(
                    "more than one account is named \"{}\"",
                    typed.escape_debug()
                ),
            });
        }
        Ok(found.id.clone())
    }
}

/// AGT-047 — the refusal of a recording past the session's limit.
fn session_limit_reached() -> Refusal {
    Refusal {
        code: "SessionLimitReached".to_string(),
        message: format!(
            "this session has recorded or added {MAX_RECORDINGS} transactions, prices and assets, its limit: the owner disconnects and allows a new session to go on"
        ),
    }
}

/// Today on this computer, as a recording without a date takes it.
fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// AGT-048 — a figure as an agent sends it: a decimal with a dot and at most six decimals,
/// as text or as a JSON number.
fn figure(arguments: &Value, name: &str) -> Result<Option<i64>, Refusal> {
    let (typed, plain) = match arguments.get(name) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(text)) => (text.clone(), text.clone()),
        Some(Value::Number(number)) => {
            let typed = number.to_string();
            let plain = without_exponent(&typed);
            (typed, plain)
        }
        Some(_) => (String::new(), String::new()),
    };
    decimal_to_micro(&plain).map(Some).ok_or_else(|| Refusal {
        code: "InvalidFigure".to_string(),
        message: format!(
            "{name} is not a decimal with a dot and at most six decimals: \"{}\"",
            typed.escape_debug()
        ),
    })
}

/// AGT-048 — a JSON number as its plain decimal: below 0.00001 or very large, it is written
/// in exponent form (`2e-6`, `1.5e-5`, `1e21`), which is moved into the digits by moving the
/// dot. Anything else is returned as it came, for `decimal_to_micro` to judge.
fn without_exponent(number: &str) -> String {
    let Some((mantissa, exponent)) = number.split_once(['e', 'E']) else {
        return number.to_string();
    };
    let Ok(exponent) = exponent.parse::<i32>() else {
        return number.to_string();
    };
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    // Where the dot falls in `digits` once the exponent has moved it.
    let dot = whole.len() as i32 + exponent;
    // A position past the figures `decimal_to_micro` can hold is refused there, without
    // writing out thousands of zeros.
    if !(-20..=40).contains(&dot) {
        return number.to_string();
    }
    let plain = if dot <= 0 {
        format!("0.{}{digits}", "0".repeat(dot.unsigned_abs() as usize))
    } else if dot as usize >= digits.len() {
        format!("{digits}{}", "0".repeat(dot as usize - digits.len()))
    } else {
        let (before, after) = digits.split_at(dot as usize);
        format!("{before}.{after}")
    };
    format!("{sign}{plain}")
}

fn amount(arguments: &Value, name: &str) -> Result<i64, Refusal> {
    figure(arguments, name)?.ok_or_else(|| Refusal {
        code: "MissingArgument".to_string(),
        message: format!("{name} is missing"),
    })
}

/// AGT-050 — the transaction `correct_recording` names and what it changes.
fn correction_of(arguments: &Value) -> Result<(String, Correction), Refusal> {
    let transaction_id = text(arguments, "transaction")?.to_string();
    let price = figure(arguments, "price")?;
    let total = figure(arguments, "total")?;
    if price.is_some() && total.is_some() {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: "give either price or total, not both".to_string(),
        });
    }
    let correction = Correction {
        date: optional_text(arguments, "date")?,
        quantity: figure(arguments, "quantity")?,
        price,
        total,
        fees: figure(arguments, "fees")?,
        rate: figure(arguments, "rate")?,
        note: note(arguments)?,
    };
    if correction == Correction::default() {
        return Err(Refusal {
            code: "MissingArgument".to_string(),
            message: "nothing to correct: give what changes".to_string(),
        });
    }
    Ok((transaction_id, correction))
}

/// AGT-062 — the price `record_price` records.
fn price_of(arguments: &Value) -> Result<PriceAt, Refusal> {
    Ok(PriceAt {
        asset: text(arguments, "asset")?.to_string(),
        date: optional_text(arguments, "date")?,
        price: amount(arguments, "price")?,
    })
}

/// AGT-072 — the calls `preview_recording` previews.
const PREVIEWED: [&str; 10] = [
    "record_purchase",
    "record_sale",
    "record_opening_balance",
    "record_split",
    "record_deposit",
    "record_withdrawal",
    "record_dividend",
    "record_free_shares",
    "record_price",
    "correct_recording",
];

/// The recordings among the calls `preview_recording` previews.
const RECORDINGS: [&str; 8] = [
    "record_purchase",
    "record_sale",
    "record_opening_balance",
    "record_split",
    "record_deposit",
    "record_withdrawal",
    "record_dividend",
    "record_free_shares",
];

/// AGT-044 / AGT-063 / AGT-072 — the recording `tool` makes of `arguments`, read as that
/// tool reads them; `None` for a tool `preview_recording` does not preview.
fn previewable(tool: &str, arguments: &Value) -> Option<Result<Recording, Refusal>> {
    let read = || -> Result<Recording, Refusal> {
        Ok(match tool {
            "record_opening_balance" => Recording::Open {
                target: target(arguments)?,
                total_cost: amount(arguments, "total_cost")?,
            },
            "record_purchase" => Recording::Buy(trade(arguments)?),
            "record_sale" => Recording::Sell(trade(arguments)?),
            "record_deposit" => Recording::Deposit(cash(arguments)?),
            "record_withdrawal" => Recording::Withdrawal(cash(arguments)?),
            "record_dividend" => Recording::Dividend(DividendAmount {
                account: text(arguments, "account")?.to_string(),
                asset: text(arguments, "asset")?.to_string(),
                date: optional_text(arguments, "date")?,
                amount: amount(arguments, "amount")?,
                rate: figure(arguments, "rate")?,
                note: note(arguments)?,
            }),
            "record_free_shares" => Recording::FreeShares(FreeShares {
                account: text(arguments, "account")?.to_string(),
                asset: text(arguments, "asset")?.to_string(),
                date: optional_text(arguments, "date")?,
                quantity: amount(arguments, "quantity")?,
                note: note(arguments)?,
            }),
            "record_split" => Recording::Split(StockSplit {
                account: text(arguments, "account")?.to_string(),
                asset: text(arguments, "asset")?.to_string(),
                date: optional_text(arguments, "date")?,
                new: whole(arguments, "new")?,
                old: whole(arguments, "old")?,
                note: note(arguments)?,
            }),
            _ => return unreachable_recording(tool),
        })
    };
    RECORDINGS.contains(&tool).then(read)
}

/// Every tool in `RECORDINGS` has a reading in `previewable`: this is never reached.
fn unreachable_recording(tool: &str) -> Result<Recording, Refusal> {
    tracing::error!(target: BACKEND, tool, "a recording tool without its reading");
    Err(portfolio_unreadable())
}

/// AGT-081 — `transaction`, a state of the transaction the owner typed, with the names `typed`
/// gives it.
fn shown(typed: &TypedTransaction, transaction: &Transaction) -> TransactionShown {
    let (asset_name, asset_reference) = typed.asset.as_ref().map_or((None, None), |asset| {
        (Some(asset.name.clone()), Some(asset.reference.clone()))
    });
    TransactionShown {
        id: transaction.id.clone(),
        date: transaction.date.clone(),
        account: typed.account_name.clone(),
        currency: typed.currency.clone(),
        kind: transaction.transaction_type,
        asset_name,
        asset_reference,
        quantity: transaction.quantity,
        amount: transaction.total_amount,
        unit_price: transaction.unit_price,
        fees: transaction.fees,
        exchange_rate: transaction.exchange_rate,
        note: transaction.note.clone(),
    }
}

/// AGT-087 — a part of a split's ratio when it is given.
fn optional_whole(arguments: &Value, name: &str) -> Result<Option<i64>, Refusal> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => whole(arguments, name).map(Some),
    }
}

/// AGT-063 — a part of a split's ratio: a whole number, as text or as a JSON number. The
/// refusal repeats at most the first 40 characters of what was sent.
fn whole(arguments: &Value, name: &str) -> Result<i64, Refusal> {
    let not_whole = |sent: &str| Refusal {
        code: "InvalidFigure".to_string(),
        message: format!("{name} is not a whole number{sent}"),
    };
    let typed = match arguments.get(name) {
        None | Some(Value::Null) => {
            return Err(Refusal {
                code: "MissingArgument".to_string(),
                message: format!("{name} is missing"),
            })
        }
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(_) => return Err(not_whole("")),
    };
    typed.trim().parse().map_err(|_| {
        let shown: String = typed.chars().take(40).collect();
        not_whole(&format!(": \"{}\"", shown.escape_debug()))
    })
}

/// AGT-070 — `transaction_type` as one of the types a transaction has, case ignored.
fn transaction_type(arguments: &Value) -> Result<Option<TransactionType>, Refusal> {
    let Some(typed) = optional_text(arguments, "transaction_type")? else {
        return Ok(None);
    };
    TransactionType::named(&typed)
        .map(Some)
        .ok_or_else(|| Refusal {
            code: "InvalidArguments".to_string(),
            message: format!(
                "transaction_type is not one of {}",
                TransactionType::names().join(", ")
            ),
        })
}

/// AGT-070 — `recorded_by` as `owner`, `agent` or `session`, this session being `session`.
fn recorded_by(arguments: &Value, session: &str) -> Result<Option<RecordedBy>, Refusal> {
    let Some(typed) = optional_text(arguments, "recorded_by")? else {
        return Ok(None);
    };
    match typed.trim().to_lowercase().as_str() {
        "owner" => Ok(Some(RecordedBy::Owner)),
        "agent" => Ok(Some(RecordedBy::Agent)),
        "session" => Ok(Some(RecordedBy::Session(session.to_string()))),
        _ => Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: "recorded_by is not one of owner, agent, session".to_string(),
        }),
    }
}

/// AGT-070 — `page` as a whole number from 1; left out, the first.
fn page(arguments: &Value) -> Result<usize, Refusal> {
    match arguments.get("page") {
        None | Some(Value::Null) => Ok(1),
        Some(value) => value
            .as_u64()
            .filter(|page| *page >= 1)
            .and_then(|page| usize::try_from(page).ok())
            .ok_or_else(|| Refusal {
                code: "InvalidArguments".to_string(),
                message: "page is not a whole number from 1".to_string(),
            }),
    }
}

/// AGT-064 — the longest name and reference an agent gives an asset, in characters.
const ASSET_NAME_LENGTH: usize = 100;
const ASSET_REFERENCE_LENGTH: usize = 30;

/// AGT-064 — text an agent wrote that the portfolio keeps and the window shows: it is
/// refused when it is too long or holds a character that shows nothing or reorders text.
fn kept_text(arguments: &Value, name: &str, longest: usize) -> Result<String, Refusal> {
    let typed = text(arguments, name)?;
    if !wire::shows_as_written(typed, longest) {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: format!(
                "{name} is longer than {longest} characters, or holds a control or an invisible character"
            ),
        });
    }
    Ok(typed.to_string())
}

/// AGT-064 — `kind` as one of the kinds the core lets a user add an asset of, case ignored;
/// left out, the core decides it (AST-034).
fn asset_kind(arguments: &Value) -> Result<Option<AssetKind>, Refusal> {
    let Some(typed) = optional_text(arguments, "kind")? else {
        return Ok(None);
    };
    AssetCreationDefaults::current()
        .kinds
        .into_iter()
        .map(|form| form.kind)
        .find(|kind| kind.to_string().eq_ignore_ascii_case(typed.trim()))
        .map(Some)
        .ok_or_else(|| Refusal {
            code: "InvalidArguments".to_string(),
            message: "kind is not one of Listed, Crypto, Custom".to_string(),
        })
}

/// AGT-064 — `class` as one of the classes the core lets a user add an asset in, case
/// ignored; left out, it is the class the core preselects for `kind` (AST-037).
fn asset_class(arguments: &Value, kind: Option<AssetKind>) -> Result<AssetClass, Refusal> {
    let Some(typed) = optional_text(arguments, "class")? else {
        return kind
            .and_then(AssetCreationDefaults::class_of)
            .ok_or_else(|| Refusal {
                code: "MissingArgument".to_string(),
                message: "class is missing: give a kind, a class, or both".to_string(),
            });
    };
    let addable = AssetClass::user_addable();
    addable
        .iter()
        .find(|class| class.to_string().eq_ignore_ascii_case(typed.trim()))
        .cloned()
        .ok_or_else(|| {
            let names: Vec<String> = addable.iter().map(ToString::to_string).collect();
            Refusal {
                code: "InvalidArguments".to_string(),
                message: format!("class is not one of {}", names.join(", ")),
            }
        })
}

/// The account, the asset, the quantity and the date of a recording.
fn target(arguments: &Value) -> Result<Target, Refusal> {
    Ok(Target {
        account: text(arguments, "account")?.to_string(),
        asset: text(arguments, "asset")?.to_string(),
        date: optional_text(arguments, "date")?,
        quantity: amount(arguments, "quantity")?,
    })
}

/// AGT-060 — a deposit or a withdrawal: the account, the amount and the date.
fn cash(arguments: &Value) -> Result<CashAmount, Refusal> {
    Ok(CashAmount {
        account: text(arguments, "account")?.to_string(),
        date: optional_text(arguments, "date")?,
        amount: amount(arguments, "amount")?,
        note: note(arguments)?,
    })
}

/// A purchase or a sale: a unit price or an all-in total, never both (CLI-013).
fn trade(arguments: &Value) -> Result<Trade, Refusal> {
    let amount = match (figure(arguments, "price")?, figure(arguments, "total")?) {
        (Some(price), None) => TradeAmount::Price(price),
        (None, Some(total)) => TradeAmount::Total(total),
        _ => {
            return Err(Refusal {
                code: "InvalidArguments".to_string(),
                message: "give either price or total, not both".to_string(),
            })
        }
    };
    Ok(Trade {
        target: target(arguments)?,
        amount,
        fees: figure(arguments, "fees")?.unwrap_or(0),
        rate: figure(arguments, "rate")?.unwrap_or(1_000_000),
        note: note(arguments)?,
    })
}

/// The longest note an agent may attach to a recording, in characters.
const NOTE_LENGTH: usize = 500;

/// An optional text argument: absent or null is none; anything but text is refused, never
/// ignored — a date sent as a number must not become today.
fn optional_text(arguments: &Value, name: &str) -> Result<Option<String>, Refusal> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: format!("{name} is not text"),
        }),
    }
}

/// AGT-048 — the note of a recording, at most `NOTE_LENGTH` characters.
fn note(arguments: &Value) -> Result<Option<String>, Refusal> {
    let note = optional_text(arguments, "note")?;
    if note
        .as_ref()
        .is_some_and(|note| note.chars().count() > NOTE_LENGTH)
    {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: format!("note is longer than {NOTE_LENGTH} characters"),
        });
    }
    Ok(note)
}

fn text<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, Refusal> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Refusal {
            code: "MissingArgument".to_string(),
            message: format!("{name} is missing"),
        })
}

fn as_json<T: serde::Serialize>(value: &T) -> Result<Value, Refusal> {
    serde_json::to_value(value).map_err(|error| {
        tracing::error!(target: BACKEND, err = ?error, "agent tool: result not serializable");
        portfolio_unreadable()
    })
}

fn unreadable(tool: &str, error: &dyn std::fmt::Debug) -> Refusal {
    tracing::error!(target: BACKEND, tool, err = ?error, "agent tool: read failed");
    portfolio_unreadable()
}

fn portfolio_unreadable() -> Refusal {
    Refusal {
        code: "DatabaseError".to_string(),
        message: "the portfolio could not be read".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const M: i64 = 1_000_000;

    fn code<T: std::fmt::Debug>(refused: Result<T, Refusal>) -> String {
        refused.expect_err("refused").code
    }

    // AGT-048 — a figure is a decimal with a dot, as text or as a JSON number; anything
    // else is refused, never guessed.
    #[test]
    fn agt_048_a_figure_is_a_decimal_as_text_or_as_a_number() {
        let arguments = json!({
            "text": "12.5", "number": 12.5, "whole": 3, "negative": -1, "nothing": null,
            "comma": "1,5", "exponent": 1e-7, "flag": true, "list": [1], "huge": "99999999999999999999",
        });
        assert_eq!(figure(&arguments, "text"), Ok(Some(12_500_000)));
        assert_eq!(figure(&arguments, "number"), Ok(Some(12_500_000)));
        assert_eq!(figure(&arguments, "whole"), Ok(Some(3 * M)));
        assert_eq!(figure(&arguments, "negative"), Ok(Some(-M)));
        assert_eq!(figure(&arguments, "nothing"), Ok(None));
        assert_eq!(figure(&arguments, "absent"), Ok(None));
        for refused in ["comma", "exponent", "flag", "list", "huge"] {
            assert_eq!(
                code(figure(&arguments, refused)),
                "InvalidFigure",
                "{refused}"
            );
        }
        assert_eq!(code(amount(&arguments, "absent")), "MissingArgument");
        assert_eq!(amount(&arguments, "whole"), Ok(3 * M));
    }

    // AGT-048 — a JSON number below 0.00001 reaches the server in exponent form (`2e-6`);
    // it is read as the decimal it stands for, and more than six decimals stay refused.
    #[test]
    fn agt_048_a_small_number_in_exponent_form_is_read_as_its_decimal() {
        let arguments = json!({
            "tiny": 2e-6, "small": 1.5e-5, "negative": -3e-6, "seven_decimals": 1e-7,
            "large": 1e21, "as_text": "2e-6",
        });
        assert_eq!(figure(&arguments, "tiny"), Ok(Some(2)));
        assert_eq!(figure(&arguments, "small"), Ok(Some(15)));
        assert_eq!(figure(&arguments, "negative"), Ok(Some(-3)));
        for refused in ["seven_decimals", "large", "as_text"] {
            assert_eq!(
                code(figure(&arguments, refused)),
                "InvalidFigure",
                "{refused}"
            );
        }
    }

    // AGT-048 / AGT-044 — a purchase or a sale takes a unit price or a total, never both and never
    // neither; fees default to 0 and the rate to 1.
    #[test]
    fn agt_044_a_trade_takes_a_price_or_a_total_and_its_defaults() {
        let base = |extra: Value| {
            let mut arguments = json!({ "account": "PEA", "asset": "CW8", "quantity": "2" });
            for (name, value) in extra.as_object().expect("object") {
                arguments[name] = value.clone();
            }
            arguments
        };

        let priced = trade(&base(json!({ "price": "100.5" }))).expect("priced");
        assert_eq!(priced.amount, TradeAmount::Price(100_500_000));
        assert_eq!((priced.fees, priced.rate), (0, M));
        assert_eq!(priced.target.date, None);
        assert_eq!(priced.note, None);
        let total = trade(&base(json!({ "total": 201, "fees": "1.99", "rate": 1.1,
            "date": "2019-12-31", "note": "from a statement" })))
        .expect("total");
        assert_eq!(total.amount, TradeAmount::Total(201 * M));
        assert_eq!((total.fees, total.rate), (1_990_000, 1_100_000));
        assert_eq!(total.target.date.as_deref(), Some("2019-12-31"));
        assert_eq!(total.note.as_deref(), Some("from a statement"));

        assert_eq!(code(trade(&base(json!({})))), "InvalidArguments");
        assert_eq!(
            code(trade(&base(json!({ "price": 1, "total": 1 })))),
            "InvalidArguments"
        );
    }

    // AGT-048 — a date or a note that is not text is refused, never ignored: a date sent as
    // a number must not become today. A note is at most 500 characters.
    #[test]
    fn agt_048_a_date_or_a_note_that_is_not_text_is_refused() {
        let arguments = |extra: (&str, Value)| {
            let mut arguments =
                json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "price": 1 });
            arguments[extra.0] = extra.1;
            arguments
        };
        let refusal = trade(&arguments(("date", json!(20260105)))).expect_err("refused");
        assert_eq!(refusal.code, "InvalidArguments");
        assert_eq!(refusal.message, "date is not text");
        assert_eq!(
            code(trade(&arguments(("note", json!(["x"]))))),
            "InvalidArguments"
        );
        assert_eq!(
            code(trade(&arguments((
                "note",
                json!("x".repeat(NOTE_LENGTH + 1))
            )))),
            "InvalidArguments"
        );
        assert!(trade(&arguments(("note", json!("é".repeat(NOTE_LENGTH))))).is_ok());
        assert_eq!(
            code(target(
                &json!({ "account": " ", "asset": "CW8", "quantity": 1 })
            )),
            "MissingArgument"
        );
    }
}
