use crate::context::account_import::domain::{DateFormat, DecimalMark, Encoding, Separator};
use crate::context::account_import::error::AccountImportError;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;

/// IMP-010 — the largest file an import reads, in bytes.
pub const LARGEST_FILE: u64 = 10 * 1024 * 1024;

/// IMP-012 — how many lines after the header the separator is detected on.
const DETECTION_LINES: usize = 20;

/// IMP-018 — what the user picked instead of the detected format, and the date column the
/// date format is detected on (IMP-014). Each left out is detected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct ReadingChoices {
    /// The encoding picked.
    pub encoding: Option<Encoding>,
    /// The separator picked.
    pub separator: Option<Separator>,
    /// The decimal mark picked.
    pub decimal_mark: Option<DecimalMark>,
    /// The date format picked.
    pub date_format: Option<DateFormat>,
    /// The header of the date column, once mapped (IMP-021).
    pub date_column: Option<String>,
}

/// The date format a file is read with, and whether it was detected (IMP-014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DateReading {
    /// The format.
    pub format: DateFormat,
    /// False when no format read every date and day/month/year was taken.
    pub detected: bool,
}

/// IMP-010 to IMP-016 — how a file was read: its format, detected or chosen, and its columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileRead {
    /// The encoding it was read with.
    pub encoding: Encoding,
    /// The separator between its fields.
    pub separator: Separator,
    /// The mark before a figure's decimals.
    pub decimal_mark: DecimalMark,
    /// The date format, once the date column is known; `None` before.
    pub date_reading: Option<DateReading>,
    /// The columns, named by the header, in the file's order; an empty header field names
    /// no column and is left out.
    pub columns: Vec<String>,
    /// How many lines after the header are not blank.
    pub rows: u32,
}

/// One line of the file after its header that is not blank (IMP-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLine {
    /// Its line number in the file, from 1 for the header.
    pub number: usize,
    /// Its fields, trimmed, in the file's order.
    pub fields: Vec<String>,
}

/// A file read into its header and its lines (IMP-015, IMP-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFile {
    /// The header's fields, trimmed, empty ones included, in the file's order.
    pub header: Vec<String>,
    /// The lines after the header that are not blank.
    pub lines: Vec<FileLine>,
}

impl ImportFile {
    /// IMP-021 — the position of the column named `name`, compared trimmed and case ignored.
    pub fn column(&self, name: &str) -> Option<usize> {
        let name = name.trim().to_lowercase();
        self.header
            .iter()
            .position(|header| !header.is_empty() && header.to_lowercase() == name)
    }
}

/// IMP-010 to IMP-016 — reads `bytes` with what the user chose, the rest detected.
pub fn read_file(
    bytes: &[u8],
    choices: &ReadingChoices,
) -> Result<(ImportFile, FileRead), AccountImportError> {
    if bytes.len() as u64 > LARGEST_FILE {
        return Err(AccountImportError::FileTooLarge);
    }
    let (text, encoding) = decode(bytes, choices.encoding)?;
    let raw: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let Some((header_line, body)) = raw
        .split_first()
        .filter(|(header, _)| !header.trim().is_empty())
    else {
        return Err(AccountImportError::FileEmpty);
    };
    let separator = match choices.separator {
        Some(separator) => separator,
        None => detect_separator(header_line, body).ok_or(AccountImportError::SeparatorNotFound)?,
    };
    let header = split_header(header_line, separator);
    let lines: Vec<FileLine> = body
        .iter()
        .enumerate()
        .map(|(index, line)| FileLine {
            number: index + 2,
            fields: split_line(line, separator),
        })
        .filter(|line| line.fields.iter().any(|field| !field.is_empty()))
        .collect();
    if lines.is_empty() {
        return Err(AccountImportError::FileHasNoRow);
    }
    let mut named: Vec<String> = Vec::with_capacity(header.len());
    for name in header.iter().filter(|name| !name.is_empty()) {
        if named
            .iter()
            .any(|seen| seen.to_lowercase() == name.to_lowercase())
        {
            return Err(AccountImportError::HeaderNameRepeated { name: name.clone() });
        }
        named.push(name.clone());
    }
    let file = ImportFile { header, lines };
    let decimal_mark = choices
        .decimal_mark
        .unwrap_or_else(|| detect_decimal_mark(&file, separator));
    let date_reading = match (choices.date_format, &choices.date_column) {
        (Some(format), _) => Some(DateReading {
            format,
            detected: true,
        }),
        (None, Some(column)) => file.column(column).map(|position| {
            let dates = file
                .lines
                .iter()
                .filter_map(|line| line.fields.get(position))
                .filter(|field| !field.is_empty());
            detect_date_format(dates)
        }),
        (None, None) => None,
    };
    let read = FileRead {
        encoding,
        separator,
        decimal_mark,
        date_reading,
        columns: named,
        rows: u32::try_from(file.lines.len()).unwrap_or(u32::MAX),
    };
    Ok((file, read))
}

