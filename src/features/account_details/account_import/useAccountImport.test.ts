import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FileRead, ImportMapper } from "@/bindings";
import { useAssetCreationRequest } from "@/lib/assetCreationRequest";
import { useAccountImport } from "./useAccountImport";

const gateway = vi.hoisted(() => ({
  listImportMappers: vi.fn(),
  saveImportMapper: vi.fn(),
  addImportMapperFromFile: vi.fn(),
  pickImportFile: vi.fn(),
  rememberedImportMapper: vi.fn(),
  readImportFile: vi.fn(),
  previewImport: vi.fn(),
  confirmImport: vi.fn(),
}));

vi.mock("./gateway", () => ({ accountImportGateway: gateway }));
const onRecorded = vi.fn();
const showSnackbar = vi.fn();
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({ useSnackbar: () => showSnackbar }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, vars?: Record<string, unknown>) =>
      vars ? `${key} ${JSON.stringify(vars)}` : key,
  }),
}));
vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

const ok = <T>(data: T) => ({ status: "ok" as const, data });
const refused = (error: object) => ({ status: "error" as const, error });

const PATH = "/exports/broker.csv";
const fileRead: FileRead = {
  encoding: "Utf8",
  separator: "Semicolon",
  decimal_mark: "Comma",
  date_reading: null,
  columns: ["Date", "Op", "ISIN"],
  rows: 2,
  operation_labels: [],
};
const saved: ImportMapper = {
  id: "fortuneo",
  info: { provider: "Fortuneo" },
  separator: "Comma",
  decimal_mark: "Dot",
  date_format: "YearMonthDay",
  encoding: "Windows1252",
  columns: {
    date: "Date",
    operation: "Op",
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
};
const counts = { ready: 1, rejected: 0, unknown_asset: 0, already_recorded: 0, not_imported: 0 };

const judged = (over = {}) => ({
  file: fileRead,
  rows: [],
  counts,
  all_recorded: false,
  deposits: { count: 0, total: 0, ready_without: 0 },
  unknown_assets: [],
  matchable_assets: [],
  ...over,
});

/** The window with a file picked, its folder remembering `remembered`. */
async function picked(remembered: string | null = null) {
  gateway.pickImportFile.mockResolvedValue(ok(PATH));
  gateway.rememberedImportMapper.mockResolvedValue(ok(remembered));
  const hook = renderHook(() => useAccountImport("acc-1", "EUR", onRecorded));
  await act(() => hook.result.current.chooseFile());
  return hook;
}

describe("useAccountImport", () => {
  beforeEach(() => {
    onRecorded.mockReset();
    showSnackbar.mockReset();
    for (const mock of Object.values(gateway)) mock.mockReset();
    gateway.listImportMappers.mockResolvedValue(ok([saved]));
    gateway.readImportFile.mockResolvedValue(ok(fileRead));
    gateway.previewImport.mockResolvedValue(ok(judged()));
  });

  it("stays on the first step when the dialog is closed", async () => {
    gateway.pickImportFile.mockResolvedValue(ok(null));
    const { result } = renderHook(() => useAccountImport("acc-1", "EUR", onRecorded));
    await act(() => result.current.chooseFile());
    expect(result.current.path).toBeNull();
    expect(gateway.readImportFile).not.toHaveBeenCalled();
  });

  // IMP-061 — no memory: a new mapper over what was detected.
  it("opens a new mapper over the format detected", async () => {
    const { result } = await picked();
    expect(gateway.readImportFile).toHaveBeenCalledWith("acc-1", PATH, {
      encoding: null,
      separator: null,
      decimal_mark: null,
      date_format: null,
      date_column: null,
      operation_column: null,
    });
    expect(result.current.mapperId).toBeNull();
    expect(result.current.editing).toBe(true);
    expect(result.current.draft?.separator).toBe("Semicolon");
    expect(result.current.mappers).toEqual([saved]);
  });

  // IMP-020D, IMP-020E, IMP-062 — the mapper the folder remembers is preselected and reads with
  // its own format; the form stays folded behind Edit and the file is said to match.
  it("preselects the remembered mapper", async () => {
    const { result } = await picked("fortuneo");
    expect(result.current.mapperId).toBe("fortuneo");
    expect(result.current.rememberedId).toBe("fortuneo");
    expect(result.current.editing).toBe(false);
    expect(result.current.matches).toBe(true);
    expect(gateway.readImportFile).toHaveBeenCalledWith(
      "acc-1",
      PATH,
      expect.objectContaining({ separator: "Comma", date_column: "Date", operation_column: "Op" }),
    );
    expect(gateway.previewImport).toHaveBeenCalledWith(
      "acc-1",
      PATH,
      "fortuneo",
      expect.not.objectContaining({ id: expect.anything() }),
    );
    expect(result.current.canEdit).toBe(true);
    act(() => result.current.edit());
    expect(result.current.editing).toBe(true);
    expect(result.current.canEdit).toBe(false);
  });

  // IMP-020D — a remembered mapper no longer held, or a memory that fails, preselects none.
  it("opens a new mapper when the memory holds nothing usable", async () => {
    const { result } = await picked("gone");
    expect(result.current.mapperId).toBeNull();

    gateway.rememberedImportMapper.mockResolvedValue(refused({ code: "FileUnreadable" }));
    await act(() => result.current.chooseFile());
    expect(result.current.mapperId).toBeNull();
    expect(result.current.problem).toBeNull();
  });

  // IMP-021, IMP-063 — every column the file lacks is told, the preview and the form wait.
  it("tells the columns a file lacks", async () => {
    gateway.previewImport.mockResolvedValue(
      refused({ code: "MappedColumnsNotInFile", columns: ["Libellé", "Qté"] }),
    );
    const { result } = await picked("fortuneo");
    expect(result.current.lackingColumns).toEqual(["Libellé", "Qté"]);
    expect(result.current.matches).toBe(false);
    expect(result.current.canPreview).toBe(false);
    expect(result.current.canEdit).toBe(false);
  });

  // IMP-061 — a file that cannot be read shows why, and the format to change is there.
  it("keeps the format to change when a new mapper's first reading fails", async () => {
    gateway.readImportFile.mockResolvedValueOnce(refused({ code: "SeparatorNotFound" }));
    const { result } = await picked();
    expect(result.current.problem?.key).toBe("account_import.error.separator_not_found");
    expect(result.current.file).toBeNull();
    expect(result.current.editing).toBe(true);
    expect(result.current.draft).not.toBeNull();

    act(() => result.current.changeFormat("separator", "Tab"));
    await waitFor(() => expect(result.current.file).toEqual(fileRead));
    expect(result.current.problem).toBeNull();
    expect(gateway.readImportFile).toHaveBeenLastCalledWith(
      "acc-1",
      PATH,
      expect.objectContaining({ separator: "Tab" }),
    );
  });

  // IMP-062 — a saved mapper the file refuses otherwise stays open to edit.
  it("offers Edit when a saved mapper cannot read the file", async () => {
    gateway.readImportFile.mockResolvedValueOnce(refused({ code: "SeparatorNotFound" }));
    const { result } = await picked("fortuneo");
    expect(result.current.matches).toBe(false);
    expect(result.current.canEdit).toBe(true);
    expect(result.current.problem?.key).toBe("account_import.error.separator_not_found");
    expect(gateway.previewImport).not.toHaveBeenCalled();
  });

  // IMP-018 — a choice is read again with the others; a failure stays on the form to change.
  it("reads the file again with the format chosen", async () => {
    const { result } = await picked();
    gateway.readImportFile.mockResolvedValue(refused({ code: "FileNotUtf8" }));
    act(() => result.current.changeFormat("encoding", "Utf8"));
    await waitFor(() =>
      expect(result.current.problem).toEqual({ key: "account_import.error.file_not_utf8" }),
    );
    expect(result.current.file).toBeNull();

    gateway.readImportFile.mockResolvedValue(ok({ ...fileRead, encoding: "Windows1252" }));
    act(() => result.current.changeFormat("encoding", "Windows1252"));
    await waitFor(() => expect(result.current.draft?.encoding).toBe("Windows1252"));
    expect(result.current.problem).toBeNull();
    expect(gateway.readImportFile).toHaveBeenLastCalledWith(
      "acc-1",
      PATH,
      expect.objectContaining({ encoding: "Windows1252", separator: null }),
    );
  });

  // IMP-018, IMP-061 — the select shows what was chosen, so a refused choice can be changed back.
  it("shows a refused format as chosen, the detected one still to pick", async () => {
    gateway.readImportFile.mockResolvedValueOnce(ok({ ...fileRead, encoding: "Windows1252" }));
    const { result } = await picked();
    expect(result.current.draft?.encoding).toBe("Windows1252");
    gateway.readImportFile.mockResolvedValueOnce(refused({ code: "FileNotUtf8" }));
    act(() => result.current.changeFormat("encoding", "Utf8"));
    expect(result.current.draft?.encoding).toBe("Utf8");
    expect(result.current.busy).toBe(true);
    await waitFor(() => expect(result.current.problem).not.toBeNull());
    expect(result.current.busy).toBe(false);
    expect(result.current.draft?.encoding).toBe("Utf8");
  });

  // IMP-065 — a change overtakes a preview running; its result is dropped.
  it("drops a preview a later change overtook, and the counts of the one before", async () => {
    const { result } = await picked();
    await act(async () => result.current.runPreview());
    await waitFor(() => expect(result.current.preview).not.toBeNull());
    let finish: (value: unknown) => void = () => {};
    gateway.previewImport.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    act(() => result.current.runPreview());
    expect(result.current.preview).toBeNull();
    act(() => result.current.changeLabelType("Buy", "Sale"));
    await act(async () => finish(ok(judged())));
    await waitFor(() => expect(result.current.busy).toBe(false));
    expect(result.current.preview).toBeNull();
  });

  // Only the latest reading stands: an earlier one finishing after it changes nothing.
  it("ignores a reading a later change replaced", async () => {
    const { result } = await picked();
    let finishFirst: (value: unknown) => void = () => {};
    gateway.readImportFile.mockReturnValueOnce(
      new Promise((resolve) => {
        finishFirst = resolve;
      }),
    );
    act(() => result.current.changeFormat("separator", "Tab"));
    gateway.readImportFile.mockResolvedValueOnce(ok({ ...fileRead, separator: "Comma" }));
    act(() => result.current.changeFormat("separator", "Comma"));
    await waitFor(() => expect(result.current.draft?.separator).toBe("Comma"));
    await act(async () => finishFirst(ok({ ...fileRead, separator: "Tab" })));
    expect(result.current.draft?.separator).toBe("Comma");
    expect(result.current.file?.separator).toBe("Comma");
  });

  // IMP-014, IMP-022A — mapping the date and operation columns reads the file again for them;
  // another column does not, and leaving an optional one out holds no column.
  it("reads again for the date and operation columns only", async () => {
    const labelled = {
      ...fileRead,
      date_reading: { format: "YearMonthDay" as const, detected: true },
      operation_labels: [{ label: "Buy", rows: 2 }],
    };
    const { result } = await picked();
    gateway.readImportFile.mockClear();
    gateway.readImportFile.mockResolvedValue(ok(labelled));
    act(() => result.current.changeColumn("operation", "Op"));
    await waitFor(() => expect(result.current.file?.operation_labels).toHaveLength(1));
    expect(gateway.readImportFile).toHaveBeenCalledWith(
      "acc-1",
      PATH,
      expect.objectContaining({ operation_column: "Op" }),
    );
    expect(result.current.draft?.date_format).toBe("YearMonthDay");

    gateway.readImportFile.mockClear();
    act(() => result.current.changeColumn("isin", "ISIN"));
    expect(result.current.draft?.columns.isin).toBe("ISIN");
    act(() => result.current.changeColumn("isin", ""));
    expect(result.current.draft?.columns.isin).toBeNull();
    expect(gateway.readImportFile).not.toHaveBeenCalled();
  });

  // IMP-020, IMP-024 — saving names the mapper in use; a refusal says why and changes nothing.
  it("saves the mapper in use", async () => {
    const { result } = await picked();
    gateway.saveImportMapper.mockResolvedValueOnce(
      refused({ code: "MapperProviderTaken", provider: "Fortuneo" }),
    );
    act(() => result.current.changeProvider("Fortuneo"));
    await act(async () => result.current.saveMapper());
    await waitFor(() =>
      expect(result.current.problem?.key).toBe("account_import.error.provider_taken"),
    );
    expect(result.current.mapperId).toBeNull();

    const added: ImportMapper = { ...saved, id: "broker", info: { provider: "Broker" } };
    gateway.saveImportMapper.mockResolvedValueOnce(ok(added));
    gateway.listImportMappers.mockResolvedValue(ok([saved, added]));
    act(() => result.current.changeProvider("Broker"));
    await act(async () => result.current.saveMapper());
    await waitFor(() => expect(result.current.mapperId).toBe("broker"));
    expect(result.current.saved).toBe(true);
    expect(gateway.saveImportMapper).toHaveBeenLastCalledWith(
      null,
      expect.objectContaining({ info: { provider: "Broker" } }),
    );
    expect(result.current.mappers).toHaveLength(2);
    act(() => result.current.changeProvider("Broker 2"));
    expect(result.current.saved).toBe(false);
  });

  // IMP-020B, IMP-064 — a refused file says why and the preview waits; a good one joins and is in use.
  it("adds a mapper from a file", async () => {
    const { result } = await picked();
    gateway.addImportMapperFromFile.mockResolvedValueOnce(
      refused({ code: "MapperFileInvalid", reason: "no date" }),
    );
    await act(async () => result.current.addMapperFromFile());
    await waitFor(() =>
      expect(result.current.addRefusal).toEqual({
        key: "account_import.error.mapper_file_invalid",
        vars: { reason: "no date" },
      }),
    );
    expect(result.current.canPreview).toBe(false);
    expect(result.current.mapperId).toBeNull();

    gateway.addImportMapperFromFile.mockResolvedValueOnce(ok(saved));
    await act(async () => result.current.addMapperFromFile());
    await waitFor(() => expect(result.current.mapperId).toBe("fortuneo"));
    expect(result.current.addRefusal).toBeNull();
    expect(result.current.matches).toBe(true);
  });

  it("leaves everything as it was when the mapper dialog is closed", async () => {
    const { result } = await picked();
    gateway.addImportMapperFromFile.mockResolvedValue(ok(null));
    await act(async () => result.current.addMapperFromFile());
    await waitFor(() => expect(result.current.busy).toBe(false));
    expect(result.current.mapperId).toBeNull();
    expect(result.current.addRefusal).toBeNull();
  });

  // IMP-030, IMP-065 — the preview runs with the mapper in use; a label's type is part of it (IMP-022).
  it("previews with the mapper in use", async () => {
    const { result } = await picked();
    act(() => result.current.changeLabelType("Buy", "Purchase"));
    await act(async () => result.current.runPreview());
    await waitFor(() => expect(result.current.preview?.counts).toEqual(counts));
    expect(gateway.previewImport).toHaveBeenLastCalledWith(
      "acc-1",
      PATH,
      null,
      expect.objectContaining({
        operation_types: [{ label: "Buy", transaction_type: "Purchase" }],
      }),
    );

    gateway.previewImport.mockResolvedValue(refused({ code: "MapperColumnMissing" }));
    await act(async () => result.current.runPreview());
    await waitFor(() =>
      expect(result.current.problem?.key).toBe("account_import.error.column_missing"),
    );
    expect(result.current.canPreview).toBe(false);
  });

  // IMP-020A — a list that fails is said; the window goes on with a new mapper.
  it("says a mappers folder that fails", async () => {
    gateway.listImportMappers.mockResolvedValue(refused({ code: "MapperFolderFailed" }));
    const { result } = await picked();
    expect(result.current.mappers).toEqual([]);
    expect(result.current.problem?.key).toBe("account_import.error.mapper_folder_failed");
    expect(result.current.path).toBe(PATH);
  });

  describe("the preview's step", () => {
    const asset = { isin: "FR0001", reference: null, name: "Exemple" };
    const operation = {
      date: "2025-03-03",
      asset,
      currency: null,
      figures: { type: "Purchase" as const, quantity: 1_000_000, unit_price: 1, fees: 0 },
    };
    const unknownRows = {
      file: fileRead,
      rows: [
        {
          line: 2,
          amount: 1,
          status: { status: "UnknownAsset" as const, asset, operation },
        },
      ],
      counts: { ...counts, ready: 0, unknown_asset: 1 },
      all_recorded: false,
      deposits: { count: 0, total: 0, ready_without: 0 },
      unknown_assets: [{ read: "FR0001", asset, lines: [2] }],
      matchable_assets: [],
    };

    async function previewed(answer: object = unknownRows) {
      const hook = await picked();
      gateway.previewImport.mockResolvedValue(ok(answer));
      await act(async () => hook.result.current.runPreview());
      return hook;
    }

    // IMP-066 — back to the mapper keeps the file and the mapper in use.
    it("goes back to the mapper with nothing lost", async () => {
      const { result } = await previewed();
      expect(result.current.preview).not.toBeNull();
      const draft = result.current.draft;
      act(() => result.current.backToMapper());
      expect(result.current.preview).toBeNull();
      expect(result.current.path).toBe(PATH);
      expect(result.current.draft).toBe(draft);
    });

    // IMP-067 — a match joins the mapper in use and the rows are judged again.
    it("matches an unknown asset to the first text it read and judges again", async () => {
      const { result } = await previewed();
      act(() => result.current.step.focus(2));
      expect(result.current.step.focused?.read).toBe("FR0001");
      expect(result.current.step.focusedRows).toHaveLength(1);
      await act(async () => result.current.step.match("FR0001", "asset-1"));

      expect(gateway.previewImport).toHaveBeenLastCalledWith(
        "acc-1",
        PATH,
        null,
        expect.objectContaining({
          asset_matches: [{ read: "FR0001", asset_id: "asset-1" }],
        }),
      );
      expect(result.current.step.focused).toBeNull();
      expect(result.current.draft?.asset_matches).toHaveLength(1);
    });

    // IMP-052, IMP-052A — the usual form prefilled; the asset created takes the row's text.
    it("asks for the asset prefilled, then matches the one created", async () => {
      const { result } = await previewed();
      act(() => result.current.step.createAsset("FR0001", asset));
      const { request } = useAssetCreationRequest.getState();
      expect(request?.prefill).toEqual({
        name: "Exemple",
        reference: null,
        isin: "FR0001",
        currency: "EUR",
        asset_class: null,
        exchange: null,
      });
      await act(async () => request?.onCreated("created"));
      expect(gateway.previewImport).toHaveBeenLastCalledWith(
        "acc-1",
        PATH,
        null,
        expect.objectContaining({
          asset_matches: [{ read: "FR0001", asset_id: "created" }],
        }),
      );
      act(() => useAssetCreationRequest.getState().close());
    });

    // IMP-051B, IMP-068 — the count follows the choice, which the confirmation carries.
    it("records with the deposits' choice, closes and says how many", async () => {
      const funded = {
        file: fileRead,
        rows: [
          {
            line: 2,
            amount: 1,
            status: { status: "Ready" as const, operation, asset_id: "a", funding: 5 },
          },
          {
            line: 3,
            amount: 1,
            status: { status: "Ready" as const, operation, asset_id: "a", funding: null },
          },
        ],
        counts: { ...counts, ready: 2 },
        all_recorded: false,
        deposits: { count: 1, total: 5, ready_without: 1 },
        unknown_assets: [],
        matchable_assets: [],
      };
      const { result } = await previewed(funded);
      expect(result.current.step.recordable).toBe(2);

      act(() => result.current.step.setAddDeposits(false));
      expect(result.current.step.recordable).toBe(1);

      gateway.confirmImport.mockResolvedValue(ok({ ...funded, counts: { ...counts, ready: 1 } }));
      await act(async () => result.current.step.confirm());
      expect(gateway.confirmImport).toHaveBeenCalledWith(
        "acc-1",
        PATH,
        null,
        expect.anything(),
        false,
      );
      expect(showSnackbar).toHaveBeenCalledWith('account_import.recorded {"count":1}', "success");
      expect(onRecorded).toHaveBeenCalledOnce();
    });

    // IMP-068 — a refusal is shown and the step stays.
    it("stays open on a refused confirmation", async () => {
      const { result } = await previewed();
      gateway.confirmImport.mockResolvedValue({
        status: "error",
        error: { code: "DatabaseError" },
      });
      await act(async () => result.current.step.confirm());
      expect(result.current.problem).toEqual({ key: "error.DatabaseError" });
      expect(result.current.preview).not.toBeNull();
      expect(onRecorded).not.toHaveBeenCalled();
    });
  });
});
