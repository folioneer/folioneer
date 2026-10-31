import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ImportPreview, MatchableAsset, UnknownAssetRows } from "@/bindings";
import { Button } from "@/ui/components/button/Button";
import { SelectField } from "@/ui/components/field/SelectField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { messageText } from "@/ui/format/i18n";
import { ImportPreviewTable, StatusChip } from "./ImportPreviewTable";
import { moneyOf, type RowTone } from "./previewPresenter";
import type { useAccountImport } from "./useAccountImport";

type View = ReturnType<typeof useAccountImport>;

interface AccountImportPreviewProps {
  view: View;
  preview: ImportPreview;
  onClose: () => void;
  accountName: string;
  accountCurrency: string;
}

const COUNTS: { field: keyof ImportPreview["counts"]; tone: RowTone }[] = [
  { field: "ready", tone: "ready" },
  { field: "already_recorded", tone: "neutral" },
  { field: "unknown_asset", tone: "unknown" },
  { field: "rejected", tone: "rejected" },
  { field: "not_imported", tone: "neutral" },
];

/** IMP-067 — an unknown asset: its rows, a match to an asset of the portfolio, or a new asset. */
function UnknownAssetStep({
  view,
  group,
  matchable,
  accountCurrency,
}: {
  view: View;
  group: UnknownAssetRows;
  matchable: MatchableAsset[];
  accountCurrency: string;
}) {
  const { t } = useTranslation();
  const { step, busy } = view;
  const [assetId, setAssetId] = useState("");
  const name = step.focusedRows[0]?.assetName ?? group.read;
  return (
    <div className="flex flex-col gap-4">
      <h3 id="account-import-unknown-title" className="text-sm font-semibold text-m3-on-surface">
        {t("account_import.unknown_title", { name, count: step.focusedRows.length })}
      </h3>
      <p className="text-sm text-m3-on-surface-variant">{t("account_import.unknown_hint")}</p>
      <div className="grid grid-cols-[1fr_auto_auto] items-end gap-3">
        <SelectField
          id="account-import-match-asset"
          label={t("account_import.match_label")}
          value={assetId}
          disabled={busy}
          onChange={(e) => setAssetId(e.target.value)}
          options={[
            { value: "", label: t("account_import.match_choose") },
            ...matchable.map((held) => ({ value: held.id, label: held.name })),
          ]}
        />
        <Button
          id="account-import-match"
          variant="tonal"
          disabled={assetId === "" || busy}
          onClick={() => step.match(group.read, assetId)}
        >
          {t("account_import.action_match")}
        </Button>
        <Button
          id="account-import-create-asset"
          variant="tonal"
          disabled={busy}
          onClick={() => step.createAsset(group.read, group.asset)}
        >
          {t("account_import.action_create_asset")}
        </Button>
      </div>
      <ImportPreviewTable
        rows={step.focusedRows}
        currency={accountCurrency}
        fundingNote={(amount) => t("account_import.note_funding", { amount })}
      />
    </div>
  );
}

/** The preview's step (IMP-066 to IMP-068): nothing is recorded until the user confirms. */
export function AccountImportPreview({
  view,
  preview,
  onClose,
  accountName,
  accountCurrency,
}: AccountImportPreviewProps) {
  const { t, i18n } = useTranslation();
  const { step, busy, problem } = view;
  const { counts } = preview;
  const onUnknownStep = step.focused !== null;

  const footer = (
    <div className="flex items-center justify-between gap-2">
      <Button
        id="account-import-back"
        variant="ghost"
        onClick={onUnknownStep ? step.unfocus : view.backToMapper}
      >
        {t(onUnknownStep ? "account_import.action_back_preview" : "account_import.action_back")}
      </Button>
      <div className="flex gap-2">
        <Button id="account-import-cancel" variant="secondary" onClick={onClose}>
          {t("action.cancel")}
        </Button>
        <Button
          id="account-import-record"
          variant="primary"
          disabled={step.recordable <= 0}
          loading={busy}
          onClick={step.confirm}
        >
          {step.recordable > 0
            ? t("account_import.action_record", { count: step.recordable })
            : t("account_import.action_nothing")}
        </Button>
      </div>
    </div>
  );

  return (
    <FormModal
      id="account-import-modal"
      isOpen
      onClose={onClose}
      title={t("account_import.preview_title", { account: accountName })}
      footer={footer}
      maxWidth="max-w-4xl"
    >
      {step.focused ? (
        <UnknownAssetStep
          key={step.focused.read}
          view={view}
          group={step.focused}
          matchable={preview.matchable_assets}
          accountCurrency={accountCurrency}
        />
      ) : (
        <div className="flex flex-col gap-4">
          <p id="account-import-preview-hint" className="text-sm text-m3-on-surface">
            {t(
              preview.all_recorded
                ? "account_import.preview_all_recorded"
                : "account_import.preview_hint",
            )}
          </p>
          <div id="account-import-counts" role="status" className="flex flex-wrap gap-2">
            {COUNTS.map(
              ({ field, tone }) =>
                counts[field] > 0 && (
                  <StatusChip
                    key={field}
                    id={`account-import-count-${field.replace("_", "-")}`}
                    tone={tone}
                    label={t(`account_import.count_${field}`, { count: counts[field] })}
                  />
                ),
            )}
          </div>
          {problem && (
            <p id="account-import-problem" role="alert" className="text-sm text-m3-error">
              {messageText(t, problem)}
            </p>
          )}
          <ImportPreviewTable
            rows={step.rows}
            currency={accountCurrency}
            onUnknown={step.focus}
            fundingNote={(amount) => t("account_import.note_funding", { amount })}
          />
          {preview.deposits.count > 0 && (
            <label
              htmlFor="account-import-add-deposits"
              className="flex cursor-pointer items-start gap-3 text-sm"
            >
              <input
                id="account-import-add-deposits"
                type="checkbox"
                className="mt-1 accent-m3-primary"
                checked={step.addDeposits}
                disabled={busy}
                onChange={(e) => step.setAddDeposits(e.target.checked)}
              />
              <span>
                <span className="block font-semibold text-m3-on-surface">
                  {t("account_import.deposits_label")}
                </span>
                <span
                  id="account-import-deposits-detail"
                  className="block text-xs text-m3-on-surface-variant"
                >
                  {t("account_import.deposits_detail", {
                    count: preview.deposits.count,
                    amount: moneyOf(preview.deposits.total, accountCurrency, i18n.language),
                  })}
                </span>
              </span>
            </label>
          )}
        </div>
      )}
    </FormModal>
  );
}
