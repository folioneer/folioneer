//! Reading a command line (CLI-010): the command, its options and their values, in
//! micro-units (TRX-024). Anything wrong is a usage error (CLI-022).

/// How to use the commands, printed by `--help` and after a usage error.
pub const USAGE: &str = "\
Usage:
  folioneer holding open --account <name> --asset <name or reference> --quantity <q> --total-cost <amount> [--date YYYY-MM-DD] [--json]
  folioneer holding buy  --account <name> --asset <name or reference> --quantity <q> (--price <amount> | --total <amount>) [--fees <amount>] [--rate <rate>] [--date YYYY-MM-DD] [--note <text>] [--json]
  folioneer holding sell (same options as buy)

Amounts and quantities are decimals with a dot. --date defaults to today, --fees to 0, --rate to 1.
Exit codes: 0 recorded, 1 refused, 2 wrong usage.";

const MICRO: i64 = 1_000_000;

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

/// A command the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// An opening balance: quantity and total cost.
    Open { target: Target, total_cost: i64 },
    /// A purchase.
    Buy(Trade),
    /// A sale.
    Sell(Trade),
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Run a command, printing text or JSON.
    Run { command: Command, json: bool },
    /// Print how to use the commands.
    Help,
}

/// A command line that cannot be run, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

/// CLI-010 — reads the arguments after the program name.
pub fn parse(args: &[String]) -> Result<Invocation, UsageError> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Invocation::Help);
    }
    let (group, verb, rest) = match args {
        [group, verb, rest @ ..] => (group.as_str(), verb.as_str(), rest),
        _ => return Err(UsageError("a command is missing".to_string())),
    };
    if group != "holding" {
        return Err(UsageError(format!("unknown command \"{group}\"")));
    }
    let allowed: &[&str] = match verb {
        "open" => &[
            "--account",
            "--asset",
            "--quantity",
            "--total-cost",
            "--date",
        ],
        "buy" | "sell" => &[
            "--account",
            "--asset",
            "--quantity",
            "--price",
            "--total",
            "--fees",
            "--rate",
            "--date",
            "--note",
        ],
        other => return Err(UsageError(format!("unknown command \"holding {other}\""))),
    };
    let mut options = Options::default();
    let mut json = false;
    let mut index = 0;
    while let Some(name) = rest.get(index).map(String::as_str) {
        if name == "--json" {
            json = true;
            index += 1;
            continue;
        }
        if !allowed.contains(&name) {
            return Err(UsageError(format!("unknown option \"{name}\"")));
        }
        let Some(value) = rest.get(index + 1) else {
            return Err(UsageError(format!("{name} needs a value")));
        };
        if options.values.iter().any(|(seen, _)| seen == name) {
            return Err(UsageError(format!("{name} is given twice")));
        }
        options.values.push((name.to_string(), value.clone()));
        index += 2;
    }
    let target = Target {
        account: options.text("--account")?,
        asset: options.text("--asset")?,
        date: options.date()?,
        quantity: options.amount("--quantity")?,
    };
    let command = match verb {
        "open" => Command::Open {
            target,
            total_cost: options.amount("--total-cost")?,
        },
        _ => {
            let amount = match (options.get("--price"), options.get("--total")) {
                (Some(_), None) => TradeAmount::Price(options.amount("--price")?),
                (None, Some(_)) => TradeAmount::Total(options.amount("--total")?),
                _ => return Err(UsageError("give either --price or --total".to_string())),
            };
            let trade = Trade {
                target,
                amount,
                fees: options.amount_or("--fees", 0)?,
                rate: options.amount_or("--rate", MICRO)?,
                note: options.get("--note").map(str::to_string),
            };
            if verb == "buy" {
                Command::Buy(trade)
            } else {
                Command::Sell(trade)
            }
        }
    };
    Ok(Invocation::Run { command, json })
}

#[derive(Default)]
struct Options {
    values: Vec<(String, String)>,
}

impl Options {
    fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(seen, _)| seen == name)
            .map(|(_, value)| value.as_str())
    }

    fn text(&self, name: &str) -> Result<String, UsageError> {
        match self.get(name).map(str::trim) {
            Some(value) if !value.is_empty() => Ok(value.to_string()),
            _ => Err(UsageError(format!("{name} is missing"))),
        }
    }

    fn amount(&self, name: &str) -> Result<i64, UsageError> {
        let value = self
            .get(name)
            .ok_or_else(|| UsageError(format!("{name} is missing")))?;
        decimal_to_micro(value)
            .ok_or_else(|| UsageError(format!("{name} is not a decimal number: \"{value}\"")))
    }

    fn amount_or(&self, name: &str, default: i64) -> Result<i64, UsageError> {
        match self.get(name) {
            Some(_) => self.amount(name),
            None => Ok(default),
        }
    }

    fn date(&self) -> Result<Option<String>, UsageError> {
        match self.get("--date") {
            None => Ok(None),
            Some(value) if is_iso_date(value) => Ok(Some(value.to_string())),
            Some(value) => Err(UsageError(format!(
                "--date is not a date as YYYY-MM-DD: \"{value}\""
            ))),
        }
    }
}

