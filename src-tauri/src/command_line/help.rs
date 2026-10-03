//! What the command line prints about itself (CLI-016, CLI-023 to CLI-025): the overview, one page
//! per command, the hint after a mistake and the closest name to a mistyped one.

/// A page of help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTopic {
    /// The commands, with examples.
    Overview,
    /// `holding open`.
    Open,
    /// `holding buy`.
    Buy,
    /// `holding sell`.
    Sell,
}

/// Every command, as typed after the program name, with its page — in the overview's order.
const PAGES: [(&str, HelpTopic); 3] = [
    ("holding open", HelpTopic::Open),
    ("holding buy", HelpTopic::Buy),
    ("holding sell", HelpTopic::Sell),
];

const EXIT_CODES: &str = "Exit codes: 0 recorded, 1 refused, 2 wrong usage.";

/// The commands, as typed after the program name.
pub fn commands() -> Vec<&'static str> {
    PAGES.iter().map(|(command, _)| *command).collect()
}

impl HelpTopic {
    /// The page of a command as typed, or the overview when it names none.
    pub fn of(group: Option<&str>, verb: Option<&str>) -> Self {
        let typed = format!("{} {}", group.unwrap_or(""), verb.unwrap_or(""));
        PAGES
            .iter()
            .find(|(command, _)| *command == typed)
            .map_or(Self::Overview, |(_, topic)| *topic)
    }

    /// What to type to read this page.
    pub fn invocation(self, program: &str) -> String {
        match PAGES.iter().find(|(_, topic)| *topic == self) {
            Some((command, _)) => format!("{program} {command} --help"),
            None => format!("{program} --help"),
        }
    }
}

/// CLI-016 — the page of help for `topic`, naming the program as the user types it.
pub fn help(program: &str, topic: HelpTopic) -> String {
    match topic {
        HelpTopic::Overview => overview(program),
        HelpTopic::Open => page(
            program,
            "holding open",
            "Add an asset you already hold to an account, as an opening balance.",
            &[
                "--account <name> --asset <name> --quantity <n>",
                "--total-cost <amount> [options]",
            ],
            &[
                ("--account <name>", "Account name"),
                ("--asset <name>", "Asset name or reference"),
                ("--quantity <n>", "Quantity held"),
                ("--total-cost <amount>", "What the position cost in total"),
            ],
            &[("--date <YYYY-MM-DD>", "Date", Some("today"))],
            "--account PEA --asset \"Air Liquide\" --quantity 10 --total-cost 1520",
        ),
        HelpTopic::Buy => page(
            program,
            "holding buy",
            "Record a purchase of an asset in an account.",
            &[
                "--account <name> --asset <name> --quantity <n>",
                "(--price <amount> | --total <amount>) [options]",
            ],
            &[
                ("--account <name>", "Account name"),
                ("--asset <name>", "Asset name or reference"),
                ("--quantity <n>", "Quantity bought"),
                ("--price <amount>", "Unit price"),
                (
                    "--total <amount>",
                    "or, in place of --price: total paid, fees included",
                ),
            ],
            &TRADE_OPTIONS,
            "--account PEA --asset ASML --quantity 1 --price 1236.50",
        ),
        HelpTopic::Sell => page(
            program,
            "holding sell",
            "Record a sale of an asset in an account.",
            &[
                "--account <name> --asset <name> --quantity <n>",
                "(--price <amount> | --total <amount>) [options]",
            ],
            &[
                ("--account <name>", "Account name"),
                ("--asset <name>", "Asset name or reference"),
                ("--quantity <n>", "Quantity sold"),
                ("--price <amount>", "Unit price"),
                (
                    "--total <amount>",
                    "or, in place of --price: total received, fees deducted",
                ),
            ],
            &TRADE_OPTIONS,
            "--account PEA --asset ASML --quantity 1 --total 1300",
        ),
    }
}

