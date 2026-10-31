import { describe, expect, it } from "vitest";
import type { OperationRead, PreviewRow, RowReason } from "@/bindings";
import { assetPrefillOf, moneyOf, rowReasonToI18n, rowViewOf } from "./previewPresenter";

const asset = { isin: null, reference: "EX", name: "Exemple" };
const purchase: OperationRead = {
  date: "2025-01-08",
  asset,
  currency: null,
  figures: { type: "Purchase", quantity: 20_000_000, unit_price: 90_000_000, fees: 0 },
};
const row = (status: PreviewRow["status"], amount: number | null = 1): PreviewRow => ({
  line: 3,
  status,
  amount,
});

describe("previewPresenter", () => {
  // IMP-066 — each status shows what the row read.
  it("shows a ready purchase with its figures and funding", () => {
    const view = rowViewOf(
      row({ status: "Ready", operation: purchase, asset_id: "a", funding: 7 }, 1_800),
    );
    expect(view).toMatchObject({
      tone: "ready",
      date: "2025-01-08",
      assetName: "Exemple",
      quantity: 20_000_000,
      amount: 1_800,
      funding: 7,
      operation: { key: "account_import.type_purchase" },
    });
  });

  it("shows a joined trade tax with the line it joins", () => {
    const tax: OperationRead = {
      ...purchase,
      figures: { type: "TradeTax", amount: 5, quantity: null },
    };
    expect(
      rowViewOf(row({ status: "JoinedTradeTax", purchase_line: 2, operation: tax })),
    ).toMatchObject({
      status: { key: "account_import.status_joined", vars: { line: 2 } },
      quantity: null,
      tone: "neutral",
    });
  });

  it("shows a not-imported row with the file's label", () => {
    expect(
      rowViewOf(row({ status: "NotImported", label: "DIVISION", date: "2025-03-20", asset }, null)),
    ).toMatchObject({
      date: "2025-03-20",
      operation: { vars: { label: "DIVISION" } },
      note: { key: "account_import.note_not_imported" },
      amount: null,
    });
  });

  it("shows a rejected row without an operation by its reason alone", () => {
    const view = rowViewOf(
      row(
        {
          status: "Rejected",
          reason: { code: "Unread", rejection: { code: "LineTooShort" } },
          operation: null,
        },
        null,
      ),
    );
    expect(view).toMatchObject({
      tone: "rejected",
      date: null,
      note: { key: "account_import.reason_line_too_short" },
    });
  });

  it("gives an already-recorded row its status", () => {
    expect(
      rowViewOf(row({ status: "AlreadyRecorded", operation: purchase, asset_id: "a" })).status,
    ).toEqual({ key: "account_import.status_already_recorded" });
  });

  // IMP-067 — a row reading no asset opens no step.
  it("opens an unknown asset's step only for a row that read one", () => {
    const of = (read: typeof asset) =>
      rowViewOf(row({ status: "UnknownAsset", asset: read, operation: purchase })).opensAsset;
    expect(of(asset)).toBe(true);
    expect(of({ isin: null, reference: null, name: null } as never)).toBe(false);
  });

  // IMP-052 — what the row read and the account's currency.
  it("prefills the asset form", () => {
    expect(assetPrefillOf(asset, "EUR")).toEqual({
      name: "Exemple",
      reference: "EX",
      isin: null,
      currency: "EUR",
      asset_class: null,
      exchange: null,
    });
    expect(assetPrefillOf({ isin: "FR1", reference: null, name: null }, "EUR").name).toBe("");
  });

  it("writes money in the account's currency, or plainly before it is known", () => {
    expect(moneyOf(2_000_000_000, "EUR", "en")).toBe("€2,000.00");
    expect(moneyOf(2_000_000_000, "", "en")).toBe("2,000.00");
  });

  // IMP-030, IMP-035, IMP-037 — each reason has its words.
  it.each<[RowReason, string]>([
    [{ code: "OtherCurrency" }, "account_import.reason_other_currency"],
    [{ code: "AssetNotHeld" }, "account_import.reason_not_held"],
    [
      { code: "ManualEntry", refusal: { code: "ClosedPosition" } },
      "account_import.reason_none_held",
    ],
    [
      { code: "ManualEntry", refusal: { code: "CascadingOversell" } },
      "transaction.error_later_left_short",
    ],
    [
      {
        code: "ManualEntry",
        refusal: { code: "InsufficientCash", current_balance_micros: 0, currency: "EUR" },
      },
      "account_import.reason_no_cash",
    ],
    [{ code: "ManualEntry", refusal: { code: "DateInFuture" } }, "error.DateInFuture"],
    [
      { code: "Unread", rejection: { code: "OperationTypeMissing", label: "x" } },
      "account_import.reason_no_type",
    ],
    [
      { code: "Unread", rejection: { code: "DateUnreadable", column: "x" } },
      "account_import.reason_date",
    ],
    [
      { code: "Unread", rejection: { code: "FigureUnreadable", column: "x" } },
      "account_import.reason_figure",
    ],
    [
      { code: "Unread", rejection: { code: "FigureMissing", field: "x" } },
      "account_import.reason_figure_missing",
    ],
    [
      { code: "Unread", rejection: { code: "QuantityZero" } },
      "account_import.reason_quantity_zero",
    ],
    [
      { code: "Unread", rejection: { code: "TradeTaxUnpaired" } },
      "account_import.reason_tax_unpaired",
    ],
  ])("words %j", (reason, key) => {
    expect(rowReasonToI18n(reason).key).toBe(key);
  });

  it("says how many are held when a sale goes over them", () => {
    expect(
      rowReasonToI18n({
        code: "ManualEntry",
        refusal: { code: "Oversell", available: 10_000_000, requested: 15_000_000 },
      }),
    ).toEqual({ key: "account_import.reason_over_held", vars: { held: "10" } });
  });
});
