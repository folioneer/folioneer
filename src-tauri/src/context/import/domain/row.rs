use crate::context::import::domain::file::{read_date, FileLine, ImportFile};
use crate::context::import::domain::{
    DateFormat, DecimalMark, MappedColumns, OperationLabel, OperationType,
};
use crate::context::import::error::ImportError;
use serde::{Deserialize, Serialize};
use specta::Type;

/// One unit in micro-units (TRX-024).
const MICRO: i128 = 1_000_000;

/// The asset a row names, each field as read, trimmed; empty ones are `None` (IMP-032).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AssetRead {
    /// The ISIN read.
    pub isin: Option<String>,
    /// The reference read.
    pub reference: Option<String>,
    /// The name read.
    pub name: Option<String>,
}

/// IMP-031 — a row's figures for its type, in micro-units, signs ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type")]
pub enum RowFigures {
    /// A purchase: its quantity, unit price and fees, a joined trade tax included (IMP-034).
    Purchase {
        /// Quantity bought.
        quantity: i64,
        /// Unit price.
        unit_price: i64,
        /// Fees.
        fees: i64,
    },
    /// A sale: its quantity, unit price and fees.
    Sale {
        /// Quantity sold.
        quantity: i64,
        /// Unit price.
        unit_price: i64,
        /// Fees.
        fees: i64,
    },
    /// A dividend's amount.
    Dividend {
        /// Amount.
        amount: i64,
    },
    /// A deposit's amount.
    Deposit {
        /// Amount.
        amount: i64,
    },
    /// A withdrawal's amount.
    Withdrawal {
        /// Amount.
        amount: i64,
    },
    /// A trade tax: its amount, and its quantity when the row has one (IMP-034).
    TradeTax {
        /// Amount.
        amount: i64,
        /// Quantity, when the row has one.
        quantity: Option<i64>,
    },
}

/// A row read as an operation, before it is matched and judged (IMP-030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OperationRead {
    /// The day, `YYYY-MM-DD`.
    pub date: String,
    /// The asset as read; all `None` for a deposit and a withdrawal (IMP-032).
    pub asset: AssetRead,
    /// The row's own currency, when the mapping names its column and the field holds one.
    pub currency: Option<String>,
    /// Its figures.
    pub figures: RowFigures,
}

/// Why a row is rejected before it is matched and judged (IMP-016, IMP-022, IMP-030,
/// IMP-031, IMP-034).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "code")]
pub enum RowRejection {
    /// The line holds fewer fields than the header (IMP-016).
    LineTooShort,
    /// The operation label has no operation type in the mapping (IMP-022).
    OperationTypeMissing {
        /// The label read.
        label: String,
    },
    /// The date does not read in the date format (IMP-030).
    DateUnreadable {
        /// The date column's header.
        column: String,
    },
    /// A figure does not read (IMP-030).
    FigureUnreadable {
        /// The figure's column header.
        column: String,
    },
    /// A figure the type needs is missing; `field` names the mapped field (IMP-031).
    FigureMissing {
        /// The mapped field, as `MappedColumns` names it.
        field: String,
    },
    /// A purchase or a sale of a quantity of zero (TRX-020).
    QuantityZero,
    /// A trade tax with no purchase of its asset, date and quantity in the file (IMP-034).
    TradeTaxUnpaired,
}

/// What a line of the file reads as (IMP-030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "status")]
pub enum RowRead {
    /// An operation, to be matched and judged.
    Operation(OperationRead),
    /// A trade tax added to the purchase on line `purchase_line` (IMP-034).
    JoinedTradeTax {
        /// The purchase's line number.
        purchase_line: u32,
    },
    /// A label the mapping leaves to record by hand (IMP-022).
    NotImported {
        /// The label read.
        label: String,
    },
    /// Rejected before it is matched.
    Rejected(RowRejection),
}

/// A line of the file and what it reads as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Row {
    /// Its line number in the file, from 1 for the header.
    pub line: u32,
    /// What it reads as.
    pub read: RowRead,
}