/// A decimal with a dot and at most six decimals, in micro-units; `None` when it is not one.
/// A leading minus is read, so the core refuses a negative figure with its own reason.
fn decimal_to_micro(text: &str) -> Option<i64> {
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

fn is_iso_date(text: &str) -> bool {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok() && text.len() == 10
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    }

    fn run(line: &str) -> Command {
        match parse(&args(line)).expect("valid") {
            Invocation::Run { command, .. } => command,
            Invocation::Help => panic!("help"),
        }
    }

    // CLI-010 — an opening balance reads its quantity and total cost; the date defaults.
    #[test]
    fn cli_010_reads_an_opening_balance() {
        assert_eq!(
            run("holding open --account PEA --asset CW8 --quantity 10 --total-cost 4950.5"),
            Command::Open {
                target: Target {
                    account: "PEA".to_string(),
                    asset: "CW8".to_string(),
                    date: None,
                    quantity: 10 * MICRO,
                },
                total_cost: 4_950_500_000,
            }
        );
    }

    // CLI-010 — a purchase by price takes the defaults; a sale by total keeps its options.
    #[test]
    fn cli_010_reads_a_purchase_and_a_sale() {
        let Command::Buy(buy) =
            run("holding buy --account PEA --asset CW8 --quantity 2 --price 495.10")
        else {
            panic!("buy");
        };
        assert_eq!(buy.amount, TradeAmount::Price(495_100_000));
        assert_eq!((buy.fees, buy.rate, buy.note), (0, MICRO, None));

        let Command::Sell(sell) = run(
            "holding sell --account PEA --asset CW8 --quantity 1 --total 980 --fees 1.99 --rate 1.1 --date 2026-09-28 --note x --json",
        ) else {
            panic!("sell");
        };
        assert_eq!(sell.amount, TradeAmount::Total(980 * MICRO));
        assert_eq!((sell.fees, sell.rate), (1_990_000, 1_100_000));
        assert_eq!(sell.target.date.as_deref(), Some("2026-09-28"));
        assert_eq!(sell.note.as_deref(), Some("x"));
        assert!(matches!(
            parse(&args(
                "holding sell --account A --asset B --quantity 1 --total 1 --json"
            )),
            Ok(Invocation::Run { json: true, .. })
        ));
    }

    // CLI-010 — `--help` anywhere asks for help.
    #[test]
    fn cli_010_help_is_help() {
        assert_eq!(parse(&args("holding buy --help")), Ok(Invocation::Help));
        assert_eq!(parse(&args("--help")), Ok(Invocation::Help));
    }

    // CLI-022 — a wrong command line is a usage error naming what is wrong.
    #[test]
    fn cli_022_a_wrong_command_line_is_a_usage_error() {
        let reason = |line: &str| parse(&args(line)).expect_err("usage error").0;
        assert_eq!(reason(""), "a command is missing");
        assert_eq!(reason("account list"), "unknown command \"account\"");
        assert_eq!(reason("holding move"), "unknown command \"holding move\"");
        assert_eq!(
            reason("holding open --account A --asset B --quantity 1 --price 2"),
            "unknown option \"--price\""
        );
        assert_eq!(
            reason("holding buy --account A --asset B --quantity 1"),
            "give either --price or --total"
        );
        assert_eq!(
            reason("holding buy --account A --asset B --quantity 1 --price 1 --total 2"),
            "give either --price or --total"
        );
        assert_eq!(
            reason("holding buy --asset B --quantity 1 --price 1"),
            "--account is missing"
        );
        assert_eq!(
            reason("holding buy --account A --asset B --quantity 1,5 --price 1"),
            "--quantity is not a decimal number: \"1,5\""
        );
        assert_eq!(
            reason("holding buy --account A --asset B --quantity 1 --price 1 --date 28/09/2026"),
            "--date is not a date as YYYY-MM-DD: \"28/09/2026\""
        );
        assert_eq!(
            reason("holding buy --account A --asset B --quantity"),
            "--quantity needs a value"
        );
        assert_eq!(
            reason("holding buy --account A --account C --asset B --quantity 1 --price 1"),
            "--account is given twice"
        );
    }

    // CLI-010 — decimals with a dot, up to six places; a minus is kept for the core to refuse.
    #[test]
    fn cli_010_decimals_become_micro_units() {
        assert_eq!(decimal_to_micro("1"), Some(MICRO));
        assert_eq!(decimal_to_micro("0.000001"), Some(1));
        assert_eq!(decimal_to_micro(".5"), Some(500_000));
        assert_eq!(decimal_to_micro("-2"), Some(-2 * MICRO));
        assert_eq!(decimal_to_micro("1.0000001"), None);
        assert_eq!(decimal_to_micro("1e3"), None);
        assert_eq!(decimal_to_micro(""), None);
        assert_eq!(decimal_to_micro("99999999999999"), None);
    }
}
