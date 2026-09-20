import type { TFunction } from "i18next";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { UnpricedAsset } from "@/bindings";
import { microToFormattedPrice } from "@/lib/microUnits";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { FormModal } from "@/ui/components/modal/FormModal";
import { type UnpricedRow, useUnpricedPrices } from "./useUnpricedPrices";

interface UnpricedPricesModalProps {
  assets: UnpricedAsset[];
  onClose: () => void;
}

/**
 * Auto-opened after a fetch task leaves assets unpriced (MKT-172). Lists each
 * unpriced asset with its last-known value, ticker, and ISIN, and lets the user
 * enter a price (MKT-175) or skip it (MKT-176), one row at a time.
 */
export function UnpricedPricesModal({ assets, onClose }: UnpricedPricesModalProps) {
  const { t } = useTranslation();
  const { rows, record, skip } = useUnpricedPrices(assets, onClose);

  return (
    <FormModal
      id="unpriced-prices-modal"
      isOpen
      onClose={onClose}
      title={t("unpriced_prices.title")}
      maxWidth="max-w-3xl"
    >
      <p className="text-sm text-m3-on-surface-variant">{t("unpriced_prices.description")}</p>
      <ul className="flex flex-col divide-y divide-neutral-30">
        {rows.map((row) => (
          <UnpricedRowItem key={row.asset_id} row={row} onRecord={record} onSkip={skip} t={t} />
        ))}
      </ul>
    </FormModal>
  );
}

interface UnpricedRowItemProps {
  row: UnpricedRow;
  onRecord: (assetId: string, price: number) => Promise<void>;
  onSkip: (assetId: string) => void;
  t: TFunction;
}

function UnpricedRowItem({ row, onRecord, onSkip, t }: UnpricedRowItemProps) {
  const [value, setValue] = useState("");
  const trimmed = value.trim();
  const parsed = Number(trimmed);
  const canConfirm = trimmed !== "" && Number.isFinite(parsed) && !row.isSubmitting;

  return (
    <li id={`unpriced-row-${row.asset_id}`} className="flex flex-wrap items-end gap-3 py-3">
      <div className="flex-1 min-w-[8rem]">
        <p className="font-medium text-m3-on-surface">{row.name}</p>
        <p
          id={`unpriced-reference-${row.asset_id}`}
          className="text-xs text-m3-on-surface-variant tabular-nums"
        >
          {row.reference}
        </p>
        {row.isin && (
          <p
            id={`unpriced-isin-${row.asset_id}`}
            className="text-xs text-m3-on-surface-variant tabular-nums"
          >
            {row.isin}
          </p>
        )}
      </div>

      <div
        id={`unpriced-last-price-${row.asset_id}`}
        className="min-w-[6rem] text-right text-sm tabular-nums text-m3-on-surface-variant"
      >
        {row.last_price != null
          ? `${microToFormattedPrice(row.last_price)} ${row.currency}`
          : t("unpriced_prices.no_previous_price")}
      </div>

      <div className="w-32">
        <TextField
          id={`unpriced-price-input-${row.asset_id}`}
          label={row.currency}
          type="number"
          inputMode="decimal"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          disabled={row.isSubmitting}
        />
      </div>

      <Button
        id={`unpriced-confirm-${row.asset_id}`}
        size="sm"
        loading={row.isSubmitting}
        disabled={!canConfirm}
        onClick={() => void onRecord(row.asset_id, parsed)}
      >
        {t("action.confirm")}
      </Button>
      <Button
        id={`unpriced-skip-${row.asset_id}`}
        size="sm"
        variant="ghost"
        disabled={row.isSubmitting}
        onClick={() => onSkip(row.asset_id)}
      >
        {t("unpriced_prices.skip")}
      </Button>

      {row.error && (
        <p
          id={`unpriced-error-${row.asset_id}`}
          role="alert"
          className="w-full text-xs text-m3-error"
        >
          {t(row.error.key, row.error.vars)}
        </p>
      )}
    </li>
  );
}
