import type {
  AccountImportError,
  CsvImportError,
  DateFormat,
  DecimalMark,
  Encoding,
  FileRead,
  ImportMapper,
  ImportMapperDraft,
  MappedColumns,
  OperationType,
  Separator,
} from "@/bindings";
import type { I18nMessage } from "@/ui/format/i18n";

/** What the user picked instead of the detected format (IMP-018). */
export type FormatChoices = {
  encoding: Encoding | null;
  separator: Separator | null;
  decimal_mark: DecimalMark | null;
  date_format: DateFormat | null;
};

/** A value of one of the format's fields. */
export type FormatValue = Encoding | Separator | DecimalMark | DateFormat;

export const NO_CHOICES: FormatChoices = {
  encoding: null,
  separator: null,
  decimal_mark: null,
  date_format: null,
};

export const ENCODING_LABELS: Record<Encoding, string> = {
  Utf8: "account_import.encoding_utf8",
  Windows1252: "account_import.encoding_windows1252",
};
export const SEPARATOR_LABELS: Record<Separator, string> = {
  Semicolon: "account_import.separator_semicolon",
  Comma: "account_import.separator_comma",
  Tab: "account_import.separator_tab",
};
export const DECIMAL_MARK_LABELS: Record<DecimalMark, string> = {
  Dot: "account_import.decimal_dot",
  Comma: "account_import.decimal_comma",
};
export const DATE_FORMAT_LABELS: Record<DateFormat, string> = {
  DayMonthYear: "account_import.date_day_month_year",
  MonthDayYear: "account_import.date_month_day_year",
  YearMonthDay: "account_import.date_year_month_day",
};
export const OPERATION_TYPE_LABELS: Record<OperationType, string> = {
  Purchase: "account_import.type_purchase",
  Sale: "account_import.type_sale",
  Dividend: "account_import.type_dividend",
  Deposit: "account_import.type_deposit",
  Withdrawal: "account_import.type_withdrawal",
  TradeTax: "account_import.type_trade_tax",
  NotImported: "account_import.type_not_imported",
};

/** The fields a mapper reads from a column (IMP-021), in the window's order. */
export const COLUMN_FIELDS: { field: keyof MappedColumns; label: string; required: boolean }[] = [
  { field: "date", label: "account_import.column_date", required: true },
  { field: "operation", label: "account_import.column_operation", required: true },
  { field: "isin", label: "account_import.column_isin", required: false },
  { field: "reference", label: "account_import.column_reference", required: false },
  { field: "name", label: "account_import.column_name", required: false },
  { field: "quantity", label: "account_import.column_quantity", required: false },
  { field: "unit_price", label: "account_import.column_unit_price", required: false },
  { field: "gross_amount", label: "account_import.column_gross_amount", required: false },
  { field: "fees", label: "account_import.column_fees", required: false },
  { field: "net_amount", label: "account_import.column_net_amount", required: false },
  { field: "currency", label: "account_import.column_currency", required: false },
];

