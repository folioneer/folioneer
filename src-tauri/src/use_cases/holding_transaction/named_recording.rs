//! Recording by what a person named (CLI-011, CLI-012, AGT-044): the account and the asset
//! found by name, the transaction recorded through the rules of the window, and the refusal
//! worded for whoever asked. The command line and the agent connection both record through
//! it, so the two never differ. A price at a date is recorded here too (CLI-029, AGT-062): it
//! is no holding transaction, and it shares the lookup by name and the refusals.

use crate::shared::domain::quantity::{from_surface, to_surface};
use serde::Serialize;

use crate::context::account::{
    Account, AccountError, AgentRecording, RecordingOp, RecordingPreview, TradeOp, Transaction,
    TransactionType, WriteOffReason,
};
use crate::context::asset::{Asset, AssetClass, AssetError, AssetPrice};
use crate::core::BACKEND;

use super::error::{
    DividendError, DividendTask, FreeSharesError, FreeSharesTask, InterestError, InterestTask,
    ManagementFeeError, ManagementFeeTask, NameLookupError, OpenHoldingError, OpenHoldingTask,
    SplitError, SplitTask, WriteOffError, WriteOffTask,
};
use super::orchestrator::{HoldingTransactionUseCase, NamedTarget, OpeningBalanceDraft, SplitSize};

const MICRO: i64 = 1_000_000;
/// The longest id a transaction carries: a generated deduction's (`fee-` and 32 digits) and
/// a uuid are both shorter.
const TRANSACTION_ID_LENGTH: usize = 64;

/// What the user typed to name the account and the asset, and the shared values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The account's name as typed.
    pub account: String,
    /// The asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// Quantity in micro-units.
    pub quantity: i64,
}

/// A purchase or a sale's money side: a unit price or the broker's all-in total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeAmount {
    /// Unit price in the asset's currency (micro-units).
    Price(i64),
    /// All-in total in the account's currency (micro-units).
    Total(i64),
}

/// A purchase or a sale as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    /// Account, asset, date and quantity.
    pub target: Target,
    /// Unit price or total.
    pub amount: TradeAmount,
    /// Fees in the account's currency (micro-units).
    pub fees: i64,
    /// Exchange rate (micro-units).
    pub rate: i64,
    /// Optional note.
    pub note: Option<String>,
}

/// A deposit or a withdrawal as typed: cash of the account, so no asset and no quantity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashAmount {
    /// The account's name as typed.
    pub account: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// The amount in the account's currency (micro-units).
    pub amount: i64,
    /// Optional note.
    pub note: Option<String>,
}

/// A dividend as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DividendAmount {
    /// The account's name as typed.
    pub account: String,
    /// The paying asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// The amount received, in the asset's currency (micro-units).
    pub amount: i64,
    /// Exchange rate from the asset's currency to the account's (micro-units), or `None`
    /// for 1 (CLI-014).
    pub rate: Option<i64>,
    /// Optional note.
    pub note: Option<String>,
}

/// A stock split as typed: its size is a "new for old" ratio, as the window takes it
/// (SPL-062).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockSplit {
    /// The account's name as typed.
    pub account: String,
    /// The asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// Shares after, for `old` shares before.
    pub new: i64,
    /// Shares before.
    pub old: i64,
    /// Optional note.
    pub note: Option<String>,
}

/// Free shares as typed: a quantity received at no cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeShares {
    /// The account's name as typed.
    pub account: String,
    /// The distributing asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// How many shares were received (micro-units).
    pub quantity: i64,
    /// Optional note.
    pub note: Option<String>,
}

/// A write-off as typed: a quantity that left a holding without proceeds (WOF-020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOff {
    /// The account's name as typed.
    pub account: String,
    /// The asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// The quantity that left (micro-units).
    pub quantity: i64,
    /// Why it left.
    pub reason: WriteOffReason,
    /// Optional note.
    pub note: Option<String>,
}

/// Interest as typed: a credit to a held asset, or to the account's cash when no asset is
/// named (INT-023); exactly one of the two figures is given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterestCredit {
    /// The account's name as typed.
    pub account: String,
    /// The asset's name or reference as typed; `None` for the account's cash.
    pub asset: Option<String>,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// The share of what is held credited (micro-percent, INT-022).
    pub percent: Option<i64>,
    /// The quantity credited (micro-units).
    pub quantity: Option<i64>,
    /// Optional note.
    pub note: Option<String>,
}

/// A management fee as typed; exactly one of the two figures is given (FEE-021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagementFee {
    /// The account's name as typed.
    pub account: String,
    /// The charged asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// The share of what is held removed (micro-percent, FEE-022).
    pub percent: Option<i64>,
    /// What the holding holds after the fee (micro-units, FEE-028).
    pub resulting_quantity: Option<i64>,
    /// Optional note.
    pub note: Option<String>,
}

/// A price of an asset at a date, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceAt {
    /// The asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// The unit price in the asset's currency (micro-units).
    pub price: i64,
}

/// A price as recorded, with what names its asset for a person.
#[derive(Debug, Clone)]
pub struct RecordedPrice {
    /// The price as stored.
    pub price: AssetPrice,
    /// The asset's reference.
    pub asset_reference: String,
    /// The asset's currency.
    pub currency: String,
}

impl RecordedPrice {
    fn of(price: AssetPrice, asset: Asset) -> Self {
        Self {
            price,
            asset_reference: asset.reference,
            currency: asset.currency,
        }
    }
}

/// A command that records a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recording {
    /// An opening balance: quantity and total cost.
    Open {
        /// Account, asset, date and quantity.
        target: Target,
        /// What the position cost in all, in the account's currency (micro-units).
        total_cost: i64,
    },
    /// A purchase.
    Buy(Trade),
    /// A sale.
    Sell(Trade),
    /// A deposit of cash into the account (CSH-022).
    Deposit(CashAmount),
    /// A withdrawal of cash from the account (CSH-032).
    Withdrawal(CashAmount),
    /// A dividend a held asset paid (DIV-023).
    Dividend(DividendAmount),
    /// A stock split of a held asset (SPL-010).
    Split(StockSplit),
    /// Free shares of a held asset (FSD-022).
    FreeShares(FreeShares),
    /// A write-off of a held asset (WOF-020).
    WriteOff(WriteOff),
    /// Interest credited to a holding or to the cash (INT-023, INT-024).
    Interest(InterestCredit),
    /// A one-off management fee taken in units (FEE-022).
    ManagementFee(ManagementFee),
}

/// Which way cash moves.
enum Cash {
    PaidIn,
    TakenOut,
}

/// A recording on a holding: what `Recording` holds besides cash and dividends.
enum OnHolding {
    Open { target: Target, total_cost: i64 },
    Buy(Trade),
    Sell(Trade),
}

/// A refusal: a stable code a script can test and a sentence for a person (CLI-021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The code: the recording's own, or the asking interface's.
    pub code: String,
    /// The reason in plain English, naming only what the user typed.
    pub message: String,
}

/// What the owner was shown of a transaction before answering its removal or change (AGT-081,
/// AGT-088).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsShown {
    /// Its date.
    pub date: String,
    /// What kind of transaction.
    pub kind: TransactionType,
    /// Its quantity, in micro-units.
    pub quantity: i64,
    /// Its amount in the account's currency, in micro-units.
    pub amount: i64,
    /// Its unit price, in micro-units.
    pub unit_price: i64,
    /// Its fees, in micro-units.
    pub fees: i64,
    /// Its exchange rate, in micro-units.
    pub exchange_rate: i64,
    /// Its note.
    pub note: Option<String>,
}

impl AsShown {
    /// Whether `transaction` still stands as the owner was shown it: the same date, type,
    /// quantity and total (AGT-081, AGT-088).
    fn stands(&self, transaction: &Transaction) -> bool {
        (
            &transaction.date,
            transaction.transaction_type,
            transaction.shown_quantity(),
            transaction.total_amount,
        ) == (&self.date, self.kind, self.quantity, self.amount)
    }

    /// Whether `transaction` still stands in full as the owner was shown it before a change:
    /// what a removal checks, and its unit price, fees, rate and note, which a yes would
    /// otherwise overwrite (AGT-088).
    fn stands_in_full(&self, transaction: &Transaction) -> bool {
        self.stands(transaction)
            && (
                transaction.unit_price,
                transaction.fees,
                transaction.exchange_rate,
                &transaction.note,
            ) == (self.unit_price, self.fees, self.exchange_rate, &self.note)
    }
}

/// The refusal of an answer to a transaction that changed since the owner was shown it.
fn transaction_changed() -> Refusal {
    Refusal {
        code: "TransactionChanged".to_string(),
        message: "the transaction changed since the owner was shown it: ask again".to_string(),
    }
}

/// A transaction the owner typed, with what names it for a person (AGT-081).
#[derive(Debug, Clone)]
pub struct TypedTransaction {
    /// The transaction as stored.
    pub transaction: Transaction,
    /// The name of its account.
    pub account_name: String,
    /// The account's currency.
    pub currency: String,
    /// Its asset; `None` for a transaction on the account's cash.
    pub asset: Option<Asset>,
}

/// What was recorded, with what names it for a person.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// The transaction as recorded.
    pub transaction: Box<Transaction>,
    /// The account's name as stored.
    pub account_name: String,
    /// The account's currency.
    pub currency: String,
    /// The asset's reference; none for a deposit or a withdrawal, which are cash.
    pub asset_reference: Option<String>,
    /// An opening balance of zero cost (TRX-065).
    pub zero_cost: bool,
}

