import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { OperationRead, PreviewRow } from "@/bindings";
import { ImportPreviewTable } from "./ImportPreviewTable";
import { rowViewOf } from "./previewPresenter";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key} ${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

const M = 1_000_000;
const buy: OperationRead = {
  date: "2025-02-14",
  asset: { isin: null, reference: null, name: "Demo" },
  currency: null,
  figures: { type: "Purchase", quantity: 30 * M, unit_price: 50 * M, fees: 0 },
};
const rows: PreviewRow[] = [
  {
    line: 7,
    amount: 1500 * M,
    status: { status: "Ready", asset_id: "a", funding: 1300 * M, operation: buy },
  },
  {
    line: 3,
    amount: 1500 * M,
    status: {
      status: "Rejected",
      reason: {
        code: "ManualEntry",
        refusal: { code: "Oversell", available: M, requested: 2 * M },
      },
      operation: buy,
    },
  },
  {
    line: 9,
    amount: null,
    status: {
      status: "NotImported",
      label: "DIVISION",
      date: "2025-03-20",
      asset: { isin: null, reference: null, name: "Demo" },
    },
  },
];

// IMP-066 — the rows in the file's order, each with its figures and what it has to say.
describe("ImportPreviewTable", () => {
  const table = () =>
    render(<ImportPreviewTable rows={rows.map(rowViewOf)} currency="EUR" />).container;

  it("lists the rows in the order given, with date, quantity and amount", () => {
    const shown = [...table().querySelectorAll("tbody[id^='account-import-row-']")];
    expect(shown.map((row) => row.id)).toEqual([
      "account-import-row-7",
      "account-import-row-3",
      "account-import-row-9",
    ]);
    const first = shown[0]?.textContent ?? "";
    expect(first).toContain("Demo");
    expect(first).toContain("30");
    expect(first).toContain("1,500.00");
  });

  it("says under a row the deposit it needs, its refusal, or that the mapper leaves it out", () => {
    table();
    const note = (line: number) =>
      document.getElementById(`account-import-note-${line}`)?.textContent;
    expect(note(7)).toContain("account_import.note_funding");
    expect(note(7)).toContain("1,300.00");
    expect(note(3)).toBeTruthy();
    expect(note(9)).toBe("account_import.note_not_imported");
  });
});
