import { useTranslation } from "react-i18next";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { ValueChartPoint } from "../shared/presenter";
import { type ChartDatum, useAccountValueChart } from "./useAccountValueChart";

interface AccountValueChartProps {
  /** Chronological value-over-time series (oldest→newest), already transformed (F5/F10). */
  points: ValueChartPoint[];
  /** Base of every id/data-testid, so two pages rendering this chart never emit colliding ids. */
  idPrefix?: string;
}

/**
 * PRF — account value over time. Presentational line chart fed by already-transformed
 * view-model points (formatting lives in the presenter/hook per F5/F10). Themed with M3
 * tokens via CSS variables, so it tracks the `.dark` class automatically.
 */
export function AccountValueChart({ points, idPrefix = "account" }: AccountValueChartProps) {
  const { t } = useTranslation();
  const { data, compactFormatter } = useAccountValueChart(points);

  if (points.length === 0) {
    return (
      <div
        id={`${idPrefix}-value-chart-empty`}
        data-testid={`${idPrefix}-value-chart-empty`}
        className="flex items-center justify-center h-40 text-m3-on-surface-variant italic text-sm"
      >
        {t("account_performance.chart.empty")}
      </div>
    );
  }

  return (
    <section
      id={`${idPrefix}-value-chart`}
      data-testid={`${idPrefix}-value-chart`}
      className="px-4 pt-2"
      aria-label={t("account_performance.chart.title")}
    >
      <h3 className="text-sm font-medium text-m3-on-surface-variant mb-2">
        {t("account_performance.chart.title")}
      </h3>
      <div className="h-56 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={data} margin={{ top: 8, right: 16, bottom: 8, left: 8 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--color-m3-outline-variant)" />
            <XAxis
              dataKey="axisLabel"
              tick={{ fontSize: 12, fill: "var(--color-m3-on-surface-variant)" }}
              stroke="var(--color-m3-outline)"
            />
            <YAxis
              tick={{ fontSize: 12, fill: "var(--color-m3-on-surface-variant)" }}
              stroke="var(--color-m3-outline)"
              tickFormatter={(value: number) => compactFormatter.format(value)}
              width={56}
            />
            <Tooltip
              content={
                <ValueChartTooltip
                  valueLabel={t("account_performance.chart.value_label")}
                  testId={`${idPrefix}-value-chart-tooltip`}
                />
              }
            />
            <Line
              type="monotone"
              dataKey="value"
              stroke="var(--color-m3-primary)"
              strokeWidth={2}
              dot={{ r: 3, fill: "var(--color-m3-primary)" }}
              activeDot={{ r: 5 }}
              isAnimationActive={false}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </section>
  );
}

interface ValueChartTooltipProps {
  active?: boolean;
  payload?: { payload: ChartDatum }[];
  valueLabel: string;
  testId: string;
}

/** Custom tooltip rendering the formatted period-end value and its date label. */
function ValueChartTooltip({ active, payload, valueLabel, testId }: ValueChartTooltipProps) {
  const first = active ? payload?.[0] : undefined;
  if (!first) return null;
  const datum = first.payload;
  return (
    <div
      data-testid={testId}
      className="rounded-lg bg-m3-surface-container-high px-3 py-2 shadow-elevation-2 text-sm"
    >
      <div className="text-m3-on-surface-variant">{datum.axisLabel}</div>
      <div className="text-m3-on-surface font-medium">
        {valueLabel}: {datum.valueFormatted}
      </div>
    </div>
  );
}