/// What an agent changes of a transaction it recorded (AGT-050): what is given replaces what
/// was recorded, the rest stays as it was.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Correction {
    /// The new date.
    pub date: Option<String>,
    /// The new quantity (micro-units).
    pub quantity: Option<i64>,
    /// The new unit price in the asset's currency (micro-units); never with `total`.
    pub price: Option<i64>,
    /// The new all-in total in the account's currency — for an opening balance, its total
    /// cost (micro-units); never with `price`.
    pub total: Option<i64>,
    /// The new fees (micro-units).
    pub fees: Option<i64>,
    /// The new exchange rate (micro-units).
    pub rate: Option<i64>,
    /// The new note.
    pub note: Option<String>,
}

/// What an agent asks to change of a transaction the owner typed (AGT-087): what is given
/// replaces what is stored, the rest stays. Which of these a transaction takes depends on its
/// type, as the window's correction of it does (TRX-031).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Change {
    /// What a purchase, a sale, an opening balance, free shares, a fee or interest changes,
    /// as `Correction` gives it: its date, quantity, price, total, fees, rate and note.
    pub correction: Correction,
    /// The amount of a deposit, a withdrawal or a dividend (micro-units).
    pub amount: Option<i64>,
    /// A split's shares after, for `old` before.
    pub new: Option<i64>,
    /// A split's shares before.
    pub old: Option<i64>,
}

/// The values the window's correction takes (TRX-031).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionFields {
    /// The date.
    pub date: String,
    /// The quantity (micro-units): a split's factor, a deposit's or a dividend's amount.
    pub quantity: i64,
    /// The unit price (micro-units).
    pub unit_price: i64,
    /// The exchange rate (micro-units).
    pub exchange_rate: i64,
    /// The fees (micro-units).
    pub fees: i64,
    /// A total kept as typed; `None` lets it follow the unit price.
    pub total: Option<i64>,
    /// The note.
    pub note: Option<String>,
}

/// A change of a transaction the owner typed, ready to be asked (AGT-087): the transaction
/// as it stands, as it would become, and what its correction takes.
#[derive(Debug, Clone)]
pub struct ChangeAsked {
    /// The transaction as it stands, with the names it is shown by.
    pub typed: TypedTransaction,
    /// The transaction as the correction would make it.
    pub becomes: Transaction,
    /// What the correction is given when the owner says yes.
    pub fields: CorrectionFields,
}

/// AGT-087 — the arguments a type of transaction takes beyond its date and note, as the
/// window's correction of it does.
fn changes_taken(kind: TransactionType) -> &'static [&'static str] {
    match kind {
        TransactionType::Purchase | TransactionType::Sell => {
            &["quantity", "price", "total", "fees", "rate"]
        }
        TransactionType::OpeningBalance => &["quantity", "total"],
        TransactionType::Deposit | TransactionType::Withdrawal => &["amount"],
        TransactionType::Dividend => &["amount", "rate"],
        TransactionType::Split => &["new", "old"],
        TransactionType::FreeShares
        | TransactionType::ManagementFee
        | TransactionType::Interest
        | TransactionType::WriteOff => &["quantity"],
        // TRF-031 — a part of a transfer is corrected as its transfer, never alone.
        TransactionType::TransferOut | TransactionType::TransferIn => &[],
    }
}

/// AGT-050 — the unit price and the total of a corrected purchase, sale or opening balance: a
/// stored total is kept while nothing it is made of changes; a quantity, fees or a rate that
/// change without a new price or total make it follow the unit price — except for an opening
/// balance, whose total cost only a new total changes.
fn price_and_total(stored: &Transaction, correction: &Correction) -> (i64, Option<i64>) {
    let follows_the_price =
        (correction.quantity.is_some() || correction.fees.is_some() || correction.rate.is_some())
            && stored.transaction_type != TransactionType::OpeningBalance;
    match (correction.price, correction.total) {
        (Some(price), _) => (price, None),
        (None, Some(total)) => (stored.unit_price, Some(total)),
        (None, None) if follows_the_price => (stored.unit_price, None),
        (None, None) => (stored.unit_price, Some(stored.total_amount)),
    }
}

/// AGT-087 — what a change can move of a transaction: equal before and after, nothing changes.
fn comparable(t: &Transaction) -> (&str, i64, i64, i64, i64, i64, Option<&str>) {
    (
        &t.date,
        t.shown_quantity(),
        t.total_amount,
        t.unit_price,
        t.fees,
        t.exchange_rate,
        t.note.as_deref(),
    )
}

/// An asset an account holds on an evening, as a preview tells it (AGT-072).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeldAsOf {
    /// The asset's reference.
    pub asset: String,
    /// The asset's name, which tells apart two assets sharing a reference (CLI-041).
    pub name: String,
    /// The quantity held (micro-units).
    pub quantity: i64,
    /// The average price, in the account's currency (micro-units).
    pub average_price: i64,
}

/// An account on the evening of a date: what it holds and its cash (AGT-072).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountAsOf {
    /// Each asset it holds then, by reference.
    pub holdings: Vec<HeldAsOf>,
    /// Its cash, in the account's currency (micro-units).
    pub cash: i64,
}

/// What a recording would do, nothing written (AGT-072).
#[derive(Debug, Clone)]
pub struct Previewed {
    /// The transaction as it would be recorded.
    pub transaction: Transaction,
    /// The account's name.
    pub account_name: String,
    /// The account's currency.
    pub currency: String,
    /// The recording's date.
    pub date: String,
    /// The account on the evening of that date without the recording.
    pub before: AccountAsOf,
    /// The account on the evening of that date with it.
    pub after: AccountAsOf,
}

/// A price previewed (AGT-072): as it would be recorded, and the one at its date it would
/// replace, if any.
#[derive(Debug, Clone)]
pub struct PricePreviewed {
    /// The price as it would be recorded.
    pub price: RecordedPrice,
    /// The price already at that date, which it would replace (MKT-103).
    pub replaced: Option<RecordedPrice>,
}

/// AGT-050 / AGT-087 — what the window's correction is given for a change an agent asks of
/// `stored`: refused for an argument its type does not take (a write-off's reason taken only
/// when `takes_reason`), for both a price and a total and for one part of a split's ratio;
/// what is given replaces what is stored, the rest stays.
fn change_fields(
    stored: &Transaction,
    change: &Change,
    reason: Option<WriteOffReason>,
    takes_reason: bool,
) -> Result<CorrectionFields, Refusal> {
    let kind = stored.transaction_type;
    let c = &change.correction;
    let given = [
        ("quantity", c.quantity.is_some()),
        ("price", c.price.is_some()),
        ("total", c.total.is_some()),
        ("fees", c.fees.is_some()),
        ("rate", c.rate.is_some()),
        ("amount", change.amount.is_some()),
        ("new", change.new.is_some()),
        ("old", change.old.is_some()),
        ("reason", reason.is_some()),
    ];
    let mut taken = changes_taken(kind).to_vec();
    if takes_reason && kind == TransactionType::WriteOff {
        taken.push("reason");
    }
    if given
        .iter()
        .any(|(name, is_given)| *is_given && !taken.contains(name))
    {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: format!(
                "this transaction takes date, note, {} and nothing else",
                taken.join(", ")
            ),
        });
    }
    if c.price.is_some() && c.total.is_some() {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: "give either price or total, not both".to_string(),
        });
    }
    let quantity = match kind {
        TransactionType::Deposit | TransactionType::Withdrawal | TransactionType::Dividend => {
            change.amount.unwrap_or(stored.shown_quantity())
        }
        TransactionType::Split => match (change.new, change.old) {
            (None, None) => stored.shown_quantity(),
            (Some(new), Some(old)) => SplitSize::Ratio {
                new: Some(new),
                old: Some(old),
            }
            .factor()
            .map_err(|error| refusal_from_account(&error))?,
            _ => {
                return Err(Refusal {
                    code: "InvalidArguments".to_string(),
                    message: "a split's ratio takes both new and old".to_string(),
                })
            }
        },
        _ => c
            .quantity
            .map(|quantity| stored.stored_correction_quantity(quantity))
            .unwrap_or(stored.quantity),
    };
    let (unit_price, total) = match kind {
        TransactionType::Purchase | TransactionType::Sell | TransactionType::OpeningBalance => {
            price_and_total(stored, c)
        }
        _ => (stored.unit_price, None),
    };
    Ok(CorrectionFields {
        date: c.date.clone().unwrap_or_else(|| stored.date.clone()),
        quantity,
        unit_price,
        exchange_rate: c.rate.unwrap_or(stored.exchange_rate),
        fees: c.fees.unwrap_or(stored.fees),
        total,
        note: c.note.clone().or_else(|| stored.note.clone()),
    })
}

/// What removing everything a session recorded did (AGT-053).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct SessionRemoval {
    /// How many transactions were removed.
    pub removed: u32,
    /// How many could not be: a transaction recorded since depends on each of them.
    pub kept: u32,
}

/// What a session recorded and that still exists (AGT-052).
#[derive(Debug, Clone)]
pub struct SessionRecordings {
    /// How many transactions.
    pub count: u32,
    /// The one recorded last, with its asset's reference.
    pub last: Option<(Transaction, String)>,
}

