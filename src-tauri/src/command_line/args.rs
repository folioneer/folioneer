//! Reading a command line (CLI-010): the command, its options and their values, in
//! micro-units (TRX-024). Anything wrong is a usage error (CLI-022).

use crate::context::asset::{AssetClass, AssetCreationDefaults, AssetKind, NamedAsset};

use crate::use_cases::holding_transaction::{
    decimal_to_micro, Recording, Target, Trade, TradeAmount,
};

use super::help::{closest, commands, HelpTopic};

const MICRO: i64 = 1_000_000;

/// A command that lists what the others can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listed {
    /// The accounts (CLI-018).
    Accounts,
    /// The assets a command can name (CLI-019), archived ones too when asked.
    Assets { archived: bool },
}

/// A command that writes: refused while the window is open (CLI-030).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Writing {
    /// Record a transaction.
    Record(Recording),
    /// Add an asset (CLI-026).
    AddAsset(NamedAsset),
}

/// A command the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Record a transaction or add an asset.
    Write(Writing),
    /// List accounts or assets; reads only.
    List(Listed),
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Run a command, printing text or JSON.
    Run { command: Command, json: bool },
    /// Print a page of help.
    Help(HelpTopic),
}

/// A command line that cannot be run: the reason, and the page of help to point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError {
    /// What is wrong, with the closest name when one was mistyped (CLI-025).
    pub message: String,
    /// The page that says how to type it (CLI-024).
    pub topic: HelpTopic,
}

/// Why a command line cannot be run.
struct Reason(String);

/// CLI-010 — reads the arguments after the program name.
pub fn parse(args: &[String]) -> Result<Invocation, UsageError> {
    let words: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|arg| !is_help(arg))
        .collect();
    let topic = HelpTopic::of(words.first().copied(), words.get(1).copied());
    if args.iter().any(|arg| is_help(arg)) {
        return Ok(Invocation::Help(topic));
    }
    read(args).map_err(|Reason(message)| UsageError { message, topic })
}

fn is_help(argument: &str) -> bool {
    argument == "--help" || argument == "-h"
}

/// `unknown <what> "<typed>"`, with the closest of `candidates` when it is a typing mistake
/// (CLI-025). What was typed is echoed with its control characters escaped.
fn unknown(what: &str, typed: &str, candidates: &[&str]) -> Reason {
    let echoed = typed.escape_debug();
    Reason(match closest(typed, candidates) {
        Some(meant) => format!("unknown {what} \"{echoed}\". Did you mean \"{meant}\"?"),
        None => format!("unknown {what} \"{echoed}\""),
    })
}

fn read(args: &[String]) -> Result<Invocation, Reason> {
    let (group, verb, rest) = match args {
        [group, verb, rest @ ..] => (group.as_str(), verb.as_str(), rest),
        [group] if group != "holding" => return Err(unknown("command", group, &commands())),
        _ => return Err(Reason("a command is missing".to_string())),
    };
    let command = format!("{group} {verb}");
    match command.as_str() {
        "account list" => {
            let (_, json) = flags(rest, &[])?;
            return Ok(Invocation::Run {
                command: Command::List(Listed::Accounts),
                json,
            });
        }
        "asset list" => {
            let (given, json) = flags(rest, &["--archived"])?;
            return Ok(Invocation::Run {
                command: Command::List(Listed::Assets {
                    archived: given.contains(&"--archived"),
                }),
                json,
            });
        }
        _ => {}
    }
    let allowed: &[&str] = match command.as_str() {
        "holding open" => &[
            "--account",
            "--asset",
            "--quantity",
            "--total-cost",
            "--date",
        ],
        "holding buy" | "holding sell" => &[
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
        "asset add" => &[
            "--name",
            "--reference",
            "--kind",
            "--class",
            "--currency",
            "--isin",
            "--exchange",
            "--risk",
            "--category",
        ],
        _ => return Err(unknown("command", &command, &commands())),
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
            let known: Vec<&str> = allowed
                .iter()
                .copied()
                .chain(["--json", "--help"])
                .collect();
            return Err(unknown("option", name, &known));
        }
        let Some(value) = rest.get(index + 1) else {
            return Err(Reason(format!("{name} needs a value")));
        };
        if options.values.iter().any(|(seen, _)| seen == name) {
            return Err(Reason(format!("{name} is given twice")));
        }
        options.values.push((name.to_string(), value.clone()));
        index += 2;
    }
    if command == "asset add" {
        return Ok(Invocation::Run {
            command: Command::Write(Writing::AddAsset(NamedAsset {
                name: options.text("--name")?,
                reference: options.text("--reference")?,
                kind: options.kind()?,
                class: options.class(options.kind()?)?,
                currency: options.text("--currency")?,
                isin: options.get("--isin").map(str::to_string),
                exchange_code: options.get("--exchange").map(str::to_string),
                risk_level: options.risk()?,
                category_name: options.get("--category").map(str::to_string),
            })),
            json,
        });
    }
    let target = Target {
        account: options.text("--account")?,
        asset: options.text("--asset")?,
        date: options.date()?,
        quantity: options.amount("--quantity")?,
    };
    let command = match verb {
        "open" => Recording::Open {
            target,
            total_cost: options.amount("--total-cost")?,
        },
        _ => {
            let amount = match (options.get("--price"), options.get("--total")) {
                (Some(_), None) => TradeAmount::Price(options.amount("--price")?),
                (None, Some(_)) => TradeAmount::Total(options.amount("--total")?),
                _ => return Err(Reason("give either --price or --total".to_string())),
            };
            let trade = Trade {
                target,
                amount,
                fees: options.amount_or("--fees", 0)?,
                rate: options.amount_or("--rate", MICRO)?,
                note: options.get("--note").map(str::to_string),
            };
            if verb == "buy" {
                Recording::Buy(trade)
            } else {
                Recording::Sell(trade)
            }
        }
    };
    Ok(Invocation::Run {
        command: Command::Write(Writing::Record(command)),
        json,
    })
}

