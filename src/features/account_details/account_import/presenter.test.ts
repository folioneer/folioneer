import { describe, expect, it } from "vitest";
import type { FileRead, ImportMapperDraft } from "@/bindings";
import {
  accountImportErrorToI18n,
  blankDraft,
  columnAsInFile,
  draftOf,
  fileNameOf,
  folderOf,
  mapperFileRefusalToI18n,
  OPERATION_TYPE_LABELS,
  typeOfLabel,
  withFormatOf,
  withLabelType,
} from "./presenter";

const read: FileRead = {
  encoding: "Windows1252",
  separator: "Semicolon",
  decimal_mark: "Comma",
  date_reading: null,
  columns: ["Date", "Opération"],
  rows: 3,
  operation_labels: [],
};

const draft = (): ImportMapperDraft => ({
  ...withFormatOf(blankDraft(), read),
  operation_types: [
    { label: "Achat", transaction_type: "Purchase" },
    { label: "Vente", transaction_type: "Sale" },
  ],
});

describe("account import presenter", () => {
  it("names a file and its folder on any system", () => {
    expect(fileNameOf("/home/me/exports/a.csv")).toBe("a.csv");
    expect(folderOf("/home/me/exports/a.csv")).toBe("/home/me/exports");
    expect(fileNameOf("C:\\exports\\a.csv")).toBe("a.csv");
    expect(folderOf("C:\\exports\\a.csv")).toBe("C:\\exports");
  });

  it("a new mapper takes the format read, the date format once told", () => {
    expect(withFormatOf(blankDraft(), read)).toMatchObject({
      encoding: "Windows1252",
      date_format: "DayMonthYear",
    });
    expect(
      withFormatOf(draft(), {
        ...read,
        separator: "Tab",
        date_reading: { format: "YearMonthDay", detected: true },
      }),
    ).toMatchObject({ separator: "Tab", date_format: "YearMonthDay" });
  });

  // IMP-021 — a mapper's column compares trimmed and case ignored; the field shows the file's.
  it("shows a column as the file writes it", () => {
    expect(columnAsInFile(read.columns, " date ")).toBe("Date");
    expect(columnAsInFile(read.columns, null)).toBe("");
  });

  // IMP-022 — a label's type is replaced or removed, case ignored; other labels are kept.
  it("sets and removes a label's type", () => {
    expect(typeOfLabel(draft(), " ACHAT ")).toBe("Purchase");
    expect(typeOfLabel(draft(), "Coupon")).toBeNull();
    const changed = withLabelType(draft(), "ACHAT", "NotImported");
    expect(typeOfLabel(changed, "achat")).toBe("NotImported");
    expect(changed.operation_types).toHaveLength(2);
    const removed = withLabelType(draft(), "achat", null);
    expect(removed.operation_types).toEqual([{ label: "Vente", transaction_type: "Sale" }]);
  });

  // IMP-064 — a refused file is not asked to choose anything.
  it("words a mapper's file refused for what it lacks", () => {
    expect(mapperFileRefusalToI18n({ code: "MapperColumnMissing" }).key).toBe(
      "account_import.error.mapper_file_no_columns",
    );
    expect(mapperFileRefusalToI18n({ code: "MapperProviderEmpty" }).key).toBe(
      "account_import.error.mapper_file_no_provider",
    );
    expect(mapperFileRefusalToI18n({ code: "MapperFileTooLarge" }).key).toBe(
      "account_import.error.mapper_file_too_large",
    );
  });

  it("gives each refusal its message and its vars", () => {
    expect(
      accountImportErrorToI18n({ code: "MappedColumnsNotInFile", columns: ["Qté", "Prix"] }),
    ).toEqual({
      key: "account_import.error.columns_not_in_file",
      vars: { columns: "Qté, Prix" },
    });
    expect(accountImportErrorToI18n({ code: "DatabaseError" })).toEqual({
      key: "error.DatabaseError",
    });
    expect(accountImportErrorToI18n({ code: "AccountNotFound", account_id: "a" })).toEqual({
      key: "error.AccountNotFound",
    });
    expect(accountImportErrorToI18n({ code: "UnreadableFile" } as never)).toEqual({
      key: "error.Unknown",
    });
  });

  // IMP-020H (owner, 2026-11-22, #722) — a refused mapper file is worded from its typed
  // problem: the field and the line, never the reader's text.
  it.each([
    ["NotJson", null, "mapper_file_not_json", { line: 3 }],
    ["MissingField", "separator", "mapper_file_missing_field", { field: "separator", line: 3 }],
    ["UnknownField", "built_in", "mapper_file_unknown_field", { field: "built_in", line: 3 }],
    ["InvalidValue", null, "mapper_file_invalid_value", { line: 3 }],
  ] as const)("words a %s refusal", (problem, field, key, vars) => {
    expect(
      accountImportErrorToI18n({ code: "MapperFileInvalid", problem, field, line: 3 }),
    ).toEqual({ key: `account_import.error.${key}`, vars });
  });
});

// #548 / IMP-031A — the mapper form offers the deposit or withdrawal by its sign, with its own
// label, beside the other operation types.
describe("OPERATION_TYPE_LABELS", () => {
  it("labels the deposit or withdrawal by its sign", () => {
    expect(OPERATION_TYPE_LABELS.DepositOrWithdrawal).toBe(
      "account_import.type_deposit_or_withdrawal",
    );
  });
});

// #548 / IMP-020F — a mapper's draft holds what its file holds: neither its identity nor the
// built-in mark, which the core refuses in a draft.
describe("draftOf", () => {
  it("leaves out the identity and the built-in mark", () => {
    const draft = draftOf({
      id: "built-in:interactive-brokers-trades",
      built_in: true,
      info: { provider: "Interactive Brokers — Trades" },
      separator: "Comma",
      decimal_mark: "Dot",
      date_format: "YearMonthDay",
      encoding: "Utf8",
      columns: {
        date: "TradeDate",
        operation: "Buy/Sell",
        isin: "ISIN",
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
    });
    expect(Object.keys(draft)).not.toContain("built_in");
    expect(Object.keys(draft)).not.toContain("id");
    expect(draft.info.provider).toBe("Interactive Brokers — Trades");
  });
});