/// AGT-051 — what an agent reads for a transaction it may not touch. One sentence for a
/// transaction that does not exist and one the owner typed: the refusal says nothing of what
/// exists (AGT-042).
fn not_its_recording() -> Refusal {
    Refusal {
        code: "NotYourRecording".to_string(),
        message: "no agent recorded this transaction: an agent changes only what an agent recorded, and asks the owner for the rest with request_removal, request_change or request_transfer_replacement".to_string(),
    }
}

impl HoldingTransactionUseCase {
    /// AGT-051 / AGT-093 — the transaction `transaction_id` when an agent recorded it,
    /// whichever session; a refusal otherwise.
    pub async fn recorded_by_an_agent(&self, transaction_id: &str) -> Result<Transaction, Refusal> {
        // An id is a uuid: anything longer is no transaction, and is not looked up.
        if transaction_id.len() > TRANSACTION_ID_LENGTH {
            return Err(not_its_recording());
        }
        let mark = self
            .account_service
            .agent_recording_of(transaction_id)
            .await
            .map_err(|error| refusal_from_account(&error))?;
        if mark.is_none() {
            return Err(not_its_recording());
        }
        self.account_service
            .get_transaction_by_id(transaction_id)
            .await
            .map_err(|error| refusal_from_account(&error))?
            .ok_or_else(not_its_recording)
    }

    /// AGT-050 — an agent cancels a transaction an agent recorded, through the rules of the
    /// window.
    pub async fn cancel_recorded_by_agent(
        &self,
        transaction_id: &str,
    ) -> Result<Transaction, Refusal> {
        let transaction = self.recorded_by_an_agent(transaction_id).await?;
        self.cancel_transaction(&transaction.account_id, &transaction.id)
            .await
            .map_err(|error| refusal_from_account(&error))?;
        Ok(transaction)
    }