/// Where each mapped column sits in the file (IMP-021).
struct Located<'a> {
    columns: &'a MappedColumns,
    date: usize,
    operation: usize,
    isin: Option<usize>,
    reference: Option<usize>,
    name: Option<usize>,
    quantity: Option<usize>,
    unit_price: Option<usize>,
    gross_amount: Option<usize>,
    fees: Option<usize>,
    net_amount: Option<usize>,
    currency: Option<usize>,
}

/// IMP-021 — the position of every mapped column, a column the file lacks refused.
fn locate<'a>(columns: &'a MappedColumns, file: &ImportFile) -> Result<Located<'a>, ImportError> {
    let find = |name: &str| {
        file.column(name)
            .ok_or_else(|| ImportError::MappedColumnNotInFile {
                column: name.trim().to_string(),
            })
    };
    let optional = |name: &Option<String>| name.as_deref().map(find).transpose();
    Ok(Located {
        columns,
        date: find(&columns.date)?,
        operation: find(&columns.operation)?,
        isin: optional(&columns.isin)?,
        reference: optional(&columns.reference)?,
        name: optional(&columns.name)?,
        quantity: optional(&columns.quantity)?,
        unit_price: optional(&columns.unit_price)?,
        gross_amount: optional(&columns.gross_amount)?,
        fees: optional(&columns.fees)?,
        net_amount: optional(&columns.net_amount)?,
        currency: optional(&columns.currency)?,
    })
}

/// How the figures and dates of a file are read.
#[derive(Debug, Clone, Copy)]
pub struct RowFormat {
    /// The mark before a figure's decimals (IMP-013).
    pub decimal_mark: DecimalMark,
    /// The date format (IMP-014).
    pub date_format: DateFormat,
}

/// IMP-030 — every line of `file` read through `columns` and `labels`, in the file's order,
/// trade taxes joined to their purchases (IMP-034).
pub fn read_rows(
    file: &ImportFile,
    columns: &MappedColumns,
    labels: &[OperationLabel],
    format: RowFormat,
) -> Result<Vec<Row>, ImportError> {
    let located = locate(columns, file)?;
    let mut rows: Vec<Row> = file
        .lines
        .iter()
        .map(|line| Row {
            line: u32::try_from(line.number).unwrap_or(u32::MAX),
            read: read_line(line, file.header.len(), &located, labels, format)
                .unwrap_or_else(RowRead::Rejected),
        })
        .collect();
    join_trade_taxes(&mut rows);
    Ok(rows)
}