const TRADE_OPTIONS: [(&str, &str, Option<&str>); 4] = [
    ("--fees <amount>", "Fees", Some("0")),
    ("--rate <rate>", "Exchange rate", Some("1")),
    ("--date <YYYY-MM-DD>", "Date", Some("today")),
    ("--note <text>", "Note", None),
];

fn overview(program: &str) -> String {
    format!(
        "\
Record holdings in your Folioneer portfolio from a terminal.

Usage: {program} <command> [options]

Commands:
  holding open   Add an asset you already hold to an account (opening balance)
  holding buy    Record a purchase
  holding sell   Record a sale

Examples:
  {program} holding buy  --account PEA --asset ASML --quantity 1 --price 1236.50
  {program} holding open --account PEA --asset \"Air Liquide\" --quantity 10 --total-cost 1520
  {program} holding sell --account PEA --asset ASML --quantity 1 --total 1300 --json

Amounts and quantities are decimals with a dot; dates are YYYY-MM-DD.
Run '{program} <command> --help' for a command's options.
Close the Folioneer window before recording.
{EXIT_CODES}"
    )
}

/// One command's page: what it does, its usage on aligned lines, one option per line with
/// its default, an example and the exit codes.
fn page(
    program: &str,
    command: &str,
    summary: &str,
    usage: &[&str],
    required: &[(&str, &str)],
    optional: &[(&str, &str, Option<&str>)],
    example: &str,
) -> String {
    const ALWAYS: [(&str, &str, Option<&str>); 2] = [
        ("--json", "Print the result as JSON", None),
        ("-h, --help", "Show this help", None),
    ];
    let lead = format!("Usage: {program} {command} ");
    let usage = usage.join(&format!("\n{}", " ".repeat(lead.len())));
    let optional: Vec<&(&str, &str, Option<&str>)> = optional.iter().chain(&ALWAYS).collect();
    let name_width = required
        .iter()
        .map(|(name, _)| name.len())
        .chain(optional.iter().map(|(name, ..)| name.len()))
        .max()
        .unwrap_or(0);
    let text_width = optional
        .iter()
        .filter(|(.., default)| default.is_some())
        .map(|(_, text, _)| text.len())
        .max()
        .unwrap_or(0);
    let required: String = required
        .iter()
        .map(|(name, text)| format!("  {name:<name_width$}   {text}\n"))
        .collect();
    let optional: String = optional
        .iter()
        .map(|(name, text, default)| match default {
            Some(default) => {
                format!("  {name:<name_width$}   {text:<text_width$}   [default: {default}]\n")
            }
            None => format!("  {name:<name_width$}   {text}\n"),
        })
        .collect();
    format!(
        "\
{summary}

{lead}{usage}

Required:
{required}
Options:
{optional}
Example:
  {program} {command} {example}

{EXIT_CODES}"
    )
}

/// CLI-025 — the candidate closest to what was typed, when it is close enough to be a
/// typing mistake: at most two letters apart, and fewer than the letters typed.
pub fn closest<'a>(typed: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .map(|candidate| (distance(typed, candidate), *candidate))
        .filter(|(distance, _)| *distance <= 2 && *distance < typed.chars().count())
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate)
}