    /// AGT-080 — the transaction `transaction_id` when the owner typed it, with the names it
    /// is shown by: refused when no transaction has that id, or when an agent recorded it.
    pub async fn typed_by_owner(&self, transaction_id: &str) -> Result<TypedTransaction, Refusal> {
        let not_found = || Refusal {
            code: "TransactionNotFound".to_string(),
            message: "no transaction has this id: list_transactions gives each one's".to_string(),
        };
        // An id is a uuid: anything longer is no transaction, and is not looked up.
        if transaction_id.len() > TRANSACTION_ID_LENGTH {
            return Err(not_found());
        }
        let transaction = self
            .account_service
            .get_transaction_by_id(transaction_id)
            .await
            .map_err(|error| refusal_from_account(&error))?
            .ok_or_else(not_found)?;
        let mark = self
            .account_service
            .agent_recording_of(transaction_id)
            .await
            .map_err(|error| refusal_from_account(&error))?;
        if mark.is_some() {
            return Err(Refusal {
                code: "NotTypedByOwner".to_string(),
                message: "an agent recorded this transaction: change it with cancel_recording or correct_recording, without asking".to_string(),
            });
        }
        let account = self
            .account_service
            .get_by_id(&transaction.account_id)
            .await
            .map_err(|error| refusal_from_account(&error))?
            .ok_or_else(not_found)?;
        let asset = self
            .asset_service
            .get_asset_by_id(&transaction.asset_id)
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "typed_by_owner: the asset could not be read");
                refusal_from_account(&AccountError::DatabaseError)
            })?
            .filter(|asset| asset.class != AssetClass::Cash);
        Ok(TypedTransaction {
            transaction,
            account_name: account.name,
            currency: account.currency,
            asset,
        })
    }

    /// AGT-080 — the transaction the owner typed that an agent may ask to remove: refused as
    /// `typed_by_owner` refuses, and with `PartOfTransfer` for a part of a transfer, whose
    /// deletion would remove the whole transfer, more than the owner is shown (TRF-040).
    pub async fn removable_by_owner(
        &self,
        transaction_id: &str,
    ) -> Result<TypedTransaction, Refusal> {
        let typed = self.typed_by_owner(transaction_id).await?;
        if typed.transaction.is_transfer_part() {
            return Err(refusal_from_account(&AccountError::PartOfTransfer));
        }
        Ok(typed)
    }

    /// AGT-080 — removes a transaction the owner typed, on the owner's answer, through the
    /// deletion of the window (TRX-034): refused as `removable_by_owner` refuses, with
    /// `TransactionChanged` when it no longer stands as the owner was shown it, or as the
    /// deletion refuses.
    pub async fn remove_typed_by_owner(
        &self,
        transaction_id: &str,
        shown: &AsShown,
    ) -> Result<Transaction, Refusal> {
        let typed = self.removable_by_owner(transaction_id).await?;
        if !shown.stands(&typed.transaction) {
            return Err(transaction_changed());
        }
        self.cancel_transaction(&typed.transaction.account_id, &typed.transaction.id)
            .await
            .map_err(|error| refusal_from_account(&error))?;
        Ok(typed.transaction)
    }

    /// AGT-087 — what an agent asks to change of a transaction the owner typed, checked
    /// before the owner is asked: refused as `typed_by_owner` refuses, for an argument its
    /// type does not take, and as the window's correction refuses, with nothing written.
    pub async fn ask_change_of_typed(
        &self,
        transaction_id: &str,
        change: Change,
    ) -> Result<ChangeAsked, Refusal> {
        if change == Change::default() {
            return Err(Refusal {
                code: "MissingArgument".to_string(),
                message: "nothing to change: give what changes".to_string(),
            });
        }
        let typed = self.typed_by_owner(transaction_id).await?;
        let stored = &typed.transaction;
        let fields = change_fields(stored, &change, None, false)?;
        let becomes = self
            .account_service
            .preview_correction(
                &stored.account_id,
                &stored.id,
                fields.date.clone(),
                fields.quantity,
                fields.unit_price,
                fields.exchange_rate,
                fields.fees,
                fields.total,
                fields.note.clone(),
            )
            .await
            .map_err(|error| refusal_from_account(&error))?;
        let becomes = becomes.transaction;
        if comparable(&becomes) == comparable(stored) {
            return Err(Refusal {
                code: "MissingArgument".to_string(),
                message: "nothing to change: what was given is what the transaction holds"
                    .to_string(),
            });
        }
        Ok(ChangeAsked {
            typed,
            becomes,
            fields,
        })
    }

    /// AGT-088 — changes a transaction the owner typed, on the owner's answer, through the
    /// correction of the window (TRX-031): refused as `typed_by_owner` refuses, with
    /// `TransactionChanged` when it no longer stands as the owner was shown it, or as the
    /// correction refuses.
    pub async fn change_typed_by_owner(
        &self,
        transaction_id: &str,
        shown: &AsShown,
        fields: CorrectionFields,
    ) -> Result<Transaction, Refusal> {
        let typed = self.typed_by_owner(transaction_id).await?;
        if !shown.stands_in_full(&typed.transaction) {
            return Err(transaction_changed());
        }
        self.correct_transaction(
            &typed.transaction.account_id,
            &typed.transaction.id,
            fields.date,
            fields.quantity,
            fields.unit_price,
            fields.exchange_rate,
            fields.fees,
            fields.total,
            fields.note,
        )
        .await
        .map_err(|error| refusal_from_account(&error))
    }

    /// AGT-050 — an agent corrects a transaction an agent recorded, whatever its type,
    /// through the correction the window calls for it (TRX-031; WOF-030 for a write-off,
    /// whose reason it alone takes): what `change` gives replaces what was recorded.
    pub async fn correct_recorded_by_agent(
        &self,
        transaction_id: &str,
        change: Change,
        reason: Option<WriteOffReason>,
    ) -> Result<Transaction, Refusal> {
        let recorded = self.recorded_by_an_agent(transaction_id).await?;
        // AGT-065A — a part of a transfer is corrected with its transfer, never alone (TRF-031).
        if recorded.is_transfer_part() {
            return Err(refusal_from_account(&AccountError::PartOfTransfer));
        }
        let fields = change_fields(&recorded, &change, reason, true)?;
        let corrected = if recorded.transaction_type == TransactionType::WriteOff {
            // A stored write-off always carries its reason (WOF entity): one without is a
            // portfolio that cannot be read as written.
            let reason = reason.or(recorded.write_off_reason).ok_or_else(|| {
                tracing::error!(target: BACKEND, transaction = %recorded.id, "agent correcting: a write-off without its reason");
                refusal_from_account(&AccountError::DatabaseError)
            })?;
            self.correct_write_off(
                &recorded.account_id,
                &recorded.id,
                fields.date,
                fields.quantity,
                reason,
                fields.note,
            )
            .await
        } else {
            self.correct_transaction(
                &recorded.account_id,
                &recorded.id,
                fields.date,
                fields.quantity,
                fields.unit_price,
                fields.exchange_rate,
                fields.fees,
                fields.total,
                fields.note,
            )
            .await
        };
        corrected.map_err(|error| refusal_from_account(&error))
    }

    /// AGT-073 — what `correct_recorded_by_agent` would do, nothing written: the same
    /// refusals, and the account on the evening of the corrected date without the
    /// correction and with it.
    pub async fn preview_correction_by_agent(
        &self,
        transaction_id: &str,
        change: Change,
        reason: Option<WriteOffReason>,
    ) -> Result<Previewed, Refusal> {
        let recorded = self.recorded_by_an_agent(transaction_id).await?;
        // AGT-065A — a part of a transfer is corrected with its transfer, never alone (TRF-031).
        if recorded.is_transfer_part() {
            return Err(refusal_from_account(&AccountError::PartOfTransfer));
        }
        let fields = change_fields(&recorded, &change, reason, true)?;
        // Read for its name and currency only: the preview loads the account again, and its
        // own load decides what the correction makes of it.
        let account = self
            .account_service
            .get_by_id(&recorded.account_id)
            .await
            .map_err(|error| refusal_from_account(&error))?
            .ok_or_else(|| {
                refusal_from_account(&AccountError::AccountNotFound {
                    account_id: recorded.account_id.clone(),
                })
            })?;
        let date = fields.date.clone();
        let mut previewed = self
            .account_service
            .preview_correction(
                &recorded.account_id,
                &recorded.id,
                fields.date,
                fields.quantity,
                fields.unit_price,
                fields.exchange_rate,
                fields.fees,
                fields.total,
                fields.note,
            )
            .await
            .map_err(|error| refusal_from_account(&error))?;
        // WOF-030 — the reason moves no figure: the write-off shows the one it would carry.
        if reason.is_some() {
            previewed.transaction.write_off_reason = reason;
        }
        Ok(self
            .told(previewed, account.name, account.currency, date)
            .await)
    }

    /// AGT-072 — what `record_price_named_by_agent` would do, nothing written: the same
    /// refusals, the price as it would be recorded and the one already at that date it would
    /// replace.
    pub async fn preview_price_named(
        &self,
        typed: PriceAt,
        today: &str,
    ) -> Result<PricePreviewed, Refusal> {
        let (asset, date) = self.priced(&typed, today).await?;
        let (price, replaced) = self
            .asset_service
            .preview_price_by_agent(&asset.id, &date, typed.price)
            .await
            .map_err(|error| refusal_from_asset(&error))?;
        Ok(PricePreviewed {
            price: RecordedPrice::of(price, asset.clone()),
            replaced: replaced.map(|price| RecordedPrice::of(price, asset)),
        })
    }

    /// AGT-052 — what the session `session` recorded and that still exists: how many
    /// transactions, and the one it recorded last.
    pub async fn recorded_in_session(
        &self,
        session: &str,
    ) -> Result<SessionRecordings, AccountError> {
        let recordings = self.account_service.recorded_in_session(session).await?;
        let count = u32::try_from(recordings.len()).unwrap_or(u32::MAX);
        let Some(newest) = recordings.first() else {
            return Ok(SessionRecordings { count, last: None });
        };
        let last = match self
            .account_service
            .get_transaction_by_id(&newest.transaction_id)
            .await?
        {
            None => None,
            Some(transaction) => {
                let reference = match self
                    .asset_service
                    .get_asset_by_id(&transaction.asset_id)
                    .await
                {
                    Ok(Some(asset)) => asset.reference,
                    Ok(None) | Err(_) => transaction.asset_id.clone(),
                };
                Some((transaction, reference))
            }
        };
        Ok(SessionRecordings { count, last })
    }

    /// AGT-053 — removes every transaction the session `session` recorded and that still
    /// exists, the last recorded first. One that a later transaction depends on is tried again once the
    /// others are gone, and kept when it still cannot go.
    pub async fn remove_recorded_in_session(
        &self,
        session: &str,
    ) -> Result<SessionRemoval, AccountError> {
        let mut left = self.account_service.recorded_in_session(session).await?;
        let mut removed = 0u32;
        loop {
            let before = left.len();
            let mut kept = Vec::new();
            for recording in left {
                match self
                    .cancel_transaction(&recording.account_id, &recording.transaction_id)
                    .await
                {
                    Ok(()) => removed += 1,
                    // Gone already: removed with the transfer it was a part of (TRF-040),
                    // or meanwhile, by the agent or by the owner.
                    Err(AccountError::TransactionNotFound | AccountError::TransferNotFound) => {
                        if recording.transfer_part {
                            removed += 1;
                        }
                    }
                    Err(AccountError::DatabaseError) => return Err(AccountError::DatabaseError),
                    Err(refusal) => {
                        tracing::info!(target: BACKEND, transaction = %recording.transaction_id, err = ?refusal, "agent recordings: one kept for now");
                        kept.push(recording);
                    }
                }
            }
            left = kept;
            // A pass that removed nothing will not do better: what is left depends on
            // transactions the session did not record.
            if left.is_empty() || left.len() == before {
                break;
            }
        }
        Ok(SessionRemoval {
            removed,
            kept: u32::try_from(left.len()).unwrap_or(u32::MAX),
        })
    }

    /// CLI-011 / CLI-012 — records what a person named: the account and the asset found by
    /// what was typed, then the transaction through the rules of the window. `today` is the
    /// date used when the recording gives none.
    pub async fn record_named(&self, command: Recording, today: &str) -> Result<Recorded, Refusal> {
        match command {
            Recording::Open { target, total_cost } => {
                self.record_on_holding(OnHolding::Open { target, total_cost }, today)
                    .await
            }
            Recording::Buy(trade) => self.record_on_holding(OnHolding::Buy(trade), today).await,
            Recording::Sell(trade) => self.record_on_holding(OnHolding::Sell(trade), today).await,
            Recording::Deposit(cash) => self.record_cash(cash, Cash::PaidIn, today).await,
            Recording::Withdrawal(cash) => self.record_cash(cash, Cash::TakenOut, today).await,
            Recording::Dividend(dividend) => self.record_dividend_named(dividend, today).await,
            Recording::Split(split) => self.record_split_named(split, today).await,
            Recording::FreeShares(free_shares) => {
                self.record_free_shares_named(free_shares, today).await
            }
            Recording::WriteOff(write_off) => self.record_write_off_named(write_off, today).await,
            Recording::Interest(interest) => self.record_interest_named(interest, today).await,
            Recording::ManagementFee(fee) => self.record_management_fee_named(fee, today).await,
        }
    }

    /// AGT-072 — what recording `command` would do, nothing written: the same lookups and
    /// checks as `record_named`, the account's recording applied in memory, and the account
    /// on the evening of the date without it and with it.
    pub async fn preview_named(
        &self,
        command: Recording,
        today: &str,
    ) -> Result<Previewed, Refusal> {
        match command {
            Recording::Open { target, total_cost } => {
                self.preview_found(
                    &target.account,
                    &target.asset,
                    target.date.clone(),
                    today,
                    |found, date| async move {
                        self.preview_opening(
                            &found.account_id,
                            found.asset_id.clone(),
                            date,
                            from_surface(target.quantity),
                            total_cost,
                        )
                        .await
                        .map_err(|error| refusal_from_open(&error))
                    },
                )
                .await
            }
            Recording::Buy(trade) => {
                let (account, asset) = (trade.target.account.clone(), trade.target.asset.clone());
                self.preview_found(
                    &account,
                    &asset,
                    trade.target.date.clone(),
                    today,
                    |found, date| async move {
                        self.preview_buy(
                            &found.account_id,
                            trade_op(&trade, found.asset_id.clone(), date),
                        )
                        .await
                        .map_err(|error| refusal_from_account(&error))
                    },
                )
                .await
            }
            Recording::Sell(trade) => {
                let (account, asset) = (trade.target.account.clone(), trade.target.asset.clone());
                self.preview_found(
                    &account,
                    &asset,
                    trade.target.date.clone(),
                    today,
                    |found, date| async move {
                        self.preview_sell(
                            &found.account_id,
                            trade_op(&trade, found.asset_id.clone(), date),
                        )
                        .await
                        .map_err(|error| refusal_from_account(&error))
                    },
                )
                .await
            }
            Recording::Split(split) => {
                let ratio = SplitSize::Ratio {
                    new: Some(split.new),
                    old: Some(split.old),
                };
                self.preview_found(
                    &split.account,
                    &split.asset,
                    split.date.clone(),
                    today,
                    |found, date| async move {
                        let factor = ratio
                            .factor()
                            .map_err(|error| refusal_from_account(&error))?;
                        self.preview_split_recording(
                            &found.account_id,
                            found.asset_id.clone(),
                            date,
                            factor,
                            split.note,
                        )
                        .await
                        .map_err(|error| refusal_from_split(&error))
                    },
                )
                .await
            }
            Recording::Deposit(cash) => self.preview_cash_named(cash, Cash::PaidIn, today).await,
            Recording::Withdrawal(cash) => {
                self.preview_cash_named(cash, Cash::TakenOut, today).await
            }
            Recording::Dividend(dividend) => {
                let (account, asset) = (dividend.account.clone(), dividend.asset.clone());
                self.preview_found(
                    &account,
                    &asset,
                    dividend.date.clone(),
                    today,
                    |found, date| async move {
                        self.preview_dividend(
                            &found.account_id,
                            found.asset_id.clone(),
                            date,
                            dividend.amount,
                            dividend.rate.unwrap_or(MICRO),
                            dividend.note,
                        )
                        .await
                        .map_err(|error| refusal_from_dividend(&error))
                    },
                )
                .await
            }
            Recording::FreeShares(free_shares) => {
                let (account, asset) = (free_shares.account.clone(), free_shares.asset.clone());
                self.preview_found(
                    &account,
                    &asset,
                    free_shares.date.clone(),
                    today,
                    |found, date| async move {
                        self.preview_free_shares(
                            &found.account_id,
                            found.asset_id.clone(),
                            date,
                            from_surface(free_shares.quantity),
                            free_shares.note,
                        )
                        .await
                        .map_err(|error| refusal_from_free_shares(&error))
                    },
                )
                .await
            }
            Recording::WriteOff(write_off) => {
                let (account, asset) = (write_off.account.clone(), write_off.asset.clone());
                self.preview_found(
                    &account,
                    &asset,
                    write_off.date.clone(),
                    today,
                    |found, date| async move {
                        self.preview_write_off(
                            &found.account_id,
                            found.asset_id.clone(),
                            date,
                            from_surface(write_off.quantity),
                            write_off.reason,
                            write_off.note,
                        )
                        .await
                        .map_err(|error| refusal_from_write_off(&error))
                    },
                )
                .await
            }
            // AGT-066 — `preview_recording` does not preview interest or a fee (AGT-072).
            Recording::Interest(_) | Recording::ManagementFee(_) => Err(Refusal {
                code: "InvalidArguments".to_string(),
                message: "interest and management fees are not previewed".to_string(),
            }),
        }
    }

    /// AGT-072 — the account and the asset found by name, `preview` run on them at the date
    /// (today when none), and the account told without the recording and with it.
    async fn preview_found<F, Fut>(
        &self,
        account: &str,
        asset: &str,
        date: Option<String>,
        today: &str,
        preview: F,
    ) -> Result<Previewed, Refusal>
    where
        F: FnOnce(NamedTarget, String) -> Fut,
        Fut: std::future::Future<Output = Result<RecordingPreview, Refusal>>,
    {
        let found = self
            .find_by_name(account, asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = date.unwrap_or_else(|| today.to_string());
        let (account_name, currency) = (found.account_name.clone(), found.currency.clone());
        let previewed = preview(found, date.clone()).await?;
        Ok(self.told(previewed, account_name, currency, date).await)
    }

    /// AGT-072 — a deposit or a withdrawal previewed on the account named, as `record_cash`
    /// records it.
    async fn preview_cash_named(
        &self,
        cash: CashAmount,
        way: Cash,
        today: &str,
    ) -> Result<Previewed, Refusal> {
        let account = self
            .find_account_by_name(&cash.account)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = cash.date.unwrap_or_else(|| today.to_string());
        let op = match way {
            Cash::PaidIn => RecordingOp::Deposit {
                date: date.clone(),
                amount: cash.amount,
                note: cash.note,
            },
            Cash::TakenOut => RecordingOp::Withdrawal {
                date: date.clone(),
                amount: cash.amount,
                note: cash.note,
            },
        };
        let previewed = self
            .preview_cash(&account.id, op)
            .await
            .map_err(|error| refusal_from_account(&error))?;
        Ok(self
            .told(previewed, account.name, account.currency, date)
            .await)
    }

    /// AGT-072 — a preview as the agent is told it: the account without the recording and
    /// with it, on the evening of `date`.
    async fn told(
        &self,
        previewed: RecordingPreview,
        account_name: String,
        currency: String,
        date: String,
    ) -> Previewed {
        Previewed {
            transaction: previewed.transaction,
            account_name,
            currency,
            before: self.account_as_of(&previewed.before, &date).await,
            after: self.account_as_of(&previewed.after, &date).await,
            date,
        }
    }

    /// AGT-065A — the sending account, the receiving account and the asset a transfer names,
    /// each found as a recording finds its own (CLI-011), refused as it is.
    pub async fn find_transfer_by_name(
        &self,
        from: &str,
        to: &str,
        asset: &str,
    ) -> Result<(NamedTarget, NamedTarget), Refusal> {
        let sending = self
            .find_by_name(from, asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let receiving = self
            .find_account_by_name(to)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let receiving = NamedTarget {
            account_id: receiving.id,
            account_name: receiving.name,
            currency: receiving.currency,
            asset_id: sending.asset_id.clone(),
            asset_reference: sending.asset_reference.clone(),
        };
        Ok((sending, receiving))
    }

    /// AGT-072 — the account `transactions` make on the evening of `date`: each asset held,
    /// by reference, and the cash.
    pub async fn account_as_of(&self, transactions: &[Transaction], date: &str) -> AccountAsOf {
        let mut assets: Vec<&str> = transactions.iter().map(|tx| tx.asset_id.as_str()).collect();
        assets.sort_unstable();
        assets.dedup();
        let mut holdings = Vec::new();
        // The cash moves with every transaction that brings or takes it, not only those on
        // the cash line: an account of opening balances alone has none there (DEBT-135).
        let cash = Account::cash_balance_as_of(transactions, date);
        for asset_id in assets {
            if crate::core::cash::is_cash_asset(asset_id) {
                continue;
            }
            let snapshot = Account::holding_snapshot_as_of(transactions, asset_id, date);
            if snapshot.quantity == 0 {
                continue;
            }
            let (reference, name) = match self.asset_service.get_asset_by_id(asset_id).await {
                Ok(Some(asset)) => (asset.reference, asset.name),
                Ok(None) => (asset_id.to_string(), String::new()),
                Err(error) => {
                    tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?error, "preview: the asset could not be read");
                    (asset_id.to_string(), String::new())
                }
            };
            holdings.push(HeldAsOf {
                asset: reference,
                name,
                quantity: to_surface(snapshot.quantity),
                average_price: snapshot.average_price,
            });
        }
        holdings.sort_by(|a, b| a.asset.cmp(&b.asset));
        AccountAsOf { holdings, cash }
    }

    /// AGT-063 — a split: the account and the asset found by what was typed, the factor
    /// its ratio gives (SPL-062), the split recorded through the rules of the window.
    async fn record_split_named(
        &self,
        split: StockSplit,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let found = self
            .find_by_name(&split.account, &split.asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let factor = SplitSize::Ratio {
            new: Some(split.new),
            old: Some(split.old),
        }
        .factor()
        .map_err(|error| refusal_from_account(&error))?;
        let date = split.date.unwrap_or_else(|| today.to_string());
        self.record_split(
            &found.account_id,
            found.asset_id.clone(),
            date,
            factor,
            split.note,
        )
        .await
        .map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: Some(found.asset_reference),
            zero_cost: false,
        })
        .map_err(|error| refusal_from_split(&error))
    }

    /// AGT-063 — free shares: the account and the distributing asset found by what was
    /// typed, the distribution recorded through the rules of the window.
    async fn record_free_shares_named(
        &self,
        free_shares: FreeShares,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let found = self
            .find_by_name(&free_shares.account, &free_shares.asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = free_shares.date.unwrap_or_else(|| today.to_string());
        self.record_free_shares(
            &found.account_id,
            found.asset_id.clone(),
            date,
            from_surface(free_shares.quantity),
            free_shares.note,
        )
        .await
        .map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: Some(found.asset_reference),
            zero_cost: false,
        })
        .map_err(|error| refusal_from_free_shares(&error))
    }

    /// AGT-066 — interest: the account and the asset found by what was typed, or the
    /// account's cash when no asset is named (INT-023), credited through the rules of the
    /// window.
    async fn record_interest_named(
        &self,
        interest: InterestCredit,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let (account_id, account_name, currency, asset_id, asset_reference) = match &interest.asset
        {
            Some(asset) => {
                let found = self
                    .find_by_name(&interest.account, asset)
                    .await
                    .map_err(|error| refusal_from_lookup(&error))?;
                (
                    found.account_id,
                    found.account_name,
                    found.currency,
                    found.asset_id,
                    Some(found.asset_reference),
                )
            }
            None => {
                let account = self
                    .find_account_by_name(&interest.account)
                    .await
                    .map_err(|error| refusal_from_lookup(&error))?;
                self.ensure_cash_for(&account.id, "record_interest")
                    .await
                    .map_err(|error| refusal_from_account(&error))?;
                let cash = account.cash_asset_id();
                (account.id, account.name, account.currency, cash, None)
            }
        };
        let date = interest.date.unwrap_or_else(|| today.to_string());
        // INT-023 — on the account's cash line the credit is money, in its own scale.
        let credited = interest
            .quantity
            .map(|quantity| crate::shared::domain::quantity::stored_for(&asset_id, quantity));
        self.record_interest(
            &account_id,
            asset_id,
            date,
            interest.percent,
            credited,
            interest.note,
        )
        .await
        .map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name,
            currency,
            asset_reference,
            zero_cost: false,
        })
        .map_err(|error| refusal_from_interest(&error))
    }

    /// AGT-066 — a management fee: the account and the asset found by what was typed, the
    /// fee recorded through the rules of the window, unless the holding's schedule already
    /// generated the fee of its period (AGT-067).
    async fn record_management_fee_named(
        &self,
        fee: ManagementFee,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let found = self
            .find_by_name(&fee.account, &fee.asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = fee.date.unwrap_or_else(|| today.to_string());
        let generated = self
            .account_service
            .has_generated_fee_for_period(&found.account_id, &found.asset_id, &date)
            .await
            .map_err(|error| refusal_from_account(&error))?;
        if generated {
            return Err(Refusal {
                code: "GeneratedFeeExists".to_string(),
                message: "the holding's fee schedule already generated the fee of this period: ask to change that fee with request_change instead".to_string(),
            });
        }
        self.record_management_fee_entry(
            &found.account_id,
            found.asset_id.clone(),
            date,
            fee.percent,
            fee.resulting_quantity.map(from_surface),
            fee.note,
        )
        .await
        .map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: Some(found.asset_reference),
            zero_cost: false,
        })
        .map_err(|error| refusal_from_management_fee(&error))
    }

    /// AGT-065 — a write-off: the account and the asset found by what was typed, the
    /// write-off recorded through the rules of the window.
    async fn record_write_off_named(
        &self,
        write_off: WriteOff,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let found = self
            .find_by_name(&write_off.account, &write_off.asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = write_off.date.unwrap_or_else(|| today.to_string());
        self.record_write_off(
            &found.account_id,
            found.asset_id.clone(),
            date,
            from_surface(write_off.quantity),
            write_off.reason,
            write_off.note,
        )
        .await
        .map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: Some(found.asset_reference),
            zero_cost: false,
        })
        .map_err(|error| refusal_from_write_off(&error))
    }

    /// CLI-027 — a deposit or a withdrawal: the account found by its name, the cash moved
    /// through the rules of the window.
    async fn record_cash(
        &self,
        cash: CashAmount,
        way: Cash,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let account = self
            .find_account_by_name(&cash.account)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = cash.date.unwrap_or_else(|| today.to_string());
        let recorded = match way {
            Cash::PaidIn => {
                self.record_deposit(&account.id, date, cash.amount, cash.note)
                    .await
            }
            Cash::TakenOut => {
                self.record_withdrawal(&account.id, date, cash.amount, cash.note)
                    .await
            }
        };
        recorded
            .map(|transaction| Recorded {
                transaction: Box::new(transaction),
                account_name: account.name,
                currency: account.currency,
                asset_reference: None,
                zero_cost: false,
            })
            .map_err(|error| refusal_from_account(&error))
    }

    /// CLI-028 — a dividend: the account and the paying asset found by what was typed, the
    /// dividend recorded through the rules of the window.
    async fn record_dividend_named(
        &self,
        dividend: DividendAmount,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let found = self
            .find_by_name(&dividend.account, &dividend.asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = dividend.date.unwrap_or_else(|| today.to_string());
        self.record_dividend(
            &found.account_id,
            found.asset_id.clone(),
            date,
            dividend.amount,
            dividend.rate.unwrap_or(MICRO),
            dividend.note,
        )
        .await
        .map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: Some(found.asset_reference),
            zero_cost: false,
        })
        .map_err(|error| refusal_from_dividend(&error))
    }

    async fn record_on_holding(
        &self,
        command: OnHolding,
        today: &str,
    ) -> Result<Recorded, Refusal> {
        let target = match &command {
            OnHolding::Open { target, .. } => target,
            OnHolding::Buy(trade) | OnHolding::Sell(trade) => &trade.target,
        };
        let found = match self.find_by_name(&target.account, &target.asset).await {
            Ok(found) => found,
            Err(error) => return Err(refusal_from_lookup(&error)),
        };
        let date = target.date.clone().unwrap_or_else(|| today.to_string());
        // TRX-065 — the core says whether an opening balance calls for the warning.
        let zero_cost = match &command {
            OnHolding::Open { target, total_cost } => self
                .validate_opening_balance_draft(&OpeningBalanceDraft {
                    account_id: found.account_id.clone(),
                    asset_id: found.asset_id.clone(),
                    date: date.clone(),
                    quantity: from_surface(target.quantity),
                    total_cost: Some(*total_cost),
                })
                .is_ok_and(|preview| preview.zero_cost),
            OnHolding::Buy(_) | OnHolding::Sell(_) => false,
        };
        let result = match command {
            OnHolding::Open { target, total_cost } => self
                .open_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    from_surface(target.quantity),
                    total_cost,
                )
                .await
                .map_err(|error| refusal_from_open(&error)),
            OnHolding::Buy(trade) => {
                let (unit_price, total) = split(&trade);
                self.buy_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    from_surface(trade.target.quantity),
                    unit_price,
                    trade.rate,
                    trade.fees,
                    total,
                    trade.note,
                )
                .await
                .map_err(|error| refusal_from_account(&error))
            }
            OnHolding::Sell(trade) => {
                let (unit_price, total) = split(&trade);
                self.sell_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    from_surface(trade.target.quantity),
                    unit_price,
                    trade.rate,
                    trade.fees,
                    total,
                    trade.note,
                )
                .await
                .map_err(|error| refusal_from_account(&error))
            }
        };
        result.map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: Some(found.asset_reference),
            zero_cost,
        })
    }

    /// CLI-029 — records the price the owner typed for the asset named: it replaces a price
    /// already at that date, as in the window (MKT-025).
    pub async fn record_price_named(
        &self,
        typed: PriceAt,
        today: &str,
    ) -> Result<RecordedPrice, Refusal> {
        let (asset, date) = self.priced(&typed, today).await?;
        self.asset_service
            .record_asset_price_in_micros(&asset.id, &date, typed.price)
            .await
            .map(|price| RecordedPrice::of(price, asset))
            .map_err(|error| refusal_from_asset(&error))
    }

    /// AGT-062 — records the price an agent gave for the asset named, with the source
    /// `Agent`; it replaces a price already at that date, like any price (MKT-103).
    pub async fn record_price_named_by_agent(
        &self,
        typed: PriceAt,
        today: &str,
    ) -> Result<RecordedPrice, Refusal> {
        let (asset, date) = self.priced(&typed, today).await?;
        self.asset_service
            .record_asset_price_by_agent(&asset.id, &date, typed.price)
            .await
            .map(|price| RecordedPrice::of(price, asset))
            .map_err(|error| refusal_from_asset(&error))
    }

    /// CLI-011 — the asset a price names and the date it is for.
    async fn priced(&self, typed: &PriceAt, today: &str) -> Result<(Asset, String), Refusal> {
        let asset = self
            .find_asset_by_name(&typed.asset)
            .await
            .map_err(|error| refusal_from_lookup(&error))?;
        let date = typed.date.clone().unwrap_or_else(|| today.to_string());
        Ok((asset, date))
    }

    /// AGT-044 / AGT-049 — records what an agent named, marked with the agent and its
    /// session. When the mark cannot be written the recording is undone, and the refusal
    /// says so.
    pub async fn record_named_by_agent(
        &self,
        command: Recording,
        today: &str,
        mark: &AgentRecording,
    ) -> Result<Recorded, Refusal> {
        let recorded = self.record_named(command, today).await?;
        let transaction = &recorded.transaction;
        let Err(error) = self
            .account_service
            .mark_agent_recording(&transaction.id, mark)
            .await
        else {
            return Ok(recorded);
        };
        tracing::error!(target: BACKEND, transaction = %transaction.id, err = ?error, "agent recording: not marked, undoing it");
        match self
            .cancel_transaction(&transaction.account_id, &transaction.id)
            .await
        {
            Ok(()) => Err(Refusal {
                code: "NotRecorded".to_string(),
                message: "the recording could not be marked as an agent's, so nothing was recorded: try again".to_string(),
            }),
            Err(error) => {
                tracing::error!(target: BACKEND, transaction = %transaction.id, err = ?error, "agent recording: not marked and not undone");
                Err(Refusal {
                    code: "RecordedNotMarked".to_string(),
                    message: "the transaction was recorded, but could not be marked as recorded by an agent: tell the owner".to_string(),
                })
            }
        }
    }
}

