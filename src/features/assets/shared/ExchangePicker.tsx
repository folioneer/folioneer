import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Exchange } from "@/bindings";
import { SelectField } from "@/ui/components/field/SelectField";
import { useAssetsStore } from "../store";

interface ExchangePickerProps {
  value: Exchange | null;
  onChange: (exchange: Exchange | null) => void;
  idPrefix?: string;
}

export function ExchangePicker({ value, onChange, idPrefix = "asset" }: ExchangePickerProps) {
  const { t } = useTranslation();
  const supportedExchanges = useAssetsStore((s) => s.supportedExchanges);
  const loadSupportedExchanges = useAssetsStore((s) => s.loadSupportedExchanges);

  useEffect(() => {
    if (supportedExchanges.length === 0) {
      loadSupportedExchanges();
    }
  }, [supportedExchanges.length, loadSupportedExchanges]);

  const options = [
    { label: t("asset.exchange_none_option"), value: "" },
    ...supportedExchanges.map((ex) => ({
      label: `${ex.label} (${ex.code})`,
      value: ex.code,
    })),
  ];

  const handleSelect = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const code = e.target.value;
    if (code === "") {
      onChange(null);
      return;
    }
    const picked = supportedExchanges.find((ex) => ex.code === code);
    onChange(picked ?? null);
  };

  return (
    <SelectField
      id={`${idPrefix}-exchange-picker`}
      data-testid="asset-exchange-picker"
      label={t("asset.field_exchange")}
      value={value?.code ?? ""}
      onChange={handleSelect}
      options={options}
    />
  );
}