/// Reads the options of a command that takes no value: which of `allowed` were given, and
/// whether `--json` was.
fn flags<'a>(rest: &[String], allowed: &[&'a str]) -> Result<(Vec<&'a str>, bool), Reason> {
    let mut given = Vec::new();
    let mut json = false;
    for name in rest.iter().map(String::as_str) {
        if name == "--json" {
            json = true;
        } else if let Some(flag) = allowed.iter().find(|flag| **flag == name) {
            given.push(*flag);
        } else {
            let known: Vec<&str> = allowed
                .iter()
                .copied()
                .chain(["--json", "--help"])
                .collect();
            return Err(unknown("option", name, &known));
        }
    }
    Ok((given, json))
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

    fn text(&self, name: &str) -> Result<String, Reason> {
        match self.get(name).map(str::trim) {
            Some(value) if !value.is_empty() => Ok(value.to_string()),
            _ => Err(Reason(format!("{name} is missing"))),
        }
    }

    fn amount(&self, name: &str) -> Result<i64, Reason> {
        let value = self
            .get(name)
            .ok_or_else(|| Reason(format!("{name} is missing")))?;
        // CLI-015 — a comma is never read: the decimals are written after a dot, whatever
        // the system's language. The figure so written is shown only when it is one.
        decimal_to_micro(value).ok_or_else(|| {
            let hint = comma_hint(value).unwrap_or_default();
            Reason(format!("{name} is not a decimal number: \"{value}\"{hint}"))
        })
    }

    fn amount_or(&self, name: &str, default: i64) -> Result<i64, Reason> {
        match self.get(name) {
            Some(_) => self.amount(name),
            None => Ok(default),
        }
    }

    /// CLI-026 — `--kind` as one of the kinds the core lets a user add an asset of, case
    /// ignored.
    fn kind(&self) -> Result<Option<AssetKind>, Reason> {
        let Some(typed) = self.get("--kind") else {
            return Ok(None);
        };
        let kinds: Vec<AssetKind> = AssetCreationDefaults::current()
            .kinds
            .into_iter()
            .map(|form| form.kind)
            .collect();
        kinds
            .iter()
            .find(|kind| kind.to_string().eq_ignore_ascii_case(typed))
            .copied()
            .map(Some)
            .ok_or_else(|| {
                let names: Vec<String> = kinds.iter().map(ToString::to_string).collect();
                Reason(format!(
                    "--kind is not one of {}: \"{}\"",
                    names.join(", "),
                    typed.escape_debug()
                ))
            })
    }

    /// CLI-026 — `--class` as one of the classes the core lets a user add an asset in, case
    /// ignored; left out, it is the class the core preselects for `kind` (AST-037).
    fn class(&self, kind: Option<AssetKind>) -> Result<AssetClass, Reason> {
        if self.get("--class").is_none() {
            if let Some(class) = kind.and_then(AssetCreationDefaults::class_of) {
                return Ok(class);
            }
        }
        let typed = self.text("--class")?;
        let addable = AssetClass::user_addable();
        addable
            .iter()
            .find(|class| class.to_string().eq_ignore_ascii_case(&typed))
            .cloned()
            .ok_or_else(|| {
                let names: Vec<String> = addable.iter().map(ToString::to_string).collect();
                Reason(format!(
                    "--class is not one of {}: \"{}\"",
                    names.join(", "),
                    typed.escape_debug()
                ))
            })
    }

    /// `--risk` as a whole number; its range is the core's to judge (CLI-012).
    fn risk(&self) -> Result<Option<u8>, Reason> {
        self.get("--risk")
            .map(|value| {
                value.trim().parse::<u8>().map_err(|_| {
                    Reason(format!(
                        "--risk is not a whole number: \"{}\"",
                        value.escape_debug()
                    ))
                })
            })
            .transpose()
    }

    fn date(&self) -> Result<Option<String>, Reason> {
        match self.get("--date") {
            None => Ok(None),
            Some(value) if is_iso_date(value) => Ok(Some(value.to_string())),
            Some(value) => Err(Reason(format!(
                "--date is not a date as YYYY-MM-DD: \"{value}\""
            ))),
        }
    }
}

