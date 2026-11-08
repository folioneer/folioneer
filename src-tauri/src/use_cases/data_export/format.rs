//! The export's format (EXP-023, EXP-030 to EXP-033, EXP-051): the tables as cells, written
//! as CSV (RFC 4180) or JSON, the per-account file names and the import mapper. Pure: no
//! file is touched here.

use std::collections::HashMap;

use crate::context::account::TransactionType;
use crate::context::account_import::{
    DateFormat, DecimalMark, Encoding, ImportMapperDraft, MappedColumns, MapperInfo,
    OperationLabel, OperationType, Separator,
};
use crate::shared::domain::quantity::{quantity_text, scaled_text};

/// EXP-032 — the format's name and version, in the JSON file and in `format.json`.
pub const FORMAT_NAME: &str = "folioneer-export";
/// EXP-032 — raised by any change to the format.
pub const FORMAT_VERSION: u32 = 2;
/// EXP-011C — the characters of an account's name kept in its file name.
const FILE_NAME_LENGTH: usize = 64;

/// One field of a row, by how it is written (EXP-023, EXP-032).
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// A text; empty or `null` when absent.
    Text(Option<String>),
    /// A figure as decimal text; empty or `null` when absent.
    Figure(Option<String>),
    /// `true` or `false`.
    Flag(bool),
    /// A whole number, a number in JSON.
    Number(i64),
}

/// One table: its name, its columns in order (EXP-031) and its rows.
#[derive(Debug, Clone)]
pub struct Table {
    /// `accounts`, `assets`, `transactions`, `prices`, `lots` or `rates`.
    pub name: &'static str,
    /// The columns, in EXP-031's order.
    pub columns: &'static [&'static str],
    /// Each row, one cell per column.
    pub rows: Vec<Vec<Cell>>,
}

/// EXP-031 — which of a transaction's figures its type fills; the others are empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fills {
    /// `unit_price`.
    pub unit_price: bool,
    /// `exchange_rate`.
    pub exchange_rate: bool,
    /// `fees`.
    pub fees: bool,
    /// `total_amount`.
    pub total_amount: bool,
    /// `realized_pnl`.
    pub realized_pnl: bool,
}

/// EXP-031 — the figures each type fills.
pub fn fills(kind: TransactionType) -> Fills {
    use TransactionType::*;
    let trade = matches!(kind, Purchase | Sell);
    let transfer = matches!(kind, TransferOut | TransferIn);
    Fills {
        unit_price: trade || transfer || kind == OpeningBalance,
        exchange_rate: trade || transfer || kind == Dividend,
        fees: trade,
        total_amount: trade
            || transfer
            || matches!(kind, OpeningBalance | Deposit | Withdrawal | Dividend),
        realized_pnl: matches!(kind, Sell | WriteOff),
    }
}

/// EXP-023 — a figure of money, six decimals at most.
pub fn money(micros: i64) -> String {
    scaled_text(micros, 6)
}

/// EXP-023 — a quantity of `asset_id` at its scale (TRX-024).
pub fn quantity(asset_id: &str, stored: i64) -> String {
    quantity_text(asset_id, stored)
}

/// EXP-030 — one field of a CSV line: quoted when it holds a comma, a quote or a line
/// break, its quotes doubled; a text as it is (EXP-025).
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn csv_text(cell: &Cell) -> String {
    match cell {
        Cell::Text(text) | Cell::Figure(text) => csv_field(text.as_deref().unwrap_or("")),
        Cell::Flag(flag) => flag.to_string(),
        Cell::Number(number) => number.to_string(),
    }
}

/// EXP-030 — a table as a CSV file (RFC 4180): its header, then a line per row, every line
/// ended by CRLF.
pub fn csv(table: &Table) -> String {
    let mut written = table.columns.join(",");
    written.push_str("\r\n");
    for row in &table.rows {
        let fields: Vec<String> = row.iter().map(csv_text).collect();
        written.push_str(&fields.join(","));
        written.push_str("\r\n");
    }
    written
}

