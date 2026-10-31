import { describe, expect, it } from "vitest";
import type { FileRead, ImportMapperDraft } from "@/bindings";
import {
  accountImportErrorToI18n,
  blankDraft,
  columnAsInFile,
  fileNameOf,
  folderOf,
  mapperFileRefusalToI18n,
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
    expect(accountImportErrorToI18n({ code: "MapperFileInvalid", reason: "no date" })).toEqual({
      key: "account_import.error.mapper_file_invalid",
      vars: { reason: "no date" },
    });
    expect(accountImportErrorToI18n({ code: "DatabaseError" })).toEqual({ key: "error.Unknown" });
  });
});