/** The last part of a path, on any system. */
export function fileNameOf(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** A path without its last part, on any system. */
export function folderOf(path: string): string {
  return path.slice(0, path.length - fileNameOf(path).length - 1);
}

/** A new mapper before its file is read: the fields to choose a format in (IMP-061). */
export function blankDraft(): ImportMapperDraft {
  return {
    info: { provider: "" },
    separator: "Semicolon",
    decimal_mark: "Comma",
    date_format: "DayMonthYear",
    encoding: "Utf8",
    columns: {
      date: "",
      operation: "",
      isin: null,
      reference: null,
      name: null,
      quantity: null,
      unit_price: null,
      gross_amount: null,
      fees: null,
      net_amount: null,
      currency: null,
    },
    operation_types: [],
    asset_matches: [],
  };
}

/** A saved mapper, as the mapper in use. */
export function draftOf({ id: _id, ...draft }: ImportMapper): ImportMapperDraft {
  return draft;
}

/** The draft with the format the file was read with. */
export function withFormatOf(draft: ImportMapperDraft, read: FileRead): ImportMapperDraft {
  return {
    ...draft,
    encoding: read.encoding,
    separator: read.separator,
    decimal_mark: read.decimal_mark,
    date_format: read.date_reading?.format ?? draft.date_format,
  };
}

const sameLabel = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** The column's name as the file writes it, the mapper's when the file lacks it (IMP-021). */
export function columnAsInFile(columns: string[], name: string | null): string {
  if (!name) return "";
  return columns.find((column) => sameLabel(column, name)) ?? name;
}

/** The type the draft gives a label, the first when a file gives it two (IMP-022). */
export function typeOfLabel(draft: ImportMapperDraft, label: string): OperationType | null {
  return (
    draft.operation_types.find((entry) => sameLabel(entry.label, label))?.transaction_type ?? null
  );
}

/** The draft with the label's type replaced, or removed for `null`; other labels are kept. */
export function withLabelType(
  draft: ImportMapperDraft,
  label: string,
  type: OperationType | null,
): ImportMapperDraft {
  const others = draft.operation_types.flatMap((entry) =>
    sameLabel(entry.label, label) ? [] : [entry],
  );
  return {
    ...draft,
    operation_types: type ? [...others, { label, transaction_type: type }] : others,
  };
}

/** IMP-064 — a mapper's file refused says what the file lacks: nothing is there to choose. */
export function mapperFileRefusalToI18n(error: AccountImportError): I18nMessage {
  switch (error.code) {
    case "MapperColumnMissing":
      return { key: "account_import.error.mapper_file_no_columns" };
    case "MapperProviderEmpty":
      return { key: "account_import.error.mapper_file_no_provider" };
    default:
      return accountImportErrorToI18n(error);
  }
}

/** F27 — the i18n message of a backend refusal. */
export function accountImportErrorToI18n(error: CsvImportError | AccountImportError): I18nMessage {
  switch (error.code) {
    case "MapperProviderEmpty":
      return { key: "account_import.error.provider_empty" };
    case "MapperProviderTaken":
      return { key: "account_import.error.provider_taken", vars: { provider: error.provider } };
    case "MapperColumnMissing":
      return { key: "account_import.error.column_missing" };
    case "MapperTextTooLong":
      return { key: "account_import.error.text_too_long" };
    case "MapperNotFound":
      return { key: "account_import.error.mapper_not_found" };
    case "MapperFileUnreadable":
      return { key: "account_import.error.mapper_file_unreadable" };
    case "MapperFileTooLarge":
      return { key: "account_import.error.mapper_file_too_large" };
    case "MapperFileInvalid":
      return { key: "account_import.error.mapper_file_invalid", vars: { reason: error.reason } };
    case "MapperFolderFailed":
      return { key: "account_import.error.mapper_folder_failed" };
    case "FileUnreadable":
    case "FileNotPicked":
      return { key: "account_import.error.file_unreadable" };
    case "FileTooLarge":
      return { key: "account_import.error.file_too_large" };
    case "FileEmpty":
      return { key: "account_import.error.file_empty" };
    case "FileHasNoRow":
      return { key: "account_import.error.file_has_no_row" };
    case "FileNotUtf8":
      return { key: "account_import.error.file_not_utf8" };
    case "SeparatorNotFound":
      return { key: "account_import.error.separator_not_found" };
    case "HeaderNameRepeated":
      return { key: "account_import.error.header_repeated", vars: { name: error.name } };
    case "MappedColumnsNotInFile":
      return {
        key: "account_import.error.columns_not_in_file",
        vars: { columns: error.columns.join(", ") },
      };
    case "MatchedAssetNotFound":
      return { key: "account_import.error.matched_asset_not_found" };
    default:
      return { key: "error.Unknown" };
  }
}