/// IMP-011 — the file's text: UTF-8 when its bytes are, its byte order mark dropped,
/// otherwise Windows-1252; a chosen UTF-8 the bytes are not is refused.
pub fn decode(
    bytes: &[u8],
    chosen: Option<Encoding>,
) -> Result<(String, Encoding), AccountImportError> {
    match (chosen, std::str::from_utf8(bytes)) {
        (Some(Encoding::Utf8) | None, Ok(text)) => Ok((
            text.strip_prefix('\u{feff}').unwrap_or(text).to_string(),
            Encoding::Utf8,
        )),
        (Some(Encoding::Utf8), Err(_)) => Err(AccountImportError::FileNotUtf8),
        (Some(Encoding::Windows1252) | None, _) => Ok((
            bytes.iter().map(|byte| windows_1252(*byte)).collect(),
            Encoding::Windows1252,
        )),
    }
}

/// The character a Windows-1252 byte stands for; the five bytes it leaves undefined stand
/// for the control character of the same value, as browsers read them.
fn windows_1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match byte {
        0x80..=0x9f => HIGH
            .get(usize::from(byte - 0x80))
            .copied()
            .unwrap_or(char::from(byte)),
        _ => char::from(byte),
    }
}

/// IMP-012 — the first of `;`, tab and `,` that splits the header into more than one field
/// and every line of the first 20 that are not blank into at least as many.
fn detect_separator(header: &str, lines: &[&str]) -> Option<Separator> {
    [Separator::Semicolon, Separator::Tab, Separator::Comma]
        .into_iter()
        .find(|separator| {
            let columns = split_header(header, *separator).len();
            columns > 1
                && lines
                    .iter()
                    .map(|line| split_line(line, *separator))
                    .filter(|fields| fields.iter().any(|field| !field.is_empty()))
                    .take(DETECTION_LINES)
                    .all(|fields| fields.len() >= columns)
        })
}

/// IMP-012A — the fields of `line`: a quoted field holds the separator as text and `""`
/// for `"`; each is trimmed.
pub fn split_line(line: &str, separator: Separator) -> Vec<String> {
    let separator = match separator {
        Separator::Semicolon => ';',
        Separator::Comma => ',',
        Separator::Tab => '\t',
    };
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                field.push('"');
                characters.next();
            }
            '"' if quoted => quoted = false,
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            _ if character == separator && !quoted => {
                fields.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(character),
        }
    }
    fields.push(field.trim().to_string());
    fields
}

/// IMP-012A — the header's fields, the empty ones at its end, a trailing separator's among
/// them, adding no column.
fn split_header(line: &str, separator: Separator) -> Vec<String> {
    let mut fields = split_line(line, separator);
    while fields.last().is_some_and(String::is_empty) {
        fields.pop();
    }
    fields
}

