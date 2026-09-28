//! The account journal (TXL-060): every transaction of an account in date order, each
//! with the cash it took out or brought in and the account's cash balance after it.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{Account, Transaction, TransactionType};

/// The cash a transaction moves on the account's cash line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashEffect {
    /// Cash brought in, in account-currency micro-units.
    In(i64),
    /// Cash taken out, in account-currency micro-units.
    Out(i64),
    /// No cash moved.
    None,
}

impl CashEffect {
    /// The effect on the balance: positive when cash comes in.
    pub fn signed(self) -> i64 {
        match self {
            CashEffect::In(amount) => amount,
            CashEffect::Out(amount) => -amount,
            CashEffect::None => 0,
        }
    }
}

/// Which rows of the account journal are listed, and in which order (TXL-060). A field
/// left empty keeps every row; the amount bounds are inclusive and compare the recorded
/// total, so a lowest total above the highest keeps no row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct JournalFilter {
    /// Only the transactions of this asset.
    pub asset_id: Option<String>,
    /// Only the transactions of this type.
    pub transaction_type: Option<TransactionType>,
    /// Only the transactions whose total is at least this (micro-units).
    pub amount_min: Option<i64>,
    /// Only the transactions whose total is at most this (micro-units).
    pub amount_max: Option<i64>,
    /// Newest first instead of oldest first.
    pub newest_first: bool,
}

/// One transaction of the account journal with its cash columns.
#[derive(Debug, Clone, Serialize, Type)]
pub struct JournalRow {
    /// The transaction.
    pub transaction: Transaction,
    /// Cash it took out (micro-units), or none.
    pub cash_out: Option<i64>,
    /// Cash it brought in (micro-units), or none.
    pub cash_in: Option<i64>,
    /// The account's cash balance after it, over every transaction of the account
    /// whatever the filter (micro-units).
    pub cash_balance: i64,
}

/// The account journal of one account, filtered (TXL-060).
#[derive(Debug, Clone, Serialize, Type)]
pub struct AccountJournal {
    /// The rows the filter keeps, oldest first — by date, then in the order entered — or
    /// newest first when the filter asks.
    pub rows: Vec<JournalRow>,
    /// The assets the account has transactions for, in order of first appearance.
    pub asset_ids: Vec<String>,
    /// The transaction types the account has, in order of first appearance.
    pub transaction_types: Vec<TransactionType>,
    /// The account has at least one transaction, whatever the filter.
    pub has_transactions: bool,
}

impl AccountJournal {
    /// TXL-060 — builds the journal of an account from all its transactions: the cash
    /// balance runs over every transaction in date order, then the filter picks the rows.
    pub fn from_transactions(mut transactions: Vec<Transaction>, filter: &JournalFilter) -> Self {
        transactions.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| a.created_at.cmp(&b.created_at))
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut asset_ids: Vec<String> = Vec::new();
        let mut transaction_types: Vec<TransactionType> = Vec::new();
        let has_transactions = !transactions.is_empty();
        let mut balance: i128 = 0;
        let mut rows = Vec::with_capacity(transactions.len());
        for transaction in transactions {
            if !asset_ids.contains(&transaction.asset_id) {
                asset_ids.push(transaction.asset_id.clone());
            }
            if !transaction_types.contains(&transaction.transaction_type) {
                transaction_types.push(transaction.transaction_type);
            }
            let effect = Account::cash_effect(&transaction);
            balance += effect.signed() as i128;
            if !filter.keeps(&transaction) {
                continue;
            }
            let (cash_out, cash_in) = match effect {
                CashEffect::In(amount) => (None, Some(amount)),
                CashEffect::Out(amount) => (Some(amount), None),
                CashEffect::None => (None, None),
            };
            rows.push(JournalRow {
                transaction,
                cash_out,
                cash_in,
                cash_balance: i64::try_from(balance).unwrap_or(if balance < 0 {
                    i64::MIN
                } else {
                    i64::MAX
                }),
            });
        }
        if filter.newest_first {
            rows.reverse();
        }
        Self {
            rows,
            asset_ids,
            transaction_types,
            has_transactions,
        }
    }
}