/// One line read, or the reason it is rejected.
fn read_line(
    line: &FileLine,
    header_length: usize,
    located: &Located,
    labels: &[OperationLabel],
    format: RowFormat,
) -> Result<RowRead, RowRejection> {
    if line.fields.len() < header_length {
        return Err(RowRejection::LineTooShort);
    }
    let field = |position: usize| line.fields.get(position).map_or("", String::as_str);
    let label = field(located.operation);
    let operation_type = labels
        .iter()
        .find(|known| known.label.trim().to_lowercase() == label.to_lowercase())
        .map(|known| known.transaction_type)
        .ok_or_else(|| RowRejection::OperationTypeMissing {
            label: label.to_string(),
        })?;
    let figure =
        |position: Option<usize>, column: &Option<String>| -> Result<Option<i64>, RowRejection> {
            let Some(position) = position else {
                return Ok(None);
            };
            let text = field(position);
            if text.is_empty() {
                return Ok(None);
            }
            read_figure(text, format.decimal_mark)
                .map(Some)
                .ok_or_else(|| RowRejection::FigureUnreadable {
                    column: column.as_deref().unwrap_or_default().trim().to_string(),
                })
        };
    let columns = located.columns;
    let quantity = figure(located.quantity, &columns.quantity)?;
    let unit_price = figure(located.unit_price, &columns.unit_price)?;
    let gross = figure(located.gross_amount, &columns.gross_amount)?;
    let fees = figure(located.fees, &columns.fees)?;
    let net = figure(located.net_amount, &columns.net_amount)?;
    let missing = |field: &str| RowRejection::FigureMissing {
        field: field.to_string(),
    };
    let trade = || -> Result<(i64, i64, i64), RowRejection> {
        let quantity = quantity.ok_or_else(|| missing("quantity"))?;
        if quantity == 0 {
            return Err(RowRejection::QuantityZero);
        }
        let unit_price = match (unit_price, gross) {
            (Some(price), _) => price,
            (None, Some(gross)) => i64::try_from(i128::from(gross) * MICRO / i128::from(quantity))
                .map_err(|_| missing("unit_price"))?,
            (None, None) => return Err(missing("unit_price")),
        };
        Ok((quantity, unit_price, fees.unwrap_or(0)))
    };
    let amount = || net.or(gross).ok_or_else(|| missing("net_amount"));
    let figures = match operation_type {
        OperationType::Purchase => {
            let (quantity, unit_price, fees) = trade()?;
            RowFigures::Purchase {
                quantity,
                unit_price,
                fees,
            }
        }
        OperationType::Sale => {
            let (quantity, unit_price, fees) = trade()?;
            RowFigures::Sale {
                quantity,
                unit_price,
                fees,
            }
        }
        OperationType::Dividend => RowFigures::Dividend { amount: amount()? },
        OperationType::Deposit => RowFigures::Deposit { amount: amount()? },
        OperationType::Withdrawal => RowFigures::Withdrawal { amount: amount()? },
        OperationType::TradeTax => RowFigures::TradeTax {
            amount: amount()?,
            quantity,
        },
        OperationType::NotImported => {
            return Ok(RowRead::NotImported {
                label: label.to_string(),
            })
        }
    };
    let date = read_date(field(located.date), format.date_format)
        .ok_or_else(|| RowRejection::DateUnreadable {
            column: located.columns.date.trim().to_string(),
        })?
        .format("%Y-%m-%d")
        .to_string();
    let text = |position: Option<usize>| {
        position
            .map(field)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let asset = match operation_type {
        OperationType::Deposit | OperationType::Withdrawal => AssetRead::default(),
        _ => AssetRead {
            isin: text(located.isin),
            reference: text(located.reference),
            name: text(located.name),
        },
    };
    Ok(RowRead::Operation(OperationRead {
        date,
        asset,
        currency: text(located.currency),
        figures,
    }))
}

/// IMP-013A — digits with no separator, or one to three digits then groups of three, each
/// after a separator of `thousands`.
fn whole_digits(whole: &str, thousands: &[char]) -> Option<String> {
    let groups: Vec<&str> = whole.split(thousands).collect();
    let (first, rest) = groups.split_first()?;
    let digits = |group: &str| group.chars().all(|c| c.is_ascii_digit());
    let grouped = rest.is_empty()
        || (1..=3).contains(&first.len()) && rest.iter().all(|group| group.len() == 3);
    (grouped && groups.iter().all(|group| digits(group))).then(|| groups.concat())
}

/// IMP-013A — a figure in micro-units, its sign ignored, a seventh decimal rounded down;
/// `None` when it does not read.
pub fn read_figure(text: &str, decimal_mark: DecimalMark) -> Option<i64> {
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (decimal, thousands): (char, &[char]) = match decimal_mark {
        DecimalMark::Dot => ('.', &[' ', '\u{a0}', '\u{202f}']),
        DecimalMark::Comma => (',', &[' ', '\u{a0}', '\u{202f}', '.']),
    };
    let (whole, decimals) = unsigned.split_once(decimal).unwrap_or((unsigned, ""));
    let whole = whole_digits(whole, thousands)?;
    let whole = whole.as_str();
    if whole.is_empty() && decimals.is_empty() || !decimals.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let micro: String = decimals.chars().chain("000000".chars()).take(6).collect();
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    whole
        .checked_mul(1_000_000)?
        .checked_add(micro.parse().ok()?)
}

/// IMP-034 — each trade tax added to the fees of the first purchase of its asset, date and
/// quantity not yet holding one; one with no such purchase rejected.
fn join_trade_taxes(rows: &mut [Row]) {
    let mut taken: Vec<usize> = Vec::new();
    for tax in 0..rows.len() {
        let Some(Row {
            read:
                RowRead::Operation(OperationRead {
                    date,
                    asset,
                    figures: RowFigures::TradeTax { amount, quantity },
                    ..
                }),
            ..
        }) = rows.get(tax)
        else {
            continue;
        };
        let (date, asset, amount, quantity) = (date.clone(), asset.clone(), *amount, *quantity);
        let purchase = rows.iter().enumerate().position(|(index, row)| {
            !taken.contains(&index)
                && matches!(
                    &row.read,
                    RowRead::Operation(OperationRead {
                        date: purchase_date,
                        asset: purchase_asset,
                        figures: RowFigures::Purchase { quantity: bought, .. },
                        ..
                    }) if *purchase_date == date
                        && *purchase_asset == asset
                        && quantity.is_none_or(|quantity| quantity == *bought)
                )
        });
        let joined = purchase.and_then(|index| {
            taken.push(index);
            let row = rows.get_mut(index)?;
            if let RowRead::Operation(OperationRead {
                figures: RowFigures::Purchase { fees, .. },
                ..
            }) = &mut row.read
            {
                *fees = fees.saturating_add(amount);
            }
            Some(RowRead::JoinedTradeTax {
                purchase_line: row.line,
            })
        });
        if let Some(row) = rows.get_mut(tax) {
            row.read = joined.unwrap_or(RowRead::Rejected(RowRejection::TradeTaxUnpaired));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::import::domain::file::{read_file, ReadingChoices};

    const DOT_DAY_FIRST: RowFormat = RowFormat {
        decimal_mark: DecimalMark::Dot,
        date_format: DateFormat::DayMonthYear,
    };

    fn columns() -> MappedColumns {
        MappedColumns {
            date: "Date".into(),
            operation: "Opération".into(),
            isin: None,
            reference: None,
            name: Some("libellé".into()),
            quantity: Some("Qté".into()),
            unit_price: Some("Prix".into()),
            gross_amount: Some("Brut".into()),
            fees: Some("Courtage".into()),
            net_amount: Some("Net".into()),
            currency: Some("Devise".into()),
        }
    }

    fn labels() -> Vec<OperationLabel> {
        [
            ("Achat Comptant", OperationType::Purchase),
            ("Vente Comptant", OperationType::Sale),
            ("Encaissement coupons", OperationType::Dividend),
            ("Virement", OperationType::Deposit),
            ("Retrait", OperationType::Withdrawal),
            ("TAXE TRANSAC FINAN", OperationType::TradeTax),
            ("OST détachement", OperationType::NotImported),
        ]
        .into_iter()
        .map(|(label, transaction_type)| OperationLabel {
            label: label.into(),
            transaction_type,
        })
        .collect()
    }

    const HEADER: &str = "libellé;Opération;Date;Qté;Prix;Brut;Courtage;Net;Devise;\n";

    fn rows(lines: &str) -> Vec<RowRead> {
        let text = format!("{HEADER}{lines}");
        // The mapping's separator, as a saved one gives it (IMP-020): a short line would
        // otherwise defeat detection (IMP-012).
        let choices = ReadingChoices {
            separator: Some(crate::context::import::domain::Separator::Semicolon),
            ..ReadingChoices::default()
        };
        let (file, _) = read_file(text.as_bytes(), &choices).expect("read");
        read_rows(&file, &columns(), &labels(), DOT_DAY_FIRST)
            .expect("rows")
            .into_iter()
            .map(|row| row.read)
            .collect()
    }

    fn operation(read: &RowRead) -> &OperationRead {
        match read {
            RowRead::Operation(operation) => operation,
            other => panic!("not an operation: {other:?}"),
        }
    }

    // IMP-013A — sign ignored, spaces between thousands, dots too under the comma, a seventh
    // decimal rounded down; a comma under the dot does not read.
    #[test]
    fn imp_013a_how_a_figure_is_read() {
        assert_eq!(
            read_figure("-1 234.5", DecimalMark::Dot),
            Some(1_234_500_000)
        );
        assert_eq!(
            read_figure("1.234,56", DecimalMark::Comma),
            Some(1_234_560_000)
        );
        assert_eq!(
            read_figure("1\u{202f}000,1234567", DecimalMark::Comma),
            Some(1_000_123_456)
        );
        assert_eq!(read_figure(".5", DecimalMark::Dot), Some(500_000));
        assert_eq!(read_figure("1,234.56", DecimalMark::Dot), None);
        assert_eq!(read_figure("12 EUR", DecimalMark::Dot), None);
        assert_eq!(read_figure("-", DecimalMark::Dot), None);
        assert_eq!(read_figure("12 34", DecimalMark::Dot), None);
        assert_eq!(read_figure("12.34", DecimalMark::Comma), None);
        assert_eq!(read_figure("1 2345,6", DecimalMark::Comma), None);
        assert_eq!(read_figure("1234 567", DecimalMark::Dot), None);
        assert_eq!(read_figure("1,23 4", DecimalMark::Comma), None);
        assert_eq!(read_figure(" 123", DecimalMark::Dot), None);
        assert_eq!(read_figure("1  234", DecimalMark::Dot), None);
        assert_eq!(
            read_figure("1.234 567,8", DecimalMark::Comma),
            Some(1_234_567_800_000)
        );
        assert_eq!(
            read_figure("1\u{a0}234", DecimalMark::Dot),
            Some(1_234_000_000)
        );
        assert_eq!(read_figure("12,", DecimalMark::Comma), Some(12_000_000));
        assert_eq!(
            read_figure("1234,5", DecimalMark::Comma),
            Some(1_234_500_000)
        );
        assert_eq!(
            read_figure("1 234 567", DecimalMark::Dot),
            Some(1_234_567_000_000)
        );
    }

    // IMP-021 — a mapped column the file lacks is refused when used on that file; headers
    // are compared trimmed and case ignored.
    #[test]
    fn imp_021_a_mapped_column_the_file_lacks_is_refused() {
        let (file, _) = read_file(
            b"DATE ;Operation\n01/02/2024;x\n",
            &ReadingChoices::default(),
        )
        .expect("read");
        let mut mapped = columns();
        mapped.operation = "operation".into();
        let refused = read_rows(&file, &mapped, &labels(), DOT_DAY_FIRST);
        assert!(matches!(
            refused,
            Err(ImportError::MappedColumnNotInFile { column }) if column == "libellé"
        ));
    }

    // IMP-022 — a label without a type is rejected with it; a not-imported label is so.
    #[test]
    fn imp_022_each_operation_label_has_its_type() {
        let read = rows(
            "X;Souscription;03/01/2024;1;1;1;0;1;EUR;\nX; ost DÉTACHEMENT ;03/01/2024;;;;;;EUR;\n",
        );
        assert_eq!(
            read[0],
            RowRead::Rejected(RowRejection::OperationTypeMissing {
                label: "Souscription".into()
            })
        );
        assert_eq!(
            read[1],
            RowRead::NotImported {
                label: "ost DÉTACHEMENT".into()
            }
        );
    }

    // IMP-016 / IMP-030 — a short line, an unreadable date and an unreadable figure are
    // rejected naming what failed.
    #[test]
    fn imp_030_a_date_or_figure_that_does_not_read_is_a_rejection() {
        let read = rows("X;Achat Comptant;03/01/2024\nX;Achat Comptant;2024-01-03;1;1;1;0;1;EUR;\nX;Achat Comptant;03/01/2024;1;1,5;1;0;1;EUR;\n");
        assert_eq!(read[0], RowRead::Rejected(RowRejection::LineTooShort));
        assert_eq!(
            read[1],
            RowRead::Rejected(RowRejection::DateUnreadable {
                column: "Date".into()
            })
        );
        assert_eq!(
            read[2],
            RowRead::Rejected(RowRejection::FigureUnreadable {
                column: "Prix".into()
            })
        );
    }

    // IMP-031 — signs ignored; the unit price from the gross amount when absent, rounded
    // down; zero quantity refused first; fees zero when empty; net amount, else gross.
    #[test]
    fn imp_031_the_figures_of_each_type() {
        let read = rows(concat!(
            "ACME;Achat Comptant;03/01/2024;3;;-100;;-101;EUR;\n",
            "ACME;Vente Comptant;04/01/2024;-2;12.5;;1.5;;EUR;\n",
            "ACME;Achat Comptant;05/01/2024;0;;-100;;;EUR;\n",
            "ACME;Achat Comptant;05/01/2024;;10;;;;EUR;\n",
            "ACME;Encaissement coupons;06/01/2024;;;8;;6.4;EUR;\n",
            "ACME;Encaissement coupons;06/01/2024;;;8;;;EUR;\n",
            ";Virement;07/01/2024;;;500;;;;\n",
            ";Retrait;07/01/2024;;;;;;;\n",
        ));
        assert_eq!(
            operation(&read[0]).figures,
            RowFigures::Purchase {
                quantity: 3_000_000,
                unit_price: 33_333_333,
                fees: 0
            }
        );
        assert_eq!(
            operation(&read[1]).figures,
            RowFigures::Sale {
                quantity: 2_000_000,
                unit_price: 12_500_000,
                fees: 1_500_000
            }
        );
        assert_eq!(read[2], RowRead::Rejected(RowRejection::QuantityZero));
        assert_eq!(
            read[3],
            RowRead::Rejected(RowRejection::FigureMissing {
                field: "quantity".into()
            })
        );
        assert_eq!(
            operation(&read[4]).figures,
            RowFigures::Dividend { amount: 6_400_000 }
        );
        assert_eq!(
            operation(&read[5]).figures,
            RowFigures::Dividend { amount: 8_000_000 }
        );
        let deposit = operation(&read[6]);
        assert_eq!(
            deposit.figures,
            RowFigures::Deposit {
                amount: 500_000_000
            }
        );
        assert_eq!(
            (&deposit.asset, &deposit.currency),
            (&AssetRead::default(), &None)
        );
        assert_eq!(
            read[7],
            RowRead::Rejected(RowRejection::FigureMissing {
                field: "net_amount".into()
            })
        );
        assert_eq!(operation(&read[0]).date, "2024-01-03");
        assert_eq!(operation(&read[0]).asset.name.as_deref(), Some("ACME"));
        assert_eq!(operation(&read[0]).currency.as_deref(), Some("EUR"));
    }

    // IMP-034 — a trade tax joins the first purchase of its asset, date and quantity not yet
    // holding one; a second one the next; one with none is rejected.
    #[test]
    fn imp_034_a_trade_tax_joins_its_purchase() {
        let read = rows(concat!(
            "ACME;Achat Comptant;03/01/2024;10;12;;1;;EUR;\n",
            "ACME;Achat Comptant;03/01/2024;10;12;;2;;EUR;\n",
            "ACME;TAXE TRANSAC FINAN;03/01/2024;10;;;;0.36;EUR;\n",
            "ACME;TAXE TRANSAC FINAN;03/01/2024;;;0.4;;;EUR;\n",
            "ACME;TAXE TRANSAC FINAN;03/01/2024;10;;;;0.36;EUR;\n",
            "OTHER;TAXE TRANSAC FINAN;03/01/2024;10;;;;0.36;EUR;\n",
        ));
        let fees = |read: &RowRead| match operation(read).figures {
            RowFigures::Purchase { fees, .. } => fees,
            _ => panic!("not a purchase"),
        };
        assert_eq!((fees(&read[0]), fees(&read[1])), (1_360_000, 2_400_000));
        assert_eq!(read[2], RowRead::JoinedTradeTax { purchase_line: 2 });
        assert_eq!(read[3], RowRead::JoinedTradeTax { purchase_line: 3 });
        assert_eq!(read[4], RowRead::Rejected(RowRejection::TradeTaxUnpaired));
        assert_eq!(read[5], RowRead::Rejected(RowRejection::TradeTaxUnpaired));
    }
}
