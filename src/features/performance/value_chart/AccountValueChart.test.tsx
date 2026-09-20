import { render, screen } from "@testing-library/react";
import { cloneElement, type ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import type { ValueChartPoint } from "../shared/presenter";
import { AccountValueChart } from "./AccountValueChart";

// recharts' ResponsiveContainer measures its parent, which is 0×0 in jsdom and
// suppresses all SVG output. Replace it with a fixed-size pass-through so the
// chart actually renders — the documented recharts+jsdom testing pattern.
vi.mock("recharts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("recharts")>();
  return {
    ...actual,
    ResponsiveContainer: ({
      children,
    }: {
      children: ReactElement<{ width?: number; height?: number }>;
    }) => cloneElement(children, { width: 800, height: 300 }),
  };
});

// Identity i18n — t(key) === key so tests assert on stable keys (F24).
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en-US" } }),
}));

const makePoint = (overrides: Partial<ValueChartPoint> = {}): ValueChartPoint => ({
  key: "2025-1",
  year: 2025,
  month: 1,
  value: 10_000,
  valueFormatted: "10,000.00",
  ...overrides,
});

describe("AccountValueChart", () => {
  it("renders the chart for a multi-point series", () => {
    const points = [
      makePoint({ key: "2025-1", month: 1, value: 9_000 }),
      makePoint({ key: "2025-2", month: 2, value: 9_500 }),
      makePoint({ key: "2025-3", month: 3, value: 10_200 }),
    ];

    const { container } = render(<AccountValueChart points={points} />);

    // Chart container present, empty-state absent, and recharts produced an SVG.
    expect(screen.getByTestId("account-value-chart")).toBeInTheDocument();
    expect(screen.queryByTestId("account-value-chart-empty")).not.toBeInTheDocument();
    expect(container.querySelector("svg")).toBeInTheDocument();
  });

  it("renders a single-point series without falling back to the empty state", () => {
    render(<AccountValueChart points={[makePoint()]} />);

    expect(screen.getByTestId("account-value-chart")).toBeInTheDocument();
    expect(screen.queryByTestId("account-value-chart-empty")).not.toBeInTheDocument();
  });

  it("renders the empty state when there are no points", () => {
    render(<AccountValueChart points={[]} />);

    expect(screen.getByTestId("account-value-chart-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("account-value-chart")).not.toBeInTheDocument();
  });
});