impl JournalFilter {
    fn keeps(&self, transaction: &Transaction) -> bool {
        self.asset_id
            .as_ref()
            .is_none_or(|asset_id| &transaction.asset_id == asset_id)
            && self
                .transaction_type
                .is_none_or(|kind| transaction.transaction_type == kind)
            && self
                .amount_min
                .is_none_or(|min| transaction.total_amount >= min)
            && self
                .amount_max
                .is_none_or(|max| transaction.total_amount <= max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: i64 = 1_000_000;

    fn tx(
        id: &str,
        asset_id: &str,
        kind: TransactionType,
        date: &str,
        created_at: &str,
        quantity: i64,
        total_amount: i64,
    ) -> Transaction {
        Transaction::restore(
            id.to_string(),
            "account-1".to_string(),
            asset_id.to_string(),
            kind,
            date.to_string(),
            quantity,
            0,
            M,
            0,
            total_amount,
            None,
            None,
            created_at.to_string(),
        )
    }

    fn cash_and_trades() -> Vec<Transaction> {
        vec![
            // Entered out of order: the journal orders by date, then entry.
            tx(
                "t4",
                "aapl",
                TransactionType::Dividend,
                "2026-01-05",
                "c4",
                M,
                5 * M,
            ),
            tx(
                "t1",
                "system-cash-eur",
                TransactionType::Deposit,
                "2026-01-01",
                "c1",
                100 * M,
                100 * M,
            ),
            tx(
                "t3",
                "aapl",
                TransactionType::Purchase,
                "2026-01-02",
                "c3",
                2 * M,
                30 * M,
            ),
            tx(
                "t2",
                "msft",
                TransactionType::OpeningBalance,
                "2026-01-02",
                "c2",
                M,
                50 * M,
            ),
            tx(
                "t5",
                "system-cash-eur",
                TransactionType::Interest,
                "2026-01-06",
                "c5",
                2 * M,
                0,
            ),
            tx(
                "t6",
                "aapl",
                TransactionType::Interest,
                "2026-01-07",
                "c6",
                3 * M,
                0,
            ),
        ]
    }

    fn ids(journal: &AccountJournal) -> Vec<&str> {
        journal
            .rows
            .iter()
            .map(|row| row.transaction.id.as_str())
            .collect()
    }

    // TXL-060 — rows run oldest first: by date, then in the order entered.
    #[test]
    fn txl_060_rows_run_by_date_then_entry() {
        let journal =
            AccountJournal::from_transactions(cash_and_trades(), &JournalFilter::default());

        assert_eq!(ids(&journal), vec!["t1", "t2", "t3", "t4", "t5", "t6"]);
    }

    // TXL-060 / INT-023 — each row carries the cash it moved and the balance after it;
    // interest on the cash line brings in its quantity, interest on another asset nothing.
    #[test]
    fn txl_060_rows_carry_their_cash_and_the_running_balance() {
        let journal =
            AccountJournal::from_transactions(cash_and_trades(), &JournalFilter::default());
        let cells: Vec<(Option<i64>, Option<i64>, i64)> = journal
            .rows
            .iter()
            .map(|row| (row.cash_out, row.cash_in, row.cash_balance))
            .collect();

        assert_eq!(
            cells,
            vec![
                (None, Some(100 * M), 100 * M),
                (None, None, 100 * M),
                (Some(30 * M), None, 70 * M),
                (None, Some(5 * M), 75 * M),
                (None, Some(2 * M), 77 * M),
                (None, None, 77 * M),
            ]
        );
    }

    // TXL-060 — the filter picks rows; the balance still runs over every transaction.
    #[test]
    fn txl_060_the_filter_keeps_rows_but_not_the_balance() {
        let journal = AccountJournal::from_transactions(
            cash_and_trades(),
            &JournalFilter {
                asset_id: Some("aapl".to_string()),
                transaction_type: None,
                amount_min: Some(5 * M),
                amount_max: Some(30 * M),
                newest_first: false,
            },
        );

        assert_eq!(ids(&journal), vec!["t3", "t4"]);
        assert_eq!(journal.rows[1].cash_balance, 75 * M);

        let by_type = AccountJournal::from_transactions(
            cash_and_trades(),
            &JournalFilter {
                transaction_type: Some(TransactionType::Interest),
                ..JournalFilter::default()
            },
        );
        assert_eq!(ids(&by_type), vec!["t5", "t6"]);
    }

    // TXL-060 — the filter choices come from the whole account, in order of first appearance.
    #[test]
    fn txl_060_lists_the_assets_and_types_present() {
        let journal = AccountJournal::from_transactions(
            cash_and_trades(),
            &JournalFilter {
                asset_id: Some("nothing".to_string()),
                ..JournalFilter::default()
            },
        );

        assert!(journal.rows.is_empty());
        assert!(journal.has_transactions);
        assert_eq!(journal.asset_ids, vec!["system-cash-eur", "msft", "aapl"]);
        assert_eq!(
            journal.transaction_types,
            vec![
                TransactionType::Deposit,
                TransactionType::OpeningBalance,
                TransactionType::Purchase,
                TransactionType::Dividend,
                TransactionType::Interest,
            ]
        );

        let empty = AccountJournal::from_transactions(Vec::new(), &JournalFilter::default());
        assert!(!empty.has_transactions);
        assert!(empty.asset_ids.is_empty());
    }

    // TXL-060 — newest first reverses the rows; each keeps its balance.
    #[test]
    fn txl_060_newest_first_reverses_the_rows() {
        let journal = AccountJournal::from_transactions(
            cash_and_trades(),
            &JournalFilter {
                newest_first: true,
                ..JournalFilter::default()
            },
        );

        assert_eq!(ids(&journal), vec!["t6", "t5", "t4", "t3", "t2", "t1"]);
        assert_eq!(journal.rows[0].cash_balance, 77 * M);
    }

    // TXL-060 — the amount bounds compare the recorded total: interest records none, and a
    // lowest total above the highest keeps no row.
    #[test]
    fn txl_060_amount_bounds_compare_the_recorded_total() {
        let from_one = AccountJournal::from_transactions(
            cash_and_trades(),
            &JournalFilter {
                amount_min: Some(1),
                ..JournalFilter::default()
            },
        );
        assert_eq!(ids(&from_one), vec!["t1", "t2", "t3", "t4"]);

        let crossed = AccountJournal::from_transactions(
            cash_and_trades(),
            &JournalFilter {
                amount_min: Some(10 * M),
                amount_max: Some(5 * M),
                ..JournalFilter::default()
            },
        );
        assert!(crossed.rows.is_empty());
    }
}
