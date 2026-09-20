import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PnlCell } from "./PnlCell";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("PnlCell", () => {
  it("displays formatted value for positive PnL", () => {
    const { getByText } = render(<PnlCell value="+€100" raw={100} />);
    expect(getByText("+€100")).toBeTruthy();
  });

  it("displays formatted value for negative PnL", () => {
    const { getByText } = render(<PnlCell value="-€50" raw={-50} />);
    expect(getByText("-€50")).toBeTruthy();
  });

  it("displays placeholder for zero PnL", () => {
    const { getByText } = render(<PnlCell value="€0" raw={0} />);
    expect(getByText("account_details.pnl_placeholder")).toBeTruthy();
  });
});
