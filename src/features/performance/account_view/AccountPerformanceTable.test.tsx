import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { presentPeriodRow } from "../shared/presenter";
import { AccountPerformanceTable } from "./AccountPerformanceTable";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const period = (sincePct: number | null) =>
  presentPeriodRow({
    year: 2025,
    month: null,
    end_value: 1_000_000_000,
    previous_value: 900_000_000,
    cash_flow: 0,
    asset_flow: 0,
    dividends: 0,
    pnl: 100_000_000,
    period_over_period: { gain: 100_000_000, pct: 11_110_000 },
    year_to_date: null,
    since_inception: { gain: 400_000_000, pct: sincePct },
    annualized_yield: sincePct === null ? null : { gain: 400_000_000, pct: 5_000_000 },
  });

describe("AccountPerformanceTable", () => {
  // PRF-088 — a suppressed lifetime percentage shows "—" with an info mark, in both columns.
  it("marks the suppressed lifetime percentages", () => {
    render(<AccountPerformanceTable rows={[period(null)]} showYtd={false} showAnnualized />);
    const since = screen.getByTestId("account-performance-since-pct-2025");
    const annualized = screen.getByTestId("account-performance-annualized-2025");
    for (const cell of [since, annualized]) {
      expect(cell.textContent).toBe("—");
      expect(
        cell.querySelector("[title='account_performance.lifetime_unavailable_mark']"),
      ).not.toBeNull();
    }
  });

  // PRF-036 — a present percentage shows as is, without a mark.
  it("shows a present percentage without a mark", () => {
    render(<AccountPerformanceTable rows={[period(20_000_000)]} showYtd={false} showAnnualized />);
    const since = screen.getByTestId("account-performance-since-pct-2025");
    expect(since.textContent).toContain("%");
    expect(since.querySelector("[title]")).toBeNull();
  });
});
