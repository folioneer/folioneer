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

  // #495, PRF-083 — in an asset-scoped read, a window the position was not held at the
  // start of shows the dash with the reason as its tooltip, as account details do.
  it("explains an absent scoped window on its dash", () => {
    const month = presentPeriodRow({
      year: 2024,
      month: 5,
      end_value: 1_250_000_000,
      previous_value: 1_100_000_000,
      cash_flow: 0,
      asset_flow: 0,
      dividends: 0,
      pnl: 150_000_000,
      period_over_period: { gain: 150_000_000, pct: 13_630_000 },
      year_to_date: null,
      since_inception: { gain: 250_000_000, pct: 25_000_000 },
      annualized_yield: null,
    });
    render(
      <AccountPerformanceTable
        rows={[month]}
        showYtd
        showAnnualized={false}
        absentWindowHint="not held at its start"
      />,
    );
    for (const id of ["ytd-value", "ytd-pct"]) {
      const cell = screen.getByTestId(`account-performance-${id}-2024-5`);
      expect(cell.textContent).toBe("—");
      expect(cell.getAttribute("title")).toBe("not held at its start");
    }
    expect(
      screen.getByTestId("account-performance-pop-pct-2024-5").getAttribute("title"),
    ).toBeNull();
  });

  // PRF-042 — without a scope the table gives no hint: a first period's dash is no window.
  it("gives no hint outside an asset scope", () => {
    render(<AccountPerformanceTable rows={[period(20_000_000)]} showYtd={false} showAnnualized />);
    expect(screen.getByTestId("account-performance-pop-pct-2025").getAttribute("title")).toBeNull();
  });
});
