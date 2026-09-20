import { useTranslation } from "react-i18next";
import type { AssetLookupResult, LookupMode } from "@/bindings";
import { formatAssetClass } from "@/features/assets/shared/presenter";
import { Button } from "@/ui/components/button/Button";
import { TextField } from "@/ui/components/field/TextField";
import { presentWebLookupError } from "./presenter";
import type { WebLookupSearchState } from "./useWebLookupSearch";

interface SearchPanelProps {
  isinQuery: string;
  keywordQuery: string;
  state: WebLookupSearchState;
  lastMode: LookupMode | null;
  submit: (mode: LookupMode) => void;
  retry: () => void;
  setIsinQuery: (q: string) => void;
  setKeywordQuery: (q: string) => void;
  onSelect: (result: AssetLookupResult) => void;
  onFillManually: () => void;
}

export function SearchPanel({
  isinQuery,
  keywordQuery,
  state,
  lastMode,
  submit,
  retry,
  setIsinQuery,
  setKeywordQuery,
  onSelect,
  onFillManually,
}: SearchPanelProps) {
  const { t } = useTranslation();
  const isLoading = state.status === "loading";
  const isIsinLoading = isLoading && lastMode === "Isin";
  const isKeywordLoading = isLoading && lastMode === "Keyword";

  const isError = state.status === "error";
  const errorCode = isError ? state.code : null;
  const errorKey = isError ? presentWebLookupError({ code: state.code }) : null;
  const showIsinError = isError && lastMode === "Isin";
  const showKeywordError = isError && lastMode === "Keyword";

  return (
    <div className="flex flex-col gap-4">
      <form
        id="web-lookup-isin-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit("Isin");
        }}
        className="flex gap-2 items-end"
      >
        <div className="flex-1">
          <TextField
            id="web-lookup-isin-input"
            data-testid="web-lookup-isin-input"
            label={t("asset.web_lookup.isin_label")}
            value={isinQuery}
            onChange={(e) => setIsinQuery(e.target.value)}
            placeholder={t("asset.web_lookup.isin_placeholder")}
            autoFocus
          />
        </div>
        <Button
          id="web-lookup-isin-submit"
          type="submit"
          form="web-lookup-isin-form"
          variant="primary"
          disabled={!isinQuery.trim() || isIsinLoading}
          loading={isIsinLoading}
          data-testid="web-lookup-isin-submit"
        >
          {t("asset.web_lookup.isin_submit")}
        </Button>
      </form>

      {isIsinLoading && (
        <p
          data-testid="web-lookup-isin-loading"
          aria-busy="true"
          className="text-sm text-m3-on-surface-variant"
        >
          {t("asset.web_lookup.loading")}
        </p>
      )}

      {showIsinError && errorKey && (
        <div data-testid="web-lookup-isin-error" role="alert" className="flex flex-col gap-2">
          <p className="text-sm text-m3-error">{t(errorKey)}</p>
          {errorCode !== "InvalidIsinFormat" && (
            <Button
              id="web-lookup-isin-retry"
              variant="outline"
              size="sm"
              aria-label={t("asset.web_lookup.action_retry")}
              onClick={retry}
            >
              {t("asset.web_lookup.action_retry")}
            </Button>
          )}
        </div>
      )}

      <form
        id="web-lookup-keyword-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit("Keyword");
        }}
        className="flex gap-2 items-end"
      >
        <div className="flex-1">
          <TextField
            id="web-lookup-keyword-input"
            data-testid="web-lookup-keyword-input"
            label={t("asset.web_lookup.keyword_label")}
            value={keywordQuery}
            onChange={(e) => setKeywordQuery(e.target.value)}
            placeholder={t("asset.web_lookup.keyword_placeholder")}
          />
        </div>
        <Button
          id="web-lookup-keyword-submit"
          type="submit"
          form="web-lookup-keyword-form"
          variant="primary"
          disabled={!keywordQuery.trim() || isKeywordLoading}
          loading={isKeywordLoading}
          data-testid="web-lookup-keyword-submit"
        >
          {t("asset.web_lookup.keyword_submit")}
        </Button>
      </form>

      {isKeywordLoading && (
        <p
          data-testid="web-lookup-keyword-loading"
          aria-busy="true"
          className="text-sm text-m3-on-surface-variant"
        >
          {t("asset.web_lookup.loading")}
        </p>
      )}

      {showKeywordError && errorKey && (
        <div data-testid="web-lookup-keyword-error" role="alert" className="flex flex-col gap-2">
          <p className="text-sm text-m3-error">{t(errorKey)}</p>
          <Button
            id="web-lookup-keyword-retry"
            variant="outline"
            size="sm"
            aria-label={t("asset.web_lookup.action_retry")}
            onClick={retry}
          >
            {t("asset.web_lookup.action_retry")}
          </Button>
        </div>
      )}

      <div className="min-h-[80px] max-h-[60vh] overflow-y-auto">
        {state.status === "idle" && (
          <p className="text-sm text-m3-on-surface-variant">{t("asset.web_lookup.idle_hint")}</p>
        )}

        {state.status === "empty" && (
          <p className="text-sm text-m3-on-surface-variant">{t("asset.web_lookup.no_results")}</p>
        )}

        {state.status === "results" && (
          <ul className="flex flex-col gap-1">
            {state.results.map((result) => {
              const typeLabel = result.asset_class
                ? formatAssetClass(result.asset_class, t)
                : t("asset.web_lookup.type_unknown");
              const secondLine = result.exchange
                ? `${typeLabel} · ${result.exchange.label}`
                : typeLabel;
              const rowKey = `${result.reference ?? result.name.replace(/\s+/g, "-")}-${result.exchange?.code ?? "none"}`;
              return (
                <li key={`${result.name}|${result.reference ?? ""}|${result.exchange?.code ?? ""}`}>
                  <button
                    id={`web-lookup-result-${rowKey}`}
                    type="button"
                    aria-label={t("asset.web_lookup.select_result", {
                      name: result.name,
                    })}
                    onClick={() => onSelect(result)}
                    className="w-full text-left px-3 py-2 rounded-xl hover:bg-m3-surface-variant/40 transition-colors"
                  >
                    <div className="flex items-baseline gap-1.5">
                      {result.reference && (
                        <span className="text-xs font-mono text-m3-on-surface-variant shrink-0">
                          {result.reference}
                        </span>
                      )}
                      <span className="font-medium text-m3-on-surface text-sm truncate">
                        {result.name}
                      </span>
                    </div>
                    <div className="text-xs text-m3-on-surface-variant mt-0.5">{secondLine}</div>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <div className="flex justify-start pt-1">
        <Button
          variant="ghost"
          size="sm"
          aria-label={t("asset.web_lookup.action_fill_manually")}
          onClick={onFillManually}
        >
          {t("asset.web_lookup.action_fill_manually")}
        </Button>
      </div>
    </div>
  );
}