/// A figure in micro-units as a decimal without trailing zeros (`2`, `992.19`, `0.5`).
pub fn decimal(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    let absolute = micros.unsigned_abs();
    let whole = absolute / 1_000_000;
    let fraction = absolute % 1_000_000;
    if fraction == 0 {
        return format!("{sign}{whole}");
    }
    let fraction = format!("{fraction:06}");
    format!("{sign}{whole}.{}", fraction.trim_end_matches('0'))
}

/// A decimal with a dot and at most six decimals, in micro-units; `None` when it is not one.
/// A leading minus is read, so the core refuses a negative figure with its own reason.
pub fn decimal_to_micro(text: &str) -> Option<i64> {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty()
        || fraction.len() > 6
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !fraction.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: i64 = format!("{fraction:0<6}").parse().ok()?;
    let micros = whole.checked_mul(MICRO)?.checked_add(fraction)?;
    Some(if negative { -micros } else { micros })
}

/// CLI-011 — a name the use case could not resolve, naming only what was typed, with any
/// control character escaped: the sentence is printed to a terminal and read by an agent.
pub(super) fn refusal_from_lookup(error: &NameLookupError) -> Refusal {
    let message = match error {
        NameLookupError::AccountNotFound { typed } => {
            format!("no account named \"{}\"", typed.escape_debug())
        }
        NameLookupError::AccountAmbiguous { typed } => {
            format!(
                "more than one account is named \"{}\"",
                typed.escape_debug()
            )
        }
        NameLookupError::AssetNotFound { typed } => {
            format!(
                "no asset matches \"{}\" by name, reference or ISIN",
                typed.escape_debug()
            )
        }
        NameLookupError::AssetAmbiguous { typed } => {
            let typed = typed.escape_debug();
            format!(
                "\"{typed}\" matches more than one asset — use its name, its ISIN, or its reference with its exchange (ASML@XAMS)"
            )
        }
        NameLookupError::DatabaseError => "the portfolio could not be read".to_string(),
    };
    Refusal {
        code: code_of(error),
        message,
    }
}