/// CLI-015 — what to write in place of a figure typed with a comma, when replacing the comma
/// by a dot makes it a decimal; `None` for anything else ("1,234.5", "1,2,3", "abc").
fn comma_hint(value: &str) -> Option<String> {
    let dotted = value.replace(',', ".");
    (value.contains(',') && decimal_to_micro(&dotted).is_some())
        .then(|| format!(" — write the decimals after a dot: {dotted}"))
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

    fn run(line: &str) -> Recording {
        match parse(&args(line)).expect("valid") {
            Invocation::Run {
                command: Command::Write(Writing::Record(recording)),
                ..
            } => recording,
            other => panic!("not a recording: {other:?}"),
        }
    }

    // CLI-010 — an opening balance reads its quantity and total cost; the date defaults.
    #[test]
    fn cli_010_reads_an_opening_balance() {
        assert_eq!(
            run("holding open --account PEA --asset CW8 --quantity 10 --total-cost 4950.5"),
            Recording::Open {
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
        let Recording::Buy(buy) =
            run("holding buy --account PEA --asset CW8 --quantity 2 --price 495.10")
        else {
            panic!("buy");
        };
        assert_eq!(buy.amount, TradeAmount::Price(495_100_000));
        assert_eq!((buy.fees, buy.rate, buy.note), (0, MICRO, None));

        let Recording::Sell(sell) = run(
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

    // CLI-018 / CLI-019 — the list commands take no value: `--json`, and `--archived` for assets.
    #[test]
    fn cli_018_reads_the_list_commands() {
        let read = |line: &str| parse(&args(line)).expect("valid");
        assert_eq!(
            read("account list"),
            Invocation::Run {
                command: Command::List(Listed::Accounts),
                json: false
            }
        );
        assert_eq!(
            read("asset list --json"),
            Invocation::Run {
                command: Command::List(Listed::Assets { archived: false }),
                json: true
            }
        );
        assert_eq!(
            read("asset list --archived"),
            Invocation::Run {
                command: Command::List(Listed::Assets { archived: true }),
                json: false
            }
        );
        assert_eq!(
            read("asset list --help"),
            Invocation::Help(HelpTopic::AssetList)
        );
    }

    // CLI-026 — `asset add` reads its four required options, the class with case ignored,
    // and leaves out what the core decides.
    #[test]
    fn cli_026_reads_an_asset_to_add() {
        let read = |line: &str| parse(&args(line));
        assert_eq!(
            read("asset add --name ASML --reference asml --class stocks --currency EUR --json"),
            Ok(Invocation::Run {
                command: Command::Write(Writing::AddAsset(NamedAsset {
                    name: "ASML".to_string(),
                    reference: "asml".to_string(),
                    kind: None,
                    class: AssetClass::Stocks,
                    currency: "EUR".to_string(),
                    isin: None,
                    exchange_code: None,
                    risk_level: None,
                    category_name: None,
                })),
                json: true,
            })
        );
        let Ok(Invocation::Run {
            command: Command::Write(Writing::AddAsset(full)),
            ..
        }) = read(
            "asset add --name A --reference B --class ETF --currency USD --isin US0378331005 --exchange xnas --risk 3 --category Tech",
        ) else {
            panic!("asset add");
        };
        assert_eq!(full.isin.as_deref(), Some("US0378331005"));
        assert_eq!(full.exchange_code.as_deref(), Some("xnas"));
        assert_eq!(full.risk_level, Some(3));
        assert_eq!(full.category_name.as_deref(), Some("Tech"));

        let named = |line: &str| match read(line) {
            Ok(Invocation::Run {
                command: Command::Write(Writing::AddAsset(named)),
                ..
            }) => named,
            other => panic!("asset add: {other:?}"),
        };
        let crypto = named("asset add --name Bitcoin --reference BTC --kind crypto --currency EUR");
        assert_eq!(
            (crypto.kind, crypto.class),
            (Some(AssetKind::Crypto), AssetClass::DigitalAsset)
        );
        let custom = named("asset add --name Flat --reference FLAT --kind CUSTOM --currency EUR");
        assert_eq!(custom.class, AssetClass::RealEstate);
        let both =
            named("asset add --name A --reference B --kind Custom --class Bonds --currency EUR");
        assert_eq!(
            (both.kind, both.class),
            (Some(AssetKind::Custom), AssetClass::Bonds)
        );

        let reason = |line: &str| read(line).expect_err("usage error").message;
        assert_eq!(
            reason("asset add --reference B --class ETF --currency USD"),
            "--name is missing"
        );
        assert_eq!(
            reason("asset add --name A --reference B --currency USD"),
            "--class is missing"
        );
        assert_eq!(
            reason("asset add --name A --reference B --kind Cash --currency USD"),
            "--kind is not one of Listed, Crypto, Custom: \"Cash\""
        );
        assert_eq!(
            reason("asset add --name A --reference B --class Cash --currency USD"),
            "--class is not one of RealEstate, Stocks, Bonds, ETF, ETP, MutualFunds, DigitalAsset, Derivatives: \"Cash\""
        );
        assert_eq!(
            reason("asset add --name A --reference B --class ETF --currency USD --risk high"),
            "--risk is not a whole number: \"high\""
        );
        assert_eq!(
            read("asset add --name A --reference B --class ETF --currency USD --quantity 1")
                .expect_err("usage error")
                .topic,
            HelpTopic::AssetAdd
        );
    }

    // CLI-016 — `--help` or `-h` anywhere asks for help: the page of the command it comes
    // with, the overview when it comes with none.
    #[test]
    fn cli_016_help_is_the_page_of_its_command() {
        let page = |line: &str| parse(&args(line));
        assert_eq!(
            page("holding buy --help"),
            Ok(Invocation::Help(HelpTopic::Buy))
        );
        assert_eq!(
            page("holding -h sell --account A"),
            Ok(Invocation::Help(HelpTopic::Sell))
        );
        assert_eq!(
            page("holding open -h"),
            Ok(Invocation::Help(HelpTopic::Open))
        );
        assert_eq!(
            page("holding --help"),
            Ok(Invocation::Help(HelpTopic::Overview))
        );
        assert_eq!(page("--help"), Ok(Invocation::Help(HelpTopic::Overview)));
        assert_eq!(page("-h"), Ok(Invocation::Help(HelpTopic::Overview)));
    }

    // CLI-024 — a usage error points at the page of the command it was typed for.
    #[test]
    fn cli_024_a_usage_error_names_the_page_to_read() {
        let topic = |line: &str| parse(&args(line)).expect_err("usage error").topic;
        assert_eq!(topic("holding buy --account A"), HelpTopic::Buy);
        assert_eq!(topic("holding open --price 1"), HelpTopic::Open);
        assert_eq!(topic("holding move"), HelpTopic::Overview);
        assert_eq!(topic(""), HelpTopic::Overview);
    }

    // CLI-025 — a mistyped command or option is answered with the closest one.
    #[test]
    fn cli_025_a_mistyped_name_is_answered_with_the_closest() {
        let reason = |line: &str| parse(&args(line)).expect_err("usage error").message;
        assert_eq!(
            reason("holding buuy"),
            "unknown command \"holding buuy\". Did you mean \"holding buy\"?"
        );
        assert_eq!(
            reason("holdin sell --account A"),
            "unknown command \"holdin sell\". Did you mean \"holding sell\"?"
        );
        assert_eq!(
            reason("holding buy --acount A"),
            "unknown option \"--acount\". Did you mean \"--account\"?"
        );
        assert_eq!(
            reason("holding buy --account A --jsn"),
            "unknown option \"--jsn\". Did you mean \"--json\"?"
        );
        assert_eq!(reason("holding buy --ASML"), "unknown option \"--ASML\"");
        // Several as close: the first in the page's order.
        assert_eq!(
            reason("holding buy --nate x"),
            "unknown option \"--nate\". Did you mean \"--rate\"?"
        );
    }

    // CLI-022 — a wrong command line is a usage error naming what is wrong.
    #[test]
    fn cli_022_a_wrong_command_line_is_a_usage_error() {
        let reason = |line: &str| parse(&args(line)).expect_err("usage error").message;
        assert_eq!(reason(""), "a command is missing");
        assert_eq!(reason("account show"), "unknown command \"account show\"");
        assert_eq!(
            reason("account list --archived"),
            "unknown option \"--archived\""
        );
        assert_eq!(
            reason("asset list --archivd"),
            "unknown option \"--archivd\". Did you mean \"--archived\"?"
        );
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
            "--quantity is not a decimal number: \"1,5\" — write the decimals after a dot: 1.5"
        );
        // CLI-015 — the hint shows a figure only when it is one: not for a comma used
        // between thousands, several commas, or no comma at all.
        for typed in ["abc", "1,234.5", "1,2,3"] {
            assert_eq!(
                reason(&format!(
                    "holding buy --account A --asset B --quantity {typed} --price 1"
                )),
                format!("--quantity is not a decimal number: \"{typed}\"")
            );
        }
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