/// The number of letters to add, remove or replace to turn one text into the other.
fn distance(from: &str, to: &str) -> usize {
    let to: Vec<char> = to.chars().collect();
    let mut row: Vec<usize> = (0..=to.len()).collect();
    for (typed, letter) in from.chars().enumerate() {
        let mut next = vec![typed + 1];
        for (pair, other) in row.windows(2).zip(&to) {
            if let [diagonal, above] = pair {
                let left = next.last().copied().unwrap_or(0);
                let replaced = diagonal + usize::from(letter != *other);
                next.push(replaced.min(above + 1).min(left + 1));
            }
        }
        row = next;
    }
    row.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // CLI-016 — the overview: a description, the commands with one line each, examples,
    // and where to read a command's options. The program is named as the user types it.
    #[test]
    fn cli_016_the_overview_lists_the_commands_with_examples() {
        let text = help("folioneer-cli", HelpTopic::Overview);

        assert!(text.starts_with("Record holdings in your Folioneer portfolio from a terminal.\n"));
        assert!(text.contains("\nUsage: folioneer-cli <command> [options]\n"));
        for command in commands() {
            assert!(text.contains(&format!("\n  {command}  ")), "{command}");
        }
        assert_eq!(text.matches("\n  folioneer-cli holding ").count(), 3);
        assert!(text.contains("Run 'folioneer-cli <command> --help' for a command's options."));
        assert!(text.ends_with(EXIT_CODES));
    }

    // CLI-023 — a command's page shows that command only: its usage, one option per line,
    // aligned, with its default, then an example.
    #[test]
    fn cli_023_a_command_page_shows_one_option_per_line_with_its_default() {
        assert_eq!(
            help("folioneer", HelpTopic::Buy),
            "\
Record a purchase of an asset in an account.

Usage: folioneer holding buy --account <name> --asset <name> --quantity <n>
                             (--price <amount> | --total <amount>) [options]

Required:
  --account <name>      Account name
  --asset <name>        Asset name or reference
  --quantity <n>        Quantity bought
  --price <amount>      Unit price
  --total <amount>      or, in place of --price: total paid, fees included

Options:
  --fees <amount>       Fees            [default: 0]
  --rate <rate>         Exchange rate   [default: 1]
  --date <YYYY-MM-DD>   Date            [default: today]
  --note <text>         Note
  --json                Print the result as JSON
  -h, --help            Show this help

Example:
  folioneer holding buy --account PEA --asset ASML --quantity 1 --price 1236.50

Exit codes: 0 recorded, 1 refused, 2 wrong usage."
        );
    }

    // CLI-023 — an opening balance and a sale have their own pages, with their own options.
    #[test]
    fn cli_023_each_command_has_its_own_page() {
        let open = help("folioneer", HelpTopic::Open);
        assert!(open.contains("\n  --total-cost <amount>   What the position cost in total\n"));
        assert!(!open.contains("--price"));
        assert!(!open.contains("--fees"));

        let sell = help("folioneer", HelpTopic::Sell);
        assert!(sell.starts_with("Record a sale of an asset in an account.\n"));
        assert!(sell.contains("Usage: folioneer holding sell --account"));
        assert!(sell.contains("\n  --fees <amount>"));
    }

    // CLI-024 — the hint names the page of the command that was mistyped.
    #[test]
    fn cli_024_a_page_is_read_by_its_own_help_option() {
        assert_eq!(
            HelpTopic::Buy.invocation("folioneer"),
            "folioneer holding buy --help"
        );
        assert_eq!(
            HelpTopic::Overview.invocation("folioneer-cli"),
            "folioneer-cli --help"
        );
        assert_eq!(
            HelpTopic::of(Some("holding"), Some("sell")),
            HelpTopic::Sell
        );
        assert_eq!(
            HelpTopic::of(Some("holding"), Some("move")),
            HelpTopic::Overview
        );
        assert_eq!(HelpTopic::of(None, None), HelpTopic::Overview);
    }

    // CLI-025 — a name one or two letters away is suggested; anything further is not.
    #[test]
    fn cli_025_the_closest_name_is_suggested_when_close_enough() {
        assert_eq!(closest("holding buuy", &commands()), Some("holding buy"));
        assert_eq!(closest("holdng sell", &commands()), Some("holding sell"));
        assert_eq!(closest("holding move", &commands()), None);
        assert_eq!(
            closest("--acount", &["--account", "--asset"]),
            Some("--account")
        );
        assert_eq!(closest("--ASML", &["--account", "--asset"]), None);
        assert_eq!(closest("-x", &["--json"]), None);
    }
}