/// A purchase or a sale as the account records it, at `date` on `asset_id`.
fn trade_op(trade: &Trade, asset_id: String, date: String) -> TradeOp {
    let (unit_price, total_amount) = split(trade);
    TradeOp {
        asset_id,
        date,
        quantity: from_surface(trade.target.quantity),
        unit_price,
        exchange_rate: trade.rate,
        fees: trade.fees,
        total_amount,
        note: trade.note.clone(),
    }
}

/// The unit price and the typed total the recording command takes: a total leaves the unit
/// price to the core (TRX-060, SEL-050).
fn split(trade: &Trade) -> (i64, Option<i64>) {
    match trade.amount {
        TradeAmount::Price(price) => (price, None),
        TradeAmount::Total(total) => (0, Some(total)),
    }
}

/// The code a rejection carries on the wire: its serde tag (CLI-021).
pub fn code_of(error: &impl Serialize) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|value| {
            value
                .get("code")
                .and_then(|code| code.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

/// CLI-021 — an opening balance's rejection keeps its code; the sentence is the command line's.
fn refusal_from_open(error: &OpenHoldingError) -> Refusal {
    match error {
        OpenHoldingError::Account(error) => refusal_from_account(error),
        OpenHoldingError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                OpenHoldingTask::AssetNotFound => "the asset no longer exists".to_string(),
                OpenHoldingTask::ArchivedAsset => "the asset is archived".to_string(),
                OpenHoldingTask::OpeningBalanceOnCashAsset => {
                    "an opening balance cannot be recorded on cash".to_string()
                }
            },
        },
    }
}

