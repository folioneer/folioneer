import { FileUp, TriangleAlert, Upload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/ui/components/button/Button";
import { IconButton } from "@/ui/components/button/IconButton";
import { SelectField } from "@/ui/components/field/SelectField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { messageText } from "@/ui/format/i18n";
import { AccountImportPreview } from "./AccountImportPreview";
import { MapperForm } from "./MapperForm";
import { fileNameOf, folderOf } from "./presenter";
import type { useAccountImport } from "./useAccountImport";

function ImportAlert({ id, title, detail }: { id: string; title: string; detail: string }) {
  return (
    <div
      id={id}
      role="alert"
      className="flex items-start gap-3 rounded-xl border border-m3-error px-4 py-3 text-sm"
    >
      <TriangleAlert size={18} className="mt-0.5 shrink-0 text-m3-error" />
      <div className="min-w-0 break-words">
        <p className="font-semibold text-m3-on-surface">{title}</p>
        <p className="text-m3-on-surface-variant">{detail}</p>
      </div>
    </div>
  );
}

interface AccountImportDialogProps {
  view: ReturnType<typeof useAccountImport>;
  onClose: () => void;
  accountName: string;
  accountCurrency: string;
}

/** What the window shows for the state its hook holds. */
export function AccountImportDialog({
  view,
  onClose,
  accountName,
  accountCurrency,
}: AccountImportDialogProps) {
  const { t } = useTranslation();
  const { path, file, draft } = view;
  const mapperName = draft?.info.provider ?? "";

  if (view.preview) {
    return (
      <AccountImportPreview
        view={view}
        preview={view.preview}
        onClose={onClose}
        accountName={accountName}
        accountCurrency={accountCurrency}
      />
    );
  }

  const footer = (
    <div className="flex justify-end gap-2">
      <Button id="account-import-cancel" variant="secondary" onClick={onClose}>
        {t("action.cancel")}
      </Button>
      {path && (
        <Button
          id="account-import-preview"
          variant="primary"
          disabled={!view.canPreview}
          loading={view.busy}
          onClick={view.runPreview}
        >
          {t("account_import.action_preview")}
        </Button>
      )}
    </div>
  );

  return (
    <FormModal
      id="account-import-modal"
      isOpen
      onClose={onClose}
      title={t("account_import.modal_title", { account: accountName })}
      footer={footer}
      maxWidth="max-w-3xl"
    >
      {!path ? (
        <div className="flex flex-col items-center gap-3 py-8 text-center">
          <FileUp size={32} className="text-m3-on-surface-variant" />
          <p className="text-sm text-m3-on-surface-variant">{t("account_import.pick_hint")}</p>
          <Button
            id="account-import-choose-file"
            variant="primary"
            loading={view.busy}
            onClick={view.chooseFile}
          >
            {t("account_import.action_choose_file")}
          </Button>
          {view.problem && (
            <p id="account-import-problem" role="alert" className="text-sm text-m3-error">
              {messageText(t, view.problem)}
            </p>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between gap-3 rounded-xl bg-m3-surface-container px-4 py-3 text-sm">
            <p id="account-import-file" className="min-w-0 truncate text-m3-on-surface-variant">
              <span className="font-semibold text-m3-on-surface">{fileNameOf(path)}</span>
              {file &&
                ` — ${t("account_import.file_summary", {
                  rows: file.rows,
                  columns: file.columns.length,
                })}`}
            </p>
            <Button
              id="account-import-choose-another"
              variant="ghost"
              disabled={view.busy}
              onClick={view.chooseFile}
            >
              {t("account_import.action_choose_another")}
            </Button>
          </div>

          <div className="flex flex-col gap-1">
            <div className="grid grid-cols-[1fr_auto] items-end gap-3">
              <SelectField
                id="account-import-mapper"
                label={t("account_import.mapper_label")}
                value={view.mapperId ?? ""}
                disabled={view.busy}
                onChange={(e) => view.selectMapper(e.target.value)}
                options={[
                  { value: "", label: t("account_import.new_mapper") },
                  ...view.mappers.map((mapper) => ({
                    value: mapper.id,
                    label: mapper.info.provider,
                  })),
                ]}
              />
              <IconButton
                id="account-import-add-mapper"
                shape="square"
                size="lg"
                variant="tonal"
                icon={<Upload size={20} />}
                disabled={view.busy}
                onClick={view.addMapperFromFile}
                aria-label={t("account_import.action_add_mapper")}
                title={t("account_import.action_add_mapper")}
              />
            </div>
            {view.mapperId !== null && view.mapperId === view.rememberedId && (
              <p id="account-import-remembered" className="px-1 text-xs text-m3-on-surface-variant">
                {t("account_import.remembered_hint", { folder: folderOf(path) })}
              </p>
            )}
          </div>

          {view.addRefusal && (
            <ImportAlert
              id="account-import-add-refused"
              title={t("account_import.add_refused_title")}
              detail={messageText(t, view.addRefusal) ?? ""}
            />
          )}
          {view.lackingColumns.length > 0 && (
            <ImportAlert
              id="account-import-mismatch"
              title={t("account_import.mismatch_title", { mapper: mapperName })}
              detail={t("account_import.mismatch_detail", {
                columns: view.lackingColumns.join(", "),
              })}
            />
          )}
          {view.problem && (
            <p id="account-import-problem" role="alert" className="text-sm text-m3-error">
              {messageText(t, view.problem)}
            </p>
          )}

          {view.matches && !view.editing && (
            <p id="account-import-matches" className="text-sm text-m3-on-surface">
              {t("account_import.matches")}
            </p>
          )}
          {view.canEdit && (
            <div>
              <Button id="account-import-edit" variant="ghost" onClick={view.edit}>
                {t("account_import.action_edit")}
              </Button>
            </div>
          )}

          {view.editing && draft && (
            <MapperForm
              file={file}
              draft={draft}
              busy={view.busy}
              saved={view.saved}
              onFormat={view.changeFormat}
              onColumn={view.changeColumn}
              onLabelType={view.changeLabelType}
              onProvider={view.changeProvider}
              onSave={view.saveMapper}
            />
          )}
        </div>
      )}
    </FormModal>
  );
}
