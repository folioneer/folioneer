import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import type { FileRead, ImportMapperDraft } from "@/bindings";
import { AccountImportDialog } from "./AccountImportDialog";
import { blankDraft } from "./presenter";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key} ${JSON.stringify(vars)}` : key,
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
  ...overrides,
});
const show = (overrides: Partial<View>) =>
  render(<AccountImportDialog view={view(overrides)} onClose={noop} accountName="PEA" />);
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
  it("shows the counts and the work under way", () => {
    const counts = {
      ready: 2,
      rejected: 1,
      unknown_asset: 0,
      already_recorded: 0,
      not_imported: 0,
    };
    const { unmount } = show({ preview: { file, rows: [], counts } });
    expect(byId("account-import-preview-counts")).toHaveTextContent('"ready":2');
    unmount();
    show({ busy: true, canPreview: false });
    expect(byId("account-import-preview")).toBeDisabled();
  });
});
