import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AssetRead, CsvImportError, ImportMapperDraft, ImportPreview } from "@/bindings";
import { useAssetCreationRequest } from "@/lib/assetCreationRequest";
import { useSnackbar } from "@/ui/components/snackbar/snackbarStore";
import { accountImportGateway } from "./gateway";
import { assetPrefillOf, rowViewOf } from "./previewPresenter";

interface UseImportPreviewStepArgs {
  accountId: string;
  accountCurrency: string;
  path: string | null;
  mapperId: string | null;
  draft: ImportMapperDraft | null;
  preview: ImportPreview | null;
  /** The mapper in use changes, and the file is judged again with it (IMP-033). */
  rejudge: (draft: ImportMapperDraft) => void;
  withBusy: (work: () => Promise<void>) => Promise<void>;
  onRefused: (error: CsvImportError) => void;
  onRecorded: () => void;
}

/** The preview's step: its rows, the unknown assets, the deposits, the confirmation. */
export function useImportPreviewStep({
  accountId,
  accountCurrency,
  path,
  mapperId,
  draft,
  preview,
  rejudge,
  withBusy,
  onRefused,
  onRecorded,
}: UseImportPreviewStepArgs) {
  const { t } = useTranslation();
  const showSnackbar = useSnackbar();
  const requestAsset = useAssetCreationRequest((state) => state.open);
  const [addDeposits, setAddDeposits] = useState(true);
  const [focusedLine, setFocusedLine] = useState<number | null>(null);

  const rows = useMemo(() => preview?.rows.map(rowViewOf) ?? [], [preview]);
  // IMP-067 — the unknown asset a row belongs to, and its rows.
  const focused = useMemo(
    () => preview?.unknown_assets.find((group) => group.lines.includes(focusedLine ?? -1)) ?? null,
    [focusedLine, preview],
  );
  const focusedRows = useMemo(() => {
    const byLine = new Map(rows.map((row) => [row.line, row]));
    return (focused?.lines ?? []).flatMap((line) => byLine.get(line) ?? []);
  }, [focused, rows]);
  // IMP-051B — unticked, the purchases needing a deposit are rejected at the confirmation.
  const recordable = addDeposits
    ? (preview?.counts.ready ?? 0)
    : (preview?.deposits.ready_without ?? 0);

  // IMP-033, IMP-052A — the match joins the mapper in use.
  const match = useCallback(
    (read: string, assetId: string) => {
      if (!draft) return;
      setFocusedLine(null);
      rejudge({ ...draft, asset_matches: [...draft.asset_matches, { read, asset_id: assetId }] });
    },
    [draft, rejudge],
  );

  const createAsset = useCallback(
    (read: string, asset: AssetRead) => {
      requestAsset({
        prefill: assetPrefillOf(asset, accountCurrency),
        onCreated: (assetId) => match(read, assetId),
      });
    },
    [accountCurrency, match, requestAsset],
  );

  // IMP-050, IMP-068 — the window closes on what was recorded; a refusal leaves it open.
  const confirm = useCallback(() => {
    if (!path || !draft) return;
    void withBusy(async () => {
      const result = await accountImportGateway.confirmImport(
        accountId,
        path,
        mapperId,
        draft,
        addDeposits,
      );
      if (result.status === "error") {
        onRefused(result.error);
        return;
      }
      const recorded = result.data.counts.ready;
      showSnackbar(
        recorded > 0
          ? t("account_import.recorded", { count: recorded })
          : t("account_import.recorded_none"),
        recorded > 0 ? "success" : "info",
      );
      onRecorded();
    });
  }, [
    accountId,
    addDeposits,
    draft,
    mapperId,
    onRecorded,
    onRefused,
    path,
    showSnackbar,
    t,
    withBusy,
  ]);

  return {
    rows,
    addDeposits,
    setAddDeposits,
    focused,
    focusedRows,
    focus: setFocusedLine,
    unfocus: useCallback(() => setFocusedLine(null), []),
    recordable,
    match,
    createAsset,
    confirm,
  };
}