/// AGT-063 — a split's rejection keeps its code.
fn refusal_from_split(error: &SplitError) -> Refusal {
    match error {
        SplitError::Account(error) => refusal_from_account(error),
        SplitError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                SplitTask::AssetNotFound => "the asset no longer exists".to_string(),
                SplitTask::AssetNotHeld => {
                    "nothing of this asset is held in the account".to_string()
                }
            },
        },
    }
}

/// AGT-063 — a rejection of free shares keeps its code.
fn refusal_from_free_shares(error: &FreeSharesError) -> Refusal {
    match error {
        FreeSharesError::Account(error) => refusal_from_account(error),
        FreeSharesError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                FreeSharesTask::AssetNotFound => "the asset no longer exists".to_string(),
                FreeSharesTask::AssetNotHeld => {
                    "nothing of this asset is held in the account".to_string()
                }
                FreeSharesTask::FreeSharesOnCashAsset => {
                    "free shares cannot be recorded on cash".to_string()
                }
            },
        },
    }
}

/// AGT-066 — a rejection of interest keeps its code.
fn refusal_from_interest(error: &InterestError) -> Refusal {
    match error {
        InterestError::Account(error) => refusal_from_account(error),
        InterestError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                InterestTask::AssetNotFound => "the asset no longer exists".to_string(),
                InterestTask::InterestNotEligible => {
                    "the asset bears no interest: only an interest-bearing asset or the cash is credited".to_string()
                }
                InterestTask::AssetNotHeld => {
                    "nothing of this asset is held in the account".to_string()
                }
            },
        },
    }
}

/// AGT-066 — a rejection of a management fee keeps its code.
fn refusal_from_management_fee(error: &ManagementFeeError) -> Refusal {
    match error {
        ManagementFeeError::Account(error) => refusal_from_account(error),
        ManagementFeeError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                ManagementFeeTask::AssetNotFound => "the asset no longer exists".to_string(),
                ManagementFeeTask::AssetNotHeld => {
                    "nothing of this asset is held in the account".to_string()
                }
                ManagementFeeTask::ManagementFeeOnCashAsset => {
                    "a management fee cannot be taken from cash".to_string()
                }
            },
        },
    }
}

/// AGT-065 — a rejection of a write-off keeps its code.
fn refusal_from_write_off(error: &WriteOffError) -> Refusal {
    match error {
        WriteOffError::Account(error) => refusal_from_account(error),
        WriteOffError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                WriteOffTask::AssetNotFound => "the asset no longer exists".to_string(),
                WriteOffTask::ArchivedAsset => {
                    "the asset is archived: unarchive it to write it off".to_string()
                }
            },
        },
    }
}

/// CLI-021 — a price's rejection keeps its code; the sentence is the command line's.
fn refusal_from_asset(error: &AssetError) -> Refusal {
    let message = match error {
        AssetError::NotPositive => "the price must be above zero".to_string(),
        AssetError::InvalidDateFormat { .. } => "the date is not a valid date".to_string(),
        AssetError::DateInFuture => "the date is in the future".to_string(),
        AssetError::Archived => "the asset is archived: unarchive it to record a price".to_string(),
        AssetError::AssetNotFound { .. } => "the asset no longer exists".to_string(),
        AssetError::DatabaseError => "the portfolio could not be read or written".to_string(),
        // The rejections of other operations on an asset, which a price never meets.
        other => format!("refused by the rules ({})", code_of(other)),
    };
    Refusal {
        code: code_of(error),
        message,
    }
}

/// CLI-021 — a dividend's rejection keeps its code; the sentence is the command line's.
fn refusal_from_dividend(error: &DividendError) -> Refusal {
    match error {
        DividendError::Account(error) => refusal_from_account(error),
        DividendError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                DividendTask::AssetNotFound => "the asset no longer exists".to_string(),
                DividendTask::AssetNotHeld => {
                    "nothing of this asset is held in the account".to_string()
                }
                DividendTask::DividendOnCashAsset => {
                    "a dividend cannot be recorded on cash".to_string()
                }
            },
        },
    }
}