/// IMP-013 — the comma when the separator is not one and a field after the header is a
/// figure written with a decimal comma; the dot otherwise.
fn detect_decimal_mark(file: &ImportFile, separator: Separator) -> DecimalMark {
    if separator == Separator::Comma {
        return DecimalMark::Dot;
    }
    let comma_figure = file
        .lines
        .iter()
        .flat_map(|line| line.fields.iter())
        .any(|field| written_with_a_comma(field));
    if comma_figure {
        DecimalMark::Comma
    } else {
        DecimalMark::Dot
    }
}

/// A sign or none, digits — their thousands separated by dots or spaces, or not separated
/// —, a comma and one or more digits (IMP-013).
fn written_with_a_comma(field: &str) -> bool {
    let unsigned = field.strip_prefix(['-', '+']).unwrap_or(field);
    let Some((whole, decimals)) = unsigned.split_once(',') else {
        return false;
    };
    if decimals.is_empty() || !decimals.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let groups: Vec<&str> = whole.split(['.', ' ', '\u{a0}', '\u{202f}']).collect();
    match groups.as_slice() {
        [single] => !single.is_empty() && single.chars().all(|c| c.is_ascii_digit()),
        [first, rest @ ..] => {
            (1..=3).contains(&first.len())
                && first.chars().all(|c| c.is_ascii_digit())
                && rest
                    .iter()
                    .all(|group| group.len() == 3 && group.chars().all(|c| c.is_ascii_digit()))
        }
        [] => false,
    }
}

/// IMP-014 — the first of day/month/year, year-month-day and month/day/year that reads every
/// date; day/month/year, not detected, when none does or the column holds no date.
fn detect_date_format<'a>(dates: impl Iterator<Item = &'a String> + Clone) -> DateReading {
    if dates.clone().next().is_none() {
        return DateReading {
            format: DateFormat::DayMonthYear,
            detected: false,
        };
    }
    [
        DateFormat::DayMonthYear,
        DateFormat::YearMonthDay,
        DateFormat::MonthDayYear,
    ]
    .into_iter()
    .find(|format| dates.clone().all(|date| read_date(date, *format).is_some()))
    .map_or(
        DateReading {
            format: DateFormat::DayMonthYear,
            detected: false,
        },
        |format| DateReading {
            format,
            detected: true,
        },
    )
}