/// A JSON string literal.
fn json_string(text: &str) -> String {
    serde_json::Value::String(text.to_string()).to_string()
}

fn json_text(cell: &Cell) -> String {
    match cell {
        Cell::Text(text) | Cell::Figure(text) => text
            .as_deref()
            .map_or_else(|| "null".to_string(), json_string),
        Cell::Flag(flag) => flag.to_string(),
        Cell::Number(number) => number.to_string(),
    }
}

/// EXP-032 — the header the JSON file and `format.json` share, in its order.
fn header_lines(exported_at: &str) -> Vec<String> {
    vec![
        format!("  \"format\": {}", json_string(FORMAT_NAME)),
        format!("  \"version\": {FORMAT_VERSION}"),
        format!("  \"exported_at\": {}", json_string(exported_at)),
    ]
}

/// EXP-011A — `format.json`: the format, its version and the moment, without the tables.
pub fn format_file(exported_at: &str) -> String {
    format!("{{\n{}\n}}\n", header_lines(exported_at).join(",\n"))
}

/// EXP-032 — the tables as one JSON object, the header first, each row an object whose keys
/// follow the columns' order — written by hand, as a map would sort them.
pub fn json(tables: &[Table], exported_at: &str) -> String {
    let mut members = header_lines(exported_at);
    for table in tables {
        let rows: Vec<String> = table
            .rows
            .iter()
            .map(|row| {
                let fields: Vec<String> = table
                    .columns
                    .iter()
                    .zip(row)
                    .map(|(column, cell)| format!("{}: {}", json_string(column), json_text(cell)))
                    .collect();
                format!("    {{{}}}", fields.join(", "))
            })
            .collect();
        let array = if rows.is_empty() {
            "[]".to_string()
        } else {
            format!("[\n{}\n  ]", rows.join(",\n"))
        };
        members.push(format!("  {}: {array}", json_string(table.name)));
    }
    format!("{{\n{}\n}}\n", members.join(",\n"))
}