/// CLI-021 — a recording rejection keeps its code; the sentence is the command line's. Every
/// variant is named, so a new one fails to compile until it has a sentence.
pub fn refusal_from_account(error: &AccountError) -> Refusal {
    let message = match error {
        AccountError::ArchivedAssetSell => {
            "the asset is archived: unarchive it to sell".to_string()
        }
        AccountError::Oversell {
            available,
            requested,
        } => format!(
            "only {} held on that date, not {}",
            decimal(*available),
            decimal(*requested)
        ),
        AccountError::InsufficientCash { .. } => "not enough cash in the account".to_string(),
        AccountError::ClosedPosition => "nothing of this asset is held on that date".to_string(),
        AccountError::CascadingOversell => {
            "this would leave a later transaction without enough units".to_string()
        }
        AccountError::WriteOffWithoutReason => {
            "this write-off has no reason: correct it with one".to_string()
        }
        AccountError::TransferToSameAccount => {
            "a transfer moves a holding to another account".to_string()
        }
        AccountError::TransferReceivedAboveSent => {
            "the quantity received cannot exceed the quantity sent".to_string()
        }
        AccountError::TransferRateMissing {
            from_currency,
            to_currency,
        } => format!(
            "no {from_currency} to {to_currency} rate is stored on or before that date: give one"
        ),
        AccountError::TransferRateNotOne => {
            "the two accounts share a currency: a transfer between them takes no rate".to_string()
        }
        AccountError::PartOfTransfer => {
            "this is part of a transfer: correct the transfer".to_string()
        }
        AccountError::TransferNotFound => "the transfer no longer exists".to_string(),
        AccountError::OpeningBalanceOnCashAsset => {
            "an opening balance cannot be recorded on cash".to_string()
        }
        AccountError::InvalidDate => "the date is not a valid date".to_string(),
        AccountError::DateInFuture => "the date is in the future".to_string(),
        AccountError::DateTooOld => "the date is before 1900".to_string(),
        AccountError::QuantityNotPositive => "the quantity must be above zero".to_string(),
        AccountError::UnitPriceNegative => "the price cannot be negative".to_string(),
        AccountError::QuantityOutOfRange => "the quantity is too large".to_string(),
        AccountError::UnitPriceOutOfRange => {
            "the price this total gives cannot be stored".to_string()
        }
        AccountError::FeesNegative => "the fees cannot be negative".to_string(),
        AccountError::ExchangeRateNotPositive => "the exchange rate must be above zero".to_string(),
        AccountError::TotalAmountNotPositive => "the total must be above zero".to_string(),
        AccountError::AmountNotPositive => "the amount must be above zero".to_string(),
        AccountError::TotalAmountBelowFees => "the total is below the fees it includes".to_string(),
        AccountError::InvalidTotalCost => "the total cost cannot be negative".to_string(),
        AccountError::SplitFactorNotPositive => {
            "new and old must be whole numbers above zero".to_string()
        }
        AccountError::SplitFactorIsOne => "a split of 1 for 1 changes nothing".to_string(),
        AccountError::SplitOnCashAsset => "cash cannot be split".to_string(),
        AccountError::SplitCollapsesPosition => {
            "this split would leave nothing of the position".to_string()
        }
        // AGT-066 — interest and management fees (INT-021, FEE-021, FEE-028, FEE-077).
        AccountError::PercentageNotPositive => "the percent must be above zero".to_string(),
        AccountError::PercentageAboveHundred => "the percent cannot be above 100".to_string(),
        AccountError::ResultingQuantityNegative => {
            "the resulting quantity cannot be negative".to_string()
        }
        AccountError::ResultingQuantityNotBelowHeld { held_quantity } => format!(
            "the resulting quantity must be below the {} held on that date",
            decimal(*held_quantity)
        ),
        AccountError::InterestAmountInvalid => "give either percent or quantity".to_string(),
        AccountError::ManagementFeeAmountInvalid => {
            "give either percent or resulting_quantity".to_string()
        }
        AccountError::ManagementFeesDisabled => {
            "management fees are turned off for this account".to_string()
        }
        AccountError::AccountNotFound { .. } => "the account no longer exists".to_string(),
        AccountError::DatabaseError | AccountError::ApplicationWriteOutranked => {
            "the portfolio could not be read or written".to_string()
        }
        // Rejections of other operations, which no recording by name meets.
        AccountError::NameEmpty
        | AccountError::InvalidCurrency { .. }
        | AccountError::InvalidCountry { .. }
        | AccountError::WrapperNotAvailable { .. }
        | AccountError::NegativeQuantity
        | AccountError::NegativeAveragePrice
        | AccountError::TransactionNotFound
        | AccountError::RateNotPositive
        | AccountError::RateAboveHundred
        | AccountError::EndBeforeStart
        | AccountError::ScheduleAlreadyExists
        | AccountError::ScheduleNotFound
        | AccountError::NoteTextEmpty
        | AccountError::NoteTextTooLong
        | AccountError::ThresholdNotPositive
        | AccountError::ThresholdIncomplete
        | AccountError::NoteOnCashAsset
        | AccountError::TradeOnCashAsset
        | AccountError::NoteOnUnheldAsset
        | AccountError::NameAlreadyExists => format!("refused by the rules ({})", code_of(error)),
    };
    Refusal {
        code: code_of(error),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CLI-021 — a recording rejection keeps its code; the sentence names the reason.
    #[test]
    fn cli_021_recording_rejections_keep_their_code() {
        let cases = [
            (
                AccountError::Oversell {
                    available: 1_000_000,
                    requested: 2_500_000,
                },
                "Oversell",
                "only 1 held on that date, not 2.5",
            ),
            (
                AccountError::ClosedPosition,
                "ClosedPosition",
                "nothing of this asset is held on that date",
            ),
            (
                AccountError::CascadingOversell,
                "CascadingOversell",
                "this would leave a later transaction without enough units",
            ),
            (
                AccountError::WriteOffWithoutReason,
                "WriteOffWithoutReason",
                "this write-off has no reason: correct it with one",
            ),
            (
                AccountError::DateInFuture,
                "DateInFuture",
                "the date is in the future",
            ),
            (
                AccountError::DateTooOld,
                "DateTooOld",
                "the date is before 1900",
            ),
            (
                AccountError::InvalidDate,
                "InvalidDate",
                "the date is not a valid date",
            ),
            (
                AccountError::QuantityNotPositive,
                "QuantityNotPositive",
                "the quantity must be above zero",
            ),
            (
                AccountError::UnitPriceNegative,
                "UnitPriceNegative",
                "the price cannot be negative",
            ),
            (
                AccountError::QuantityOutOfRange,
                "QuantityOutOfRange",
                "the quantity is too large",
            ),
            (
                AccountError::UnitPriceOutOfRange,
                "UnitPriceOutOfRange",
                "the price this total gives cannot be stored",
            ),
            (
                AccountError::FeesNegative,
                "FeesNegative",
                "the fees cannot be negative",
            ),
            (
                AccountError::ExchangeRateNotPositive,
                "ExchangeRateNotPositive",
                "the exchange rate must be above zero",
            ),
            (
                AccountError::TotalAmountNotPositive,
                "TotalAmountNotPositive",
                "the total must be above zero",
            ),
            (
                AccountError::TotalAmountBelowFees,
                "TotalAmountBelowFees",
                "the total is below the fees it includes",
            ),
            (
                AccountError::InvalidTotalCost,
                "InvalidTotalCost",
                "the total cost cannot be negative",
            ),
            (
                AccountError::OpeningBalanceOnCashAsset,
                "OpeningBalanceOnCashAsset",
                "an opening balance cannot be recorded on cash",
            ),
            (
                AccountError::DatabaseError,
                "DatabaseError",
                "the portfolio could not be read or written",
            ),
            (
                AccountError::NameEmpty,
                "NameEmpty",
                "refused by the rules (NameEmpty)",
            ),
        ];
        for (error, code, message) in cases {
            let refusal = refusal_from_account(&error);
            assert_eq!(
                (refusal.code.as_str(), refusal.message.as_str()),
                (code, message)
            );
        }
    }

    // CLI-021 — an opening balance's own rejections keep their code too.
    #[test]
    fn cli_021_opening_balance_rejections_keep_their_code() {
        let archived =
            refusal_from_open(&OpenHoldingError::UseCase(OpenHoldingTask::ArchivedAsset));
        assert_eq!(
            (archived.code.as_str(), archived.message.as_str()),
            ("ArchivedAsset", "the asset is archived")
        );
        let gone = refusal_from_open(&OpenHoldingError::UseCase(OpenHoldingTask::AssetNotFound));
        assert_eq!(gone.code, "AssetNotFound");
        let cash = refusal_from_open(&OpenHoldingError::UseCase(
            OpenHoldingTask::OpeningBalanceOnCashAsset,
        ));
        assert_eq!(
            cash.message,
            "an opening balance cannot be recorded on cash"
        );
        let through = refusal_from_open(&OpenHoldingError::Account(AccountError::DateInFuture));
        assert_eq!(through.code, "DateInFuture");
    }

    // CLI-011 / AGT-042 — what was typed is repeated with its control characters escaped:
    // the sentence reaches a terminal and an agent.
    #[test]
    fn cli_011_a_lookup_refusal_escapes_what_was_typed() {
        let typed = "PEA\u{1b}[2J\nRecorded:".to_string();
        for error in [
            NameLookupError::AccountNotFound {
                typed: typed.clone(),
            },
            NameLookupError::AccountAmbiguous {
                typed: typed.clone(),
            },
            NameLookupError::AssetNotFound {
                typed: typed.clone(),
            },
            NameLookupError::AssetAmbiguous {
                typed: typed.clone(),
            },
        ] {
            let message = refusal_from_lookup(&error).message;
            assert!(message.contains("PEA\\u{1b}[2J\\nRecorded:"), "{message}");
            assert!(!message.chars().any(char::is_control), "{message}");
        }
    }

    // CLI-021 — a price's rejections keep their code, each with its sentence.
    #[test]
    fn cli_021_price_rejections_keep_their_code() {
        for (error, code, message) in [
            (
                AssetError::NotPositive,
                "NotPositive",
                "the price must be above zero",
            ),
            (
                AssetError::DateInFuture,
                "DateInFuture",
                "the date is in the future",
            ),
            (
                AssetError::Archived,
                "Archived",
                "the asset is archived: unarchive it to record a price",
            ),
            (
                AssetError::InvalidDateFormat {
                    date: "31/12/2019".to_string(),
                },
                "InvalidDateFormat",
                "the date is not a valid date",
            ),
        ] {
            let refusal = refusal_from_asset(&error);
            assert_eq!(
                (refusal.code.as_str(), refusal.message.as_str()),
                (code, message)
            );
        }
    }

    // CLI-021 — a dividend's rejections keep their code, each with its sentence.
    #[test]
    fn cli_021_dividend_rejections_keep_their_code() {
        for (task, code, message) in [
            (
                DividendTask::AssetNotHeld,
                "AssetNotHeld",
                "nothing of this asset is held in the account",
            ),
            (
                DividendTask::AssetNotFound,
                "AssetNotFound",
                "the asset no longer exists",
            ),
            (
                DividendTask::DividendOnCashAsset,
                "DividendOnCashAsset",
                "a dividend cannot be recorded on cash",
            ),
        ] {
            let refusal = refusal_from_dividend(&DividendError::UseCase(task));
            assert_eq!(
                (refusal.code.as_str(), refusal.message.as_str()),
                (code, message)
            );
        }
        let refusal =
            refusal_from_dividend(&DividendError::Account(AccountError::AmountNotPositive));
        assert_eq!(refusal.code, "AmountNotPositive");
        assert_eq!(refusal.message, "the amount must be above zero");
    }

    // CLI-011 — a name the use case could not resolve keeps its code and names only what was
    // typed.
    #[test]
    fn cli_011_lookup_refusals_name_only_what_was_typed() {
        let typed = || "PEA".to_string();
        let cases = [
            (
                NameLookupError::AccountNotFound { typed: typed() },
                "AccountNotFound",
                "no account named \"PEA\"",
            ),
            (
                NameLookupError::AccountAmbiguous { typed: typed() },
                "AccountAmbiguous",
                "more than one account is named \"PEA\"",
            ),
            (
                NameLookupError::AssetNotFound { typed: typed() },
                "AssetNotFound",
                "no asset matches \"PEA\" by name, reference or ISIN",
            ),
            (
                NameLookupError::AssetAmbiguous { typed: typed() },
                "AssetAmbiguous",
                "\"PEA\" matches more than one asset — use its name, its ISIN, or its reference with its exchange (ASML@XAMS)",
            ),
            (
                NameLookupError::DatabaseError,
                "DatabaseError",
                "the portfolio could not be read",
            ),
        ];
        for (error, code, message) in cases {
            let refusal = refusal_from_lookup(&error);
            assert_eq!(
                (refusal.code.as_str(), refusal.message.as_str()),
                (code, message)
            );
        }
    }
}
