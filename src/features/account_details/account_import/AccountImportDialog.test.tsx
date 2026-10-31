import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import type { FileRead, ImportMapperDraft } from "@/bindings";
import { AccountImportDialog } from "./AccountImportDialog";
import { blankDraft } from "./presenter";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key} ${JSON.stringify(vars)}` : key,
    i18n: { language: "en" },
  }),
}));

type View = ComponentProps<typeof AccountImportDialog>["view"];

const file: FileRead = {
  encoding: "Utf8",
  separator: "Semicolon",
  decimal_mark: "Comma",
  date_reading: { format: "DayMonthYear", detected: false },
  columns: ["Date", "Op"],
  rows: 3,
  operation_labels: [{ label: "Buy", rows: 3 }],
};
const draft: ImportMapperDraft = { ...blankDraft(), info: { provider: "Fortuneo" } };
const noop = () => {};
const view = (overrides: Partial<View>): View => ({
  path: "/exports/broker.csv",
  file,
  mappers: [],
  mapperId: null,
  rememberedId: null,
  draft,
  editing: false,
  matches: false,
  lackingColumns: [],
  problem: null,
  addRefusal: null,
  preview: null,
  saved: false,
  busy: false,
  canEdit: false,
  canPreview: true,
  chooseFile: async () => {},
  selectMapper: noop,
  edit: noop,
  changeFormat: noop,
  changeColumn: noop,
  changeLabelType: noop,
  changeProvider: noop,
  saveMapper: noop,
  addMapperFromFile: noop,
  runPreview: noop,
  backToMapper: noop,
  step: {
    rows: [],
    addDeposits: true,
    setAddDeposits: noop,
    focused: null,
    focusedRows: [],
    focus: noop,
    unfocus: noop,
    recordable: 0,
    match: noop,
    createAsset: noop,
    confirm: noop,
  },
  ...overrides,
});
const show = (overrides: Partial<View>) =>
  render(
    <AccountImportDialog
      view={view(overrides)}
      onClose={noop}
      accountName="PEA"
      accountCurrency="EUR"
    />,
  );
const byId = (id: string) => document.getElementById(id);

describe("AccountImportDialog", () => {
  // IMP-060 — each opening starts from picking a file.
  it("starts from picking a file, with no Preview", () => {
    show({ path: null, file: null, draft: null });
    expect(byId("account-import-choose-file")).toBeInTheDocument();
    expect(byId("account-import-preview")).toBeNull();
  });

  // IMP-061 — the format, the columns, each label with its count, the provider.
  it("shows the whole form over a file read", () => {
    show({ editing: true, saved: true });
    expect(byId("account-import-separator")).toBeInTheDocument();
    expect(byId("account-import-column-date")).toHaveTextContent("Op");
    expect(screen.getByText("(3)")).toBeInTheDocument();
    expect(byId("account-import-provider")).toHaveValue("Fortuneo");
    expect(byId("account-import-saved")).toBeInTheDocument();
    expect(byId("account-import-date-undetected")).toBeInTheDocument();
  });

  // IMP-061 — a file not read shows why and only its format.
  it("shows only the format when the file cannot be read", () => {
    show({ editing: true, file: null, problem: { key: "account_import.error.file_empty" } });
    expect(byId("account-import-problem")).toHaveTextContent("account_import.error.file_empty");
    expect(byId("account-import-separator")).toBeInTheDocument();
    expect(byId("account-import-column-date")).toBeNull();
    expect(byId("account-import-provider")).toBeNull();
  });

  // IMP-062 — the file matches, the form folded behind Edit; the folder is named.
  it("folds the form of a mapper the file matches", () => {
    const edit = vi.fn();
    show({ mapperId: "m", rememberedId: "m", matches: true, canEdit: true, edit });
    expect(byId("account-import-matches")).toBeInTheDocument();
    expect(byId("account-import-remembered")).toHaveTextContent("/exports");
    expect(byId("account-import-column-date")).toBeNull();
    fireEvent.click(byId("account-import-edit") as HTMLElement);
    expect(edit).toHaveBeenCalled();
  });

  // IMP-063 — the alert names the columns; Preview is disabled; no Edit.
  it("names the columns a file lacks", () => {
    show({ mapperId: "m", lackingColumns: ["Qté", "Prix"], canPreview: false });
    expect(byId("account-import-mismatch")).toHaveTextContent("Qté, Prix");
    expect(byId("account-import-mismatch")).toHaveTextContent("Fortuneo");
    expect(byId("account-import-preview")).toBeDisabled();
    expect(byId("account-import-edit")).toBeNull();
  });

  // IMP-064 — the alert holds the reason; Preview is disabled.
  it("says why a mapper's file is refused", () => {
    show({
      addRefusal: { key: "account_import.error.mapper_file_invalid", vars: { reason: "no date" } },
      canPreview: false,
    });
    expect(byId("account-import-add-refused")).toHaveTextContent("no date");
    expect(byId("account-import-preview")).toBeDisabled();
  });

  // IMP-065 — the counts, and the action while the window works.
  it("shows the work under way", () => {
    show({ busy: true, canPreview: false });
    expect(byId("account-import-preview")).toBeDisabled();
  });

  describe("the preview's step", () => {
    const counts = {
      ready: 0,
      rejected: 0,
      unknown_asset: 0,
      already_recorded: 0,
      not_imported: 0,
    };
    const previewOf = (over: Partial<typeof counts>, more = {}) => ({
      file,
      rows: [],
      counts: { ...counts, ...over },
      all_recorded: false,
      deposits: { count: 0, total: 0, ready_without: 0 },
      unknown_assets: [],
      matchable_assets: [{ id: "a", name: "Demo Bank" }],
      ...more,
    });
    const row = {
      line: 6,
      status: { key: "account_import.status_unknown_asset" },
      tone: "unknown" as const,
      date: "2025-03-03",
      operation: { key: "account_import.type_purchase" },
      assetName: "EXEMPLE",
      quantity: 10_000_000,
      amount: 420_000_000,
      funding: null,
      note: null,
      opensAsset: true,
    };
    const step = view({}).step;

    // IMP-066, IMP-068 — the counts held, the rows, the action naming what it records.
    it("lists the rows, the counts held and what confirming records", () => {
      const focus = vi.fn();
      show({
        preview: previewOf({ ready: 41, unknown_asset: 1 }),
        step: { ...step, rows: [row], recordable: 41, focus },
      });
      expect(byId("account-import-count-ready")).toBeInTheDocument();
      expect(byId("account-import-count-rejected")).toBeNull();
      expect(byId("account-import-row-6")).toHaveTextContent("EXEMPLE");
      expect(byId("account-import-record")).toHaveTextContent('action_record {"count":41}');
      fireEvent.click(byId("account-import-unknown-6") as HTMLElement);
      expect(focus).toHaveBeenCalledWith(6);
    });

    // IMP-053, IMP-066, IMP-068 — the same file twice records nothing, and says so.
    it("says nothing is left to record when every row is recorded", () => {
      show({ preview: previewOf({ already_recorded: 57 }, { all_recorded: true }), step });
      expect(byId("account-import-preview-hint")).toHaveTextContent("preview_all_recorded");
      expect(byId("account-import-record")).toHaveTextContent("action_nothing");
      expect(byId("account-import-record")).toBeDisabled();
    });

    // IMP-051B — offered only when a purchase needs a deposit.
    it("offers the deposits only when a purchase needs one", () => {
      const setAddDeposits = vi.fn();
      show({ preview: previewOf({ ready: 1 }), step: { ...step, recordable: 1 } });
      expect(byId("account-import-add-deposits")).toBeNull();
      cleanup();
      show({
        preview: previewOf({ ready: 1 }, { deposits: { count: 3, total: 3, ready_without: 0 } }),
        step: { ...step, recordable: 1, setAddDeposits },
      });
      fireEvent.click(byId("account-import-add-deposits") as HTMLElement);
      expect(setAddDeposits).toHaveBeenCalledWith(false);
    });

    // IMP-067 — the asset's step: its rows, the match, and Back to the preview.
    it("shows an unknown asset's step, matching only once an asset is chosen", () => {
      const match = vi.fn();
      const unfocus = vi.fn();
      show({
        preview: previewOf({ unknown_asset: 1 }),
        step: {
          ...step,
          focused: {
            read: "EXEMPLE",
            asset: { isin: null, reference: null, name: "EXEMPLE" },
            lines: [6],
          },
          focusedRows: [row],
          match,
          unfocus,
        },
      });
      expect(byId("account-import-unknown-title")).toHaveTextContent("EXEMPLE");
      expect(byId("account-import-match")).toBeDisabled();
      const select = byId("account-import-match-asset") as HTMLSelectElement;
      expect(Array.from(select.options).map((o) => o.value)).toEqual(["", "a"]);
      fireEvent.change(select, { target: { value: "a" } });
      fireEvent.click(byId("account-import-match") as HTMLElement);
      expect(match).toHaveBeenCalledWith("EXEMPLE", "a");
      fireEvent.click(byId("account-import-back") as HTMLElement);
      expect(unfocus).toHaveBeenCalled();
    });
  });
});