/// EXP-011C — an account's name made a valid file name on any system: its first 64
/// characters, each one a system refuses replaced by `_`.
fn file_safe(name: &str) -> String {
    name.chars()
        .take(FILE_NAME_LENGTH)
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// EXP-011B / EXP-011C — each account's `transactions-{account}.csv`, by its identifier; two names that
/// meet, case ignored, each carry their identifier.
pub fn account_file_names(accounts: &[(String, String)]) -> HashMap<String, String> {
    let mut by_name: HashMap<String, Vec<&(String, String)>> = HashMap::new();
    for account in accounts {
        by_name
            .entry(file_safe(&account.1).to_lowercase())
            .or_default()
            .push(account);
    }
    let mut names = HashMap::new();
    for sharing in by_name.values() {
        for (id, name) in sharing {
            let stem = file_safe(name);
            let file = if sharing.len() > 1 {
                format!("transactions-{stem}-{id}.csv")
            } else {
                format!("transactions-{stem}.csv")
            };
            names.insert(id.clone(), file);
        }
    }
    names
}

/// EXP-014 — the first name `taken` does not hold: `stem`, then `stem (2)`, `stem (3)`…,
/// before the extension when there is one.
pub fn free_name(stem: &str, extension: Option<&str>, taken: impl Fn(&str) -> bool) -> String {
    let named = |suffix: &str| match extension {
        Some(extension) => format!("{stem}{suffix}.{extension}"),
        None => format!("{stem}{suffix}"),
    };
    let first = named("");
    if !taken(&first) {
        return first;
    }
    (2..)
        .map(|n| named(&format!(" ({n})")))
        .find(|name| !taken(name))
        .unwrap_or(first)
}

/// EXP-051 — the import mapper that reads the per-account files of the export of `date`.
pub fn import_mapper_file(date: &str) -> serde_json::Result<Vec<u8>> {
    let column = |name: &str| Some(name.to_string());
    let imported = [
        ("Purchase", OperationType::Purchase),
        ("Sell", OperationType::Sale),
        ("Dividend", OperationType::Dividend),
        ("Deposit", OperationType::Deposit),
        ("Withdrawal", OperationType::Withdrawal),
    ];
    let left_out = [
        "OpeningBalance",
        "FreeShares",
        "ManagementFee",
        "Split",
        "Interest",
        "WriteOff",
        "TransferOut",
        "TransferIn",
    ];
    let operation_types = imported
        .into_iter()
        .chain(
            left_out
                .into_iter()
                .map(|label| (label, OperationType::NotImported)),
        )
        .map(|(label, transaction_type)| OperationLabel {
            label: label.to_string(),
            transaction_type,
        })
        .collect();
    let draft = ImportMapperDraft {
        info: MapperInfo {
            provider: format!("Folioneer export {date}"),
        },
        separator: Separator::Comma,
        decimal_mark: DecimalMark::Dot,
        date_format: DateFormat::YearMonthDay,
        encoding: Encoding::Utf8,
        columns: MappedColumns {
            date: "date".into(),
            operation: "type".into(),
            isin: column("asset_isin"),
            reference: column("asset_reference"),
            name: column("asset_name"),
            quantity: column("quantity"),
            unit_price: column("unit_price"),
            gross_amount: None,
            fees: column("fees"),
            net_amount: column("total_amount"),
            currency: column("account_currency"),
        },
        operation_types,
        asset_matches: Vec::new(),
    };
    serde_json::to_vec_pretty(&draft)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: Vec<Vec<Cell>>) -> Table {
        Table {
            name: "t",
            columns: &["a", "b", "c", "d"],
            rows,
        }
    }

    // EXP-030 — RFC 4180: a comma, a quote or a line break quotes the field, its quotes
    // doubled; every line, the last included, ends with CRLF; a text as it is (EXP-025).
    #[test]
    fn csv_follows_the_standard() {
        let written = csv(&table(vec![vec![
            Cell::Text(Some("a,b".into())),
            Cell::Text(Some("say \"hi\"\nthere".into())),
            Cell::Text(Some("=1+1".into())),
            Cell::Text(None),
        ]]));
        assert_eq!(
            written,
            "a,b,c,d\r\n\"a,b\",\"say \"\"hi\"\"\nthere\",=1+1,\r\n"
        );
    }

    // EXP-032 — the JSON written by hand escapes what JSON must: a quote, a line break, a
    // control character; EXP-030 — a CR alone quotes a CSV field too.
    #[test]
    fn text_is_escaped_where_each_format_needs_it() {
        let tricky = "say \"hi\"\r\nline\u{1}";
        let written = json(
            &[table(vec![vec![
                Cell::Text(Some(tricky.into())),
                Cell::Text(None),
                Cell::Text(None),
                Cell::Text(None),
            ]])],
            "now",
        );
        let value: serde_json::Value = serde_json::from_str(&written).expect("JSON");
        assert_eq!(value["t"][0]["a"], tricky);
        assert!(csv(&table(vec![vec![
            Cell::Text(Some("a\rb".into())),
            Cell::Text(None),
            Cell::Text(None),
            Cell::Text(None),
        ]]))
        .contains("\"a\rb\""));
    }

    // EXP-031 — each type fills its own figures.
    #[test]
    fn each_type_fills_its_figures() {
        use TransactionType::*;
        let all = |f: Fills| {
            (
                f.unit_price,
                f.exchange_rate,
                f.fees,
                f.total_amount,
                f.realized_pnl,
            )
        };
        assert_eq!(all(fills(Purchase)), (true, true, true, true, false));
        assert_eq!(all(fills(Sell)), (true, true, true, true, true));
        assert_eq!(
            all(fills(OpeningBalance)),
            (true, false, false, true, false)
        );
        assert_eq!(all(fills(Deposit)), (false, false, false, true, false));
        assert_eq!(all(fills(Dividend)), (false, true, false, true, false));
        assert_eq!(all(fills(Split)), (false, false, false, false, false));
        assert_eq!(all(fills(WriteOff)), (false, false, false, false, true));
        assert_eq!(all(fills(TransferIn)), (true, true, false, true, false));
        assert_eq!(all(fills(Interest)), (false, false, false, false, false));
    }

    // EXP-030 — a table with no row is its header alone.
    #[test]
    fn an_empty_table_is_its_header() {
        assert_eq!(csv(&table(Vec::new())), "a,b,c,d\r\n");
    }

    // EXP-023 / EXP-032 — a figure is text, a flag a boolean, a number a number, empty null.
    #[test]
    fn json_writes_each_cell_by_its_kind() {
        let written = json(
            &[table(vec![vec![
                Cell::Figure(Some("1.5".into())),
                Cell::Flag(true),
                Cell::Number(3),
                Cell::Figure(None),
            ]])],
            "2026-11-07T10:00:00Z",
        );
        let value: serde_json::Value = serde_json::from_str(&written).expect("JSON");
        assert_eq!(value["format"], "folioneer-export");
        assert_eq!(value["version"], 2);
        assert_eq!(value["exported_at"], "2026-11-07T10:00:00Z");
        assert_eq!(
            value["t"][0],
            serde_json::json!({ "a": "1.5", "b": true, "c": 3, "d": null })
        );
        let keys: Vec<&str> = written
            .lines()
            .filter_map(|line| line.trim().strip_prefix('"'))
            .filter_map(|line| line.split('"').next())
            .collect();
        assert_eq!(&keys[..4], ["format", "version", "exported_at", "t"]);
    }

    // EXP-023 — money to six decimals, a quantity at its asset's scale, no trailing zero.
    #[test]
    fn figures_are_decimal_text() {
        assert_eq!(money(12_500_000), "12.5");
        assert_eq!(money(-3_000_000), "-3");
        assert_eq!(quantity("asset", 12_345_678), "0.12345678");
        assert_eq!(quantity("system-cash-eur", 1_500_000), "1.5");
    }

    // EXP-011C — a file name holds no character a system refuses, 64 characters at most, and
    // two names that meet, case ignored, each carry their identifier.
    #[test]
    fn account_file_names_stay_valid_and_apart() {
        let long = "x".repeat(80);
        let names = account_file_names(&[
            ("a1".into(), "PEA: main/2".into()),
            ("a2".into(), "Main".into()),
            ("a3".into(), "main".into()),
            ("a4".into(), long.clone()),
        ]);
        assert_eq!(names["a1"], "transactions-PEA_ main_2.csv");
        assert_eq!(names["a2"], "transactions-Main-a2.csv");
        assert_eq!(names["a3"], "transactions-main-a3.csv");
        assert_eq!(names["a4"], format!("transactions-{}.csv", "x".repeat(64)));
    }

    // EXP-014 — never over what is there: the first free " (n)".
    #[test]
    fn a_taken_name_takes_the_first_free_number() {
        let taken = [
            "folioneer-export-2026-11-07.json",
            "folioneer-export-2026-11-07 (2).json",
        ];
        let free = free_name("folioneer-export-2026-11-07", Some("json"), |name| {
            taken.contains(&name)
        });
        assert_eq!(free, "folioneer-export-2026-11-07 (3).json");
        assert_eq!(
            free_name("folioneer-export-2026-11-07", None, |_| false),
            "folioneer-export-2026-11-07"
        );
    }

    // EXP-051 — the mapper written beside the export is one the import takes.
    #[test]
    fn the_mapper_is_one_the_import_takes() {
        let bytes = import_mapper_file("2026-11-07").expect("file");
        let draft =
            crate::context::account_import::ImportMapperDraft::from_file(&bytes).expect("reads");
        let mapper = crate::context::account_import::ImportMapper::with_id(String::new(), draft)
            .expect("valid");
        assert_eq!(mapper.info.provider, "Folioneer export 2026-11-07");
        assert!(mapper.asset_matches.is_empty());
        assert_eq!(mapper.operation_types.len(), 13);
    }
}