/// IMP-014 — the day `text` stands for in `format`: parts separated by `/`, `-` or `.`, a
/// four-digit year, a time after the date ignored; `None` when it is no real day.
pub fn read_date(text: &str, format: DateFormat) -> Option<NaiveDate> {
    let date = text.split_whitespace().next()?;
    let date = date.split('T').next()?;
    let parts: Vec<&str> = date.split(['/', '-', '.']).collect();
    let [a, b, c] = parts.as_slice() else {
        return None;
    };
    let (year, month, day) = match format {
        DateFormat::DayMonthYear => (c, b, a),
        DateFormat::MonthDayYear => (c, a, b),
        DateFormat::YearMonthDay => (a, b, c),
    };
    if year.len() != 4
        || [year, month, day]
            .iter()
            .any(|part| !part.chars().all(|c| c.is_ascii_digit()) || part.is_empty())
    {
        return None;
    }
    NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<(ImportFile, FileRead), AccountImportError> {
        read_file(text.as_bytes(), &ReadingChoices::default())
    }

    // IMP-011 — UTF-8 bytes are read as UTF-8, the byte order mark dropped; others as
    // Windows-1252; a chosen UTF-8 the bytes are not is refused.
    #[test]
    fn imp_011_the_encoding_is_detected() {
        let (text, encoding) = decode("\u{feff}Libellé".as_bytes(), None).expect("read");
        assert_eq!((text.as_str(), encoding), ("Libellé", Encoding::Utf8));
        let latin = [b'L', b'i', b'b', b'e', b'l', b'l', 0xe9, b' ', 0x80];
        let (text, encoding) = decode(&latin, None).expect("read");
        assert_eq!(
            (text.as_str(), encoding),
            ("Libellé €", Encoding::Windows1252)
        );
        assert!(matches!(
            decode(&latin, Some(Encoding::Utf8)),
            Err(AccountImportError::FileNotUtf8)
        ));
        let (text, _) = decode("é".as_bytes(), Some(Encoding::Windows1252)).expect("read");
        assert_eq!(text, "Ã©");
    }

    // IMP-012 — the first separator splitting the header and the lines alike, `;` before
    // tab before `,`; none is refused, unless one is chosen (IMP-018).
    #[test]
    fn imp_012_the_separator_is_detected() {
        let (_, semicolon) = read("Date;Opération;Montant\n01/02/2024;Achat;1,5\n").expect("read");
        assert_eq!(semicolon.separator, Separator::Semicolon);
        let (_, comma) = read("Date,Operation,Amount\n2024-02-01,Buy,1.5\n").expect("read");
        assert_eq!(comma.separator, Separator::Comma);
        let (_, tab) = read("Date\tOperation\n2024-02-01\tBuy, more\n").expect("read");
        assert_eq!(tab.separator, Separator::Tab);
        assert!(matches!(
            read("Date Operation\n2024-02-01 Buy\n"),
            Err(AccountImportError::SeparatorNotFound)
        ));
        // A short line among the first 20 defeats a separator; the user can still pick it.
        assert!(matches!(
            read("A;B;C\n1;2;3\n4\n"),
            Err(AccountImportError::SeparatorNotFound)
        ));
        let chosen = ReadingChoices {
            separator: Some(Separator::Semicolon),
            ..ReadingChoices::default()
        };
        let (file, _) = read_file(b"A;B;C\n1;2;3\n4\n", &chosen).expect("read");
        assert_eq!(file.lines[1].fields, ["4"]);
    }

    // IMP-012A — quotes hold the separator and `""`; fields are trimmed; a trailing
    // separator adds no column.
    #[test]
    fn imp_012a_how_a_field_is_read() {
        assert_eq!(
            split_line(r#" a ; "b;c" ;"say ""hi""";"#, Separator::Semicolon),
            ["a", "b;c", r#"say "hi""#, ""]
        );
        assert_eq!(split_header("a;b;;", Separator::Semicolon), ["a", "b"]);
        // A line's empty last field is a field: it is not short of the header's.
        let (file, _) = read("A,B,C\n1,2,\n").expect("read");
        assert_eq!(file.lines[0].fields, ["1", "2", ""]);
        assert_eq!(split_line("a;;b", Separator::Semicolon), ["a", "", "b"]);
    }

    // IMP-013 — the comma when a field is a figure written with one, thousands dotted or
    // spaced; the dot under a comma separator and otherwise.
    #[test]
    fn imp_013_the_decimal_mark_is_detected() {
        for (figure, mark) in [
            ("1.234,56", DecimalMark::Comma),
            ("-12,5", DecimalMark::Comma),
            ("1\u{a0}234,5", DecimalMark::Comma),
            ("12.5", DecimalMark::Dot),
            ("1,2,3", DecimalMark::Dot),
            ("12.34,5", DecimalMark::Dot),
        ] {
            let (_, read) = read(&format!("A;B\nx;{figure}\n")).expect("read");
            assert_eq!(read.decimal_mark, mark, "{figure}");
        }
        let (_, comma) = read("A,B\nx,\"1,5\"\n").expect("read");
        assert_eq!(comma.decimal_mark, DecimalMark::Dot);
    }

    // IMP-014 — the first format reading every date that is not empty; day/month/year, not
    // detected, when none does; a time is ignored.
    #[test]
    fn imp_014_the_date_format_is_detected_on_the_date_column() {
        let detect = |text: &str| {
            let choices = ReadingChoices {
                date_column: Some(" date ".to_string()),
                ..ReadingChoices::default()
            };
            read_file(text.as_bytes(), &choices)
                .expect("read")
                .1
                .date_reading
        };
        let detected = |format| {
            Some(DateReading {
                format,
                detected: true,
            })
        };
        assert_eq!(
            detect("Date;X\n13/02/2024;a\n;b\n"),
            detected(DateFormat::DayMonthYear)
        );
        assert_eq!(
            detect("Date;X\n2024-02-13 10:00;a\n"),
            detected(DateFormat::YearMonthDay)
        );
        assert_eq!(
            detect("Date;X\n02.13.2024;a\n"),
            detected(DateFormat::MonthDayYear)
        );
        assert_eq!(
            detect("Date;X\n13/02/2024;a\n02/13/2024;b\n"),
            Some(DateReading {
                format: DateFormat::DayMonthYear,
                detected: false
            })
        );
        assert_eq!(
            read("Date;X\n13/02/2024;a\n").expect("read").1.date_reading,
            None
        );
        assert_eq!(
            detect("Date;X\n;a\n;b\n"),
            Some(DateReading {
                format: DateFormat::DayMonthYear,
                detected: false
            }),
            "a column with no date is not detected"
        );
        assert_eq!(read_date("13/02/24", DateFormat::DayMonthYear), None);
        assert_eq!(read_date("31/02/2024", DateFormat::DayMonthYear), None);
    }

    // IMP-015 — a header name held twice, case ignored, is refused; empty header fields
    // name no column.
    #[test]
    fn imp_015_the_header_names_the_columns() {
        assert!(matches!(
            read("Date;date\n1;2\n"),
            Err(AccountImportError::HeaderNameRepeated { name }) if name == "date"
        ));
        let (file, found) = read("Date;;X;;\n1;2;3;4\n").expect("read");
        assert_eq!(found.columns, ["Date", "X"]);
        assert_eq!(file.column("x"), Some(2));
        assert_eq!(file.column(""), None);
    }

    // IMP-010 / IMP-016 — blank lines, separators only included, are skipped and are no
    // row; an empty file and a header alone are refused; line numbers count every line.
    #[test]
    fn imp_016_blank_lines_are_skipped() {
        let (file, found) = read("A;B\r\n\r\n;;\r\n1;2\r\n").expect("read");
        assert_eq!(found.rows, 1);
        assert_eq!(file.lines[0].number, 4);
        assert!(matches!(read(""), Err(AccountImportError::FileEmpty)));
        assert!(matches!(
            read("A;B\n;\n\n"),
            Err(AccountImportError::FileHasNoRow)
        ));
        let large = vec![b'a'; (LARGEST_FILE + 1) as usize];
        assert!(matches!(
            read_file(&large, &ReadingChoices::default()),
            Err(AccountImportError::FileTooLarge)
        ));
    }

    // A Fortuneo-shaped export: ISO-8859-1, `;` with a trailing `;`, decimal dot, dd/mm/yyyy.
    #[test]
    fn a_french_broker_export_is_read() {
        let mut bytes = "libellé;Opération;Place;Date;Qté;Prix d'éxé;Montant brut;Courtage/Prélèvement;Montant net;Devise;\r\nACME WORLD;Achat Comptant;PARIS;03/01/2024;10;12.5;-125.0;1.95;-126.95;EUR;\r\n"
            .chars()
            .map(|c| u8::try_from(u32::from(c)).expect("latin-1"))
            .collect::<Vec<u8>>();
        bytes.extend_from_slice(b"\r\n");
        let choices = ReadingChoices {
            date_column: Some("Date".to_string()),
            ..ReadingChoices::default()
        };
        let (file, read) = read_file(&bytes, &choices).expect("read");
        assert_eq!(read.encoding, Encoding::Windows1252);
        assert_eq!(read.separator, Separator::Semicolon);
        assert_eq!(read.decimal_mark, DecimalMark::Dot);
        assert_eq!(
            read.date_reading.map(|reading| reading.format),
            Some(DateFormat::DayMonthYear)
        );
        assert_eq!(read.columns.len(), 10);
        assert_eq!(file.lines[0].fields[1], "Achat Comptant");
    }
}
