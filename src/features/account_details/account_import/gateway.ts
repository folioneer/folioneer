import type {
  AccountImportError,
  CsvImportError,
  FileRead,
  ImportMapper,
  ImportMapperDraft,
  ImportPreview,
  ReadingChoices,
  Result,
} from "@/bindings";
import { commands } from "@/bindings";

/** The account import's commands (IMP spec), each returned as the backend answers it. */
export const accountImportGateway = {
  listImportMappers(): Promise<Result<ImportMapper[], AccountImportError>> {
    return commands.listImportMappers();
  },
  saveImportMapper(
    mapperId: string | null,
    draft: ImportMapperDraft,
  ): Promise<Result<ImportMapper, AccountImportError>> {
    return commands.saveImportMapper(mapperId, draft);
  },
  addImportMapperFromFile(): Promise<Result<ImportMapper | null, AccountImportError>> {
    return commands.addImportMapperFromFile();
  },
  pickImportFile(): Promise<Result<string | null, CsvImportError>> {
    return commands.pickImportFile();
  },
  rememberedImportMapper(path: string): Promise<Result<string | null, CsvImportError>> {
    return commands.rememberedImportMapper(path);
  },
  readImportFile(
    accountId: string,
    path: string,
    choices: ReadingChoices,
  ): Promise<Result<FileRead, CsvImportError>> {
    return commands.readImportFile(accountId, path, choices);
  },
  previewImport(
    accountId: string,
    path: string,
    mapperId: string | null,
    mapper: ImportMapperDraft,
  ): Promise<Result<ImportPreview, CsvImportError>> {
    return commands.previewImport(accountId, path, mapperId, mapper);
  },
  confirmImport(
    accountId: string,
    path: string,
    mapperId: string | null,
    mapper: ImportMapperDraft,
    addDeposits: boolean,
  ): Promise<Result<ImportPreview, CsvImportError>> {
    return commands.confirmImport(accountId, path, mapperId, mapper, addDeposits);
  },
};
