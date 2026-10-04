import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { monthLabel, type ValueChartPoint } from "../shared/presenter";

/** Data shape fed to recharts: the point plus its render-time translated X-axis label. */
export interface ChartDatum extends ValueChartPoint {
  axisLabel: string;
}

/**
 * Derives the recharts dataset (each point + its translated X-axis label) and a
 * compact number formatter for the Y axis. Keeps this logic out of the
 * presentational chart component (F10).
 */
export function useAccountValueChart(points: ValueChartPoint[]): {
  data: ChartDatum[];
  compactFormatter: Intl.NumberFormat;
} {
  const { t, i18n } = useTranslation();

  const data = useMemo<ChartDatum[]>(
    () =>
      points.map((point) => ({
        ...point,
        axisLabel: point.month !== null ? t(monthLabel(point.month)) : String(point.year),
      })),
    [points, t],
  );

  // NUM-013 — the axis follows the application's language, not the system's.
  const compactFormatter = useMemo(
    () => new Intl.NumberFormat(i18n.language, { notation: "compact" }),
    [i18n.language],
  );

  return { data, compactFormatter };
}
