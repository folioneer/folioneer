import { useCallback, useEffect, useRef, useState } from "react";
import type {
  AccountImportError,
  CsvImportError,
  FileRead,
  ImportMapper,
  ImportMapperDraft,
  ImportPreview,
  MappedColumns,
  OperationType,
} from "@/bindings";
import { logger } from "@/lib/logger";
import type { I18nMessage } from "@/ui/format/i18n";
import { accountImportGateway } from "./gateway";
import {
  accountImportErrorToI18n,
  blankDraft,
  draftOf,
  type FormatChoices,
  type FormatValue,
  mapperFileRefusalToI18n,
  NO_CHOICES,
  withFormatOf,
  withLabelType,
} from "./presenter";
import { useImportPreviewStep } from "./useImportPreviewStep";

type FormatField = keyof FormatChoices;

/**
 * The import window before its preview: the file picked and read, the mapper in use, its
 * format, columns and operations (IMP-018, IMP-020 to IMP-022). Holds view state only: every
 * reading and refusal is the core's.
 */
export function useAccountImport(
  accountId: string,
  accountCurrency: string,
  onRecorded: () => void,
) {
  const [path, setPath] = useState<string | null>(null);
  const [file, setFile] = useState<FileRead | null>(null);
  const [mappers, setMappers] = useState<ImportMapper[]>([]);
  const [mapperId, setMapperId] = useState<string | null>(null);
  const [rememberedId, setRememberedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ImportMapperDraft | null>(null);
  const [choices, setChoices] = useState<FormatChoices>(NO_CHOICES);
  const [editing, setEditing] = useState(false);
  const [matches, setMatches] = useState(false);
  const [lackingColumns, setLackingColumns] = useState<string[]>([]);
  const [problem, setProblem] = useState<I18nMessage | null>(null);
  const [addRefusal, setAddRefusal] = useState<I18nMessage | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [saved, setSaved] = useState(false);
  const [working, setWorking] = useState(0);
  // Only the latest work stands: a change made while a reading or a preview runs replaces it.
  const latestReading = useRef(0);

  useEffect(() => {
    logger.info("[AccountImportModal] mounted");
  }, []);

  // F14 — a refusal is logged with its code, then said in the window.
  const refuse = useCallback((error: CsvImportError | AccountImportError) => {
    logger.error("[useAccountImport] refused", { error });
    setProblem(accountImportErrorToI18n(error));
  }, []);

  const clearOutcome = useCallback(() => {
    latestReading.current += 1;
    setProblem(null);
    setAddRefusal(null);
    setLackingColumns([]);
    setMatches(false);
    setPreview(null);
    setSaved(false);
  }, []);

  const read = useCallback(
    async (at: string, mapper: ImportMapperDraft | null, format: FormatChoices) => {
      const reading = ++latestReading.current;
      const result = await accountImportGateway.readImportFile(accountId, at, {
        ...format,
        date_column: mapper?.columns.date || null,
        operation_column: mapper?.columns.operation || null,
      });
      if (reading !== latestReading.current) return null;
      if (result.status === "error") {
        // IMP-061 — a refused reading leaves no columns or labels of the one before it.
        setFile(null);
        refuse(result.error);
        return null;
      }
      setFile(result.data);
      return result.data;
    },
    [accountId, refuse],
  );

  const startNewMapper = useCallback(
    async (at: string) => {
      setMapperId(null);
      setChoices(NO_CHOICES);
      setEditing(true);
      // IMP-061 — a file not read yet still gives the format to choose.
      setDraft(blankDraft());
      const fileRead = await read(at, null, NO_CHOICES);
      if (fileRead) setDraft(withFormatOf(blankDraft(), fileRead));
    },
    [read],
  );

  // IMP-020E — a saved mapper reads the file with its own format; run through the preview to
  // tell at once whether the file matches it (IMP-021).
  const applySavedMapper = useCallback(
    async (at: string, mapper: ImportMapper) => {
      const saved = draftOf(mapper);
      const format: FormatChoices = {
        encoding: mapper.encoding,
        separator: mapper.separator,
        decimal_mark: mapper.decimal_mark,
        date_format: mapper.date_format,
      };
      setMapperId(mapper.id);
      setChoices(format);
      setEditing(false);
      setDraft(saved);
      const fileRead = await read(at, saved, format);
      if (!fileRead) return;
      const reading = latestReading.current;
      const result = await accountImportGateway.previewImport(accountId, at, mapper.id, saved);
      if (reading !== latestReading.current) return;
      if (result.status === "ok") {
        setMatches(true);
      } else if (result.error.code === "MappedColumnsNotInFile") {
        setLackingColumns(result.error.columns);
      } else {
        refuse(result.error);
      }
    },
    [accountId, read, refuse],
  );

  const withBusy = useCallback(async (work: () => Promise<void>) => {
    setWorking((count) => count + 1);
    try {
      await work();
    } finally {
      setWorking((count) => count - 1);
    }
  }, []);

  const listMappers = useCallback(async () => {
    const result = await accountImportGateway.listImportMappers();
    if (result.status === "error") {
      refuse(result.error);
      return [];
    }
    setMappers(result.data);
    return result.data;
  }, [refuse]);

  const chooseFile = useCallback(
    () =>
      withBusy(async () => {
        const picked = await accountImportGateway.pickImportFile();
        if (picked.status === "error") {
          refuse(picked.error);
          return;
        }
        if (picked.data === null) return;
        const at = picked.data;
        clearOutcome();
        setPath(at);
        setFile(null);
        const held = await listMappers();
        // IMP-020D — a memory that cannot be told preselects nothing; the window goes on.
        const remembered = await accountImportGateway.rememberedImportMapper(at);
        const id = remembered.status === "ok" ? remembered.data : null;
        if (remembered.status === "error") {
          logger.warn("[AccountImportModal] no remembered mapper", remembered.error);
        }
        setRememberedId(id);
        const mapper = held.find((candidate) => candidate.id === id);
        await (mapper ? applySavedMapper(at, mapper) : startNewMapper(at));
      }),
    [clearOutcome, listMappers, refuse, startNewMapper, applySavedMapper, withBusy],
  );

  const selectMapper = useCallback(
    (id: string) => {
      if (!path) return;
      clearOutcome();
      const mapper = mappers.find((candidate) => candidate.id === id);
      void withBusy(() => (mapper ? applySavedMapper(path, mapper) : startNewMapper(path)));
    },
    [clearOutcome, mappers, path, startNewMapper, applySavedMapper, withBusy],
  );

  // IMP-018 — the choice replaces the detected format, and the file is read again with it.
  const changeFormat = useCallback(
    (field: FormatField, value: FormatValue) => {
      if (!path || !draft) return;
      clearOutcome();
      const format = { ...choices, [field]: value };
      const next: ImportMapperDraft = { ...draft, [field]: value };
      setChoices(format);
      setDraft(next);
      void withBusy(async () => {
        const fileRead = await read(path, next, format);
        if (fileRead) setDraft((held) => held && withFormatOf(held, fileRead));
      });
    },
    [choices, clearOutcome, draft, path, read, withBusy],
  );

  // IMP-014, IMP-022A — the date and operation columns are what the file is read again for.
  const changeColumn = useCallback(
    (field: keyof MappedColumns, value: string) => {
      if (!path || !draft) return;
      clearOutcome();
      const next: ImportMapperDraft = {
        ...draft,
        columns: {
          ...draft.columns,
          [field]: value === "" && field !== "date" && field !== "operation" ? null : value,
        },
      };
      setDraft(next);
      if (field === "date" || field === "operation") {
        void withBusy(async () => {
          const fileRead = await read(path, next, choices);
          if (fileRead) setDraft((held) => held && withFormatOf(held, fileRead));
        });
      }
    },
    [choices, clearOutcome, draft, path, read, withBusy],
  );

  const changeLabelType = useCallback(
    (label: string, type: OperationType | null) => {
      clearOutcome();
      setDraft((held) => held && withLabelType(held, label, type));
    },
    [clearOutcome],
  );

  const changeProvider = useCallback((provider: string) => {
    setProblem(null);
    setAddRefusal(null);
    setLackingColumns([]);
    setSaved(false);
    setDraft((held) => held && { ...held, info: { provider } });
  }, []);

  const saveMapper = useCallback(() => {
    if (!draft) return;
    setProblem(null);
    void withBusy(async () => {
      const result = await accountImportGateway.saveImportMapper(mapperId, draft);
      if (result.status === "error") {
        refuse(result.error);
        return;
      }
      setMapperId(result.data.id);
      setDraft(draftOf(result.data));
      await listMappers();
      setSaved(true);
    });
  }, [draft, listMappers, mapperId, refuse, withBusy]);

  // IMP-020B — a refused file says why; nothing changes in the window.
  const addMapperFromFile = useCallback(() => {
    void withBusy(async () => {
      const result = await accountImportGateway.addImportMapperFromFile();
      if (result.status === "error") {
        logger.error("[useAccountImport] mapper file refused", { error: result.error });
        setAddRefusal(mapperFileRefusalToI18n(result.error));
        return;
      }
      if (result.data === null || !path) return;
      clearOutcome();
      await listMappers();
      await applySavedMapper(path, result.data);
    });
  }, [clearOutcome, listMappers, path, applySavedMapper, withBusy]);

  const runPreview = useCallback(() => {
    if (!path || !draft) return;
    setProblem(null);
    setPreview(null);
    const reading = ++latestReading.current;
    void withBusy(async () => {
      const result = await accountImportGateway.previewImport(accountId, path, mapperId, draft);
      if (reading !== latestReading.current) return;
      if (result.status === "ok") {
        setPreview(result.data);
      } else if (result.error.code === "MappedColumnsNotInFile") {
        setLackingColumns(result.error.columns);
      } else {
        refuse(result.error);
      }
    });
  }, [accountId, draft, mapperId, path, refuse, withBusy]);

  const edit = useCallback(() => setEditing(true), []);

  // IMP-066 — back to the mapper, with everything in it kept, its new matches too.
  const backToMapper = useCallback(() => {
    setProblem(null);
    setPreview(null);
  }, []);

  // IMP-033 — the rows judged again; the preview stays until the new one replaces it, and a
  // refused match does not join the mapper.
  const rejudge = useCallback(
    (next: ImportMapperDraft) => {
      if (!path) return;
      setProblem(null);
      const reading = ++latestReading.current;
      void withBusy(async () => {
        const result = await accountImportGateway.previewImport(accountId, path, mapperId, next);
        if (reading !== latestReading.current) return;
        if (result.status === "ok") {
          setDraft(next);
          setPreview(result.data);
        } else {
          refuse(result.error);
        }
      });
    },
    [accountId, mapperId, path, refuse, withBusy],
  );

  const step = useImportPreviewStep({
    accountId,
    accountCurrency,
    path,
    mapperId,
    draft,
    preview,
    rejudge,
    withBusy,
    onRefused: refuse,
    onRecorded,
  });

  const busy = working > 0;

  return {
    path,
    file,
    mappers,
    mapperId,
    rememberedId,
    draft,
    editing,
    matches,
    lackingColumns,
    problem,
    addRefusal,
    preview,
    saved,
    busy,
    // IMP-062, IMP-063 — a file the saved mapper lacks columns for is not edited into matching.
    canEdit: mapperId !== null && !editing && !busy && lackingColumns.length === 0,
    canPreview:
      file !== null &&
      draft !== null &&
      !busy &&
      problem === null &&
      addRefusal === null &&
      lackingColumns.length === 0,
    chooseFile,
    selectMapper,
    edit,
    changeFormat,
    changeColumn,
    changeLabelType,
    changeProvider,
    saveMapper,
    addMapperFromFile,
    runPreview,
    backToMapper,
    step,
  };
}
