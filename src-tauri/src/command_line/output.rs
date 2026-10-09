//! What a command prints (CLI-020, CLI-021) and the exit code it returns (CLI-022).

use serde_json::json;

use super::orchestrator::{verb, AssetRow, Listing, Outcome, Refusal};

/// A command recorded.
pub const RECORDED: i32 = 0;
/// A command was refused.
pub const REFUSED: i32 = 1;
/// The command line itself is wrong.
pub const WRONG_USAGE: i32 = 2;

/// What to print, where, and the exit code.
#[derive(Debug, PartialEq, Eq)]
pub struct Printed {
    /// Text for standard output.
    pub stdout: Option<String>,
    /// Text for standard error.
    pub stderr: Option<String>,
    /// The exit code.
    pub exit_code: i32,
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

/// A money figure in micro-units with two decimals, rounded half away from zero.
fn money(micros: i64) -> String {
    let cents = (micros as i128 * 100 + if micros >= 0 { 500_000 } else { -500_000 }) / 1_000_000;
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

/// CLI-020/021/022 — renders an outcome as text or JSON with its exit code.
pub fn render(outcome: &Outcome, json: bool) -> Printed {
    match (outcome, json) {
        (
            Outcome::Recorded {
                transaction,
                account_name,
                currency,
                asset_reference,
                zero_cost,
            },
            false,
        ) => Printed {
            stdout: Some(format!(
                "Recorded: {} {} {} in {} on {} — {} {}",
                verb(transaction.transaction_type),
                decimal(transaction.quantity),
                asset_reference,
                account_name,
                transaction.date,
                money(transaction.total_amount),
                currency
            )),
            // TRX-065 — recorded all the same: the warning never blocks.
            stderr: zero_cost.then(|| {
                "Warning: a total cost of 0 declares no invested amount; the account's lifetime \
                 performance may not be computed, or be overstated. Enter the position's value \
                 on that date instead."
                    .to_string()
            }),
            exit_code: RECORDED,
        },
        (
            Outcome::Recorded {
                transaction,
                zero_cost,
                ..
            },
            true,
        ) => Printed {
            stdout: Some(if *zero_cost {
                json!({ "status": "recorded", "transaction": transaction, "warning": "ZeroCostOpeningBalance" })
            } else {
                json!({ "status": "recorded", "transaction": transaction })
            }
            .to_string()),
            stderr: None,
            exit_code: RECORDED,
        },
        (
            Outcome::AssetAdded {
                asset,
                reference_shared,
            },
            true,
        ) => Printed {
            stdout: Some(if *reference_shared {
                json!({ "status": "recorded", "asset": asset, "warning": "ReferenceShared" })
            } else {
                json!({ "status": "recorded", "asset": asset })
            }
            .to_string()),
            stderr: None,
            exit_code: RECORDED,
        },
        (
            Outcome::AssetAdded {
                asset,
                reference_shared,
            },
            false,
        ) => Printed {
            stdout: Some(format!(
                "Recorded: added {} ({}) — {}, {}",
                printable(&asset.name),
                printable(&asset.reference),
                asset.class,
                asset.currency
            )),
            // CLI-041 — allowed, and worth saying: `--asset` by this reference is now ambiguous.
            stderr: reference_shared.then(|| {
                format!(
                    "Warning: another asset has the reference {}; name this one in --asset by {}.",
                    printable(&asset.reference),
                    ways_to_name(asset)
                )
            }),
            exit_code: RECORDED,
        },
        (Outcome::Listed(listing), json) => Printed {
            stdout: Some(if json {
                listed_as_json(listing)
            } else {
                listed_as_table(listing)
            }),
            stderr: None,
            exit_code: RECORDED,
        },
        (Outcome::Refused(refusal), json) => refused(refusal, json),
    }
}

/// CLI-021 — a list as one JSON object, every field of each row.
fn listed_as_json(listing: &Listing) -> String {
    match listing {
        Listing::Accounts(rows) => json!({ "status": "listed", "accounts": rows }),
        Listing::Assets(rows) => json!({ "status": "listed", "assets": rows }),
    }
    .to_string()
}

/// CLI-041 — what still names an asset whose reference another asset has: its name, its
/// ISIN when it has one, and its reference with its exchange when it is on one.
fn ways_to_name(asset: &AssetRow) -> String {
    match (&asset.isin, &asset.exchange) {
        (Some(_), Some(code)) => format!(
            "its name, its ISIN or {}@{}",
            printable(&asset.reference),
            printable(code)
        ),
        (Some(_), None) => "its name or its ISIN".to_string(),
        (None, _) => "its name".to_string(),
    }
}

/// CLI-020 — a list as a table: a header, then what a command needs to name each row.
fn listed_as_table(listing: &Listing) -> String {
    match listing {
        Listing::Accounts(rows) => table(
            ["NAME", "CURRENCY"],
            rows.iter()
                .map(|row| [row.name.as_str(), row.currency.as_str()]),
        ),
        Listing::Assets(rows) => table(
            ["NAME", "REFERENCE"],
            rows.iter()
                .map(|row| [row.name.as_str(), row.reference.as_str()]),
        ),
    }
}

/// Two columns, the first padded to its widest cell, three spaces apart. A control
/// character in a cell is printed escaped, never sent to the terminal.
fn table<'a>(header: [&'a str; 2], rows: impl Iterator<Item = [&'a str; 2]>) -> String {
    let lines: Vec<[String; 2]> = std::iter::once(header)
        .chain(rows)
        .map(|cells| cells.map(printable))
        .collect();
    let width = lines
        .iter()
        .map(|[first, _]| first.chars().count())
        .max()
        .unwrap_or(0);
    lines
        .iter()
        .map(|[first, second]| format!("{first:<width$}   {second}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text` with every control character replaced by its escape (`\u{1b}`, `\r`).
fn printable(text: &str) -> String {
    text.chars()
        .flat_map(|letter| {
            if letter.is_control() {
                letter.escape_default().collect::<Vec<char>>()
            } else {
                vec![letter]
            }
        })
        .collect()
}

/// A refusal as text on standard error, or as JSON on standard output.
pub fn refused(refusal: &Refusal, json: bool) -> Printed {
    if json {
        Printed {
            stdout: Some(
                json!({ "status": "refused", "code": refusal.code, "message": refusal.message })
                    .to_string(),
            ),
            stderr: None,
            exit_code: REFUSED,
        }
    } else {
        Printed {
            stdout: None,
            stderr: Some(format!("Refused: {}", refusal.message)),
            exit_code: REFUSED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{Transaction, TransactionType};

    // CLI-020 — a list of nothing prints its header alone; a control character in a name is
    // printed escaped, and the column is as wide as what is printed.
    #[test]
    fn cli_020_a_table_escapes_control_characters_and_prints_its_header_alone() {
        use crate::command_line::orchestrator::AccountRow;

        let none = render(&Outcome::Listed(Listing::Accounts(vec![])), false);
        assert_eq!(none.stdout.as_deref(), Some("NAME   CURRENCY"));

        let row = |name: &str| AccountRow {
            name: name.to_string(),
            currency: "EUR".to_string(),
        };
        let listed = render(
            &Outcome::Listed(Listing::Accounts(vec![row("PE\u{1b}[2JA"), row("CTO")])),
            false,
        );
        assert_eq!(
            listed.stdout.as_deref(),
            Some("NAME           CURRENCY\nPE\\u{1b}[2JA   EUR\nCTO            EUR")
        );
    }

    fn purchase() -> Transaction {
        Transaction::restore(
            "tx-1".to_string(),
            "acc".to_string(),
            "cw8".to_string(),
            TransactionType::Purchase,
            "2026-09-28".to_string(),
            2_000_000,
            495_100_000,
            1_000_000,
            1_990_000,
            992_190_000,
            None,
            None,
            "2026-09-28T10:00:00Z".to_string(),
        )
    }

    fn recorded() -> Outcome {
        Outcome::Recorded {
            transaction: Box::new(purchase()),
            account_name: "PEA".to_string(),
            currency: "EUR".to_string(),
            asset_reference: "CW8".to_string(),
            zero_cost: false,
        }
    }

    // CLI-020 — a recorded purchase prints one line on standard output and exits 0.
    #[test]
    fn cli_020_a_recorded_transaction_prints_one_line() {
        assert_eq!(
            render(&recorded(), false),
            Printed {
                stdout: Some(
                    "Recorded: bought 2 CW8 in PEA on 2026-09-28 — 992.19 EUR".to_string()
                ),
                stderr: None,
                exit_code: RECORDED,
            }
        );
    }

    // CLI-020 / CLI-022 — a refusal prints its reason on standard error and exits 1.
    #[test]
    fn cli_020_a_refusal_prints_its_reason_on_standard_error() {
        let refusal = Refusal {
            code: "AccountNotFound".to_string(),
            message: "no account named \"PEA2\"".to_string(),
        };
        assert_eq!(
            render(&Outcome::Refused(refusal), false),
            Printed {
                stdout: None,
                stderr: Some("Refused: no account named \"PEA2\"".to_string()),
                exit_code: REFUSED,
            }
        );
    }

    // CLI-021 — with --json, one object on standard output: the transaction, or the code.
    #[test]
    fn cli_021_json_carries_the_transaction_or_the_code() {
        let printed = render(&recorded(), true);
        let value: serde_json::Value =
            serde_json::from_str(&printed.stdout.expect("json")).expect("valid json");
        assert_eq!(value["status"], "recorded");
        assert_eq!(value["transaction"]["id"], "tx-1");
        assert_eq!(value["transaction"]["total_amount"], 992_190_000);

        let refusal = Refusal {
            code: "Oversell".to_string(),
            message: "only 1 held, not 2".to_string(),
        };
        let printed = render(&Outcome::Refused(refusal), true);
        assert_eq!(printed.exit_code, REFUSED);
        assert_eq!(printed.stderr, None);
        let value: serde_json::Value =
            serde_json::from_str(&printed.stdout.expect("json")).expect("valid json");
        assert_eq!(
            value,
            json!({"status":"refused","code":"Oversell","message":"only 1 held, not 2"})
        );
    }

    // CLI-020 — figures read as a person writes them.
    #[test]
    fn cli_020_figures_read_as_written() {
        assert_eq!(decimal(2_000_000), "2");
        assert_eq!(decimal(1_500_000), "1.5");
        assert_eq!(decimal(1), "0.000001");
        assert_eq!(decimal(-2_500_000), "-2.5");
        assert_eq!(money(992_190_000), "992.19");
        assert_eq!(money(992_195_000), "992.20");
        assert_eq!(money(-5_000), "-0.01");
        assert_eq!(money(0), "0.00");
    }
    // CLI-041 — the warning names what still tells the added asset apart: always its name,
    // its ISIN when it has one, its reference with its own exchange when it is on one.
    #[test]
    fn cli_041_the_shared_reference_warning_names_what_tells_the_asset_apart() {
        use crate::context::asset::{AssetClass, AssetKind};

        let warned = |isin: Option<&str>, exchange: Option<&str>| {
            render(
                &Outcome::AssetAdded {
                    asset: AssetRow {
                        name: "ASML".to_string(),
                        reference: "ASML".to_string(),
                        kind: AssetKind::Listed,
                        class: AssetClass::Stocks,
                        currency: "EUR".to_string(),
                        isin: isin.map(str::to_string),
                        exchange: exchange.map(str::to_string),
                        archived: false,
                    },
                    reference_shared: true,
                },
                false,
            )
            .stderr
            .expect("warning")
        };
        let start = "Warning: another asset has the reference ASML; name this one in --asset by";

        assert_eq!(
            warned(Some("NL0010273215"), Some("XAMS")),
            format!("{start} its name, its ISIN or ASML@XAMS.")
        );
        assert_eq!(
            warned(Some("NL0010273215"), None),
            format!("{start} its name or its ISIN.")
        );
        assert_eq!(warned(None, None), format!("{start} its name."));
    }
}
