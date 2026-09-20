import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/lib/store";
import { PriceFetchProgressBar } from "./PriceFetchProgressBar";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("PriceFetchProgressBar (MKT-180)", () => {
  afterEach(() => {
    useAppStore.setState({ priceFetch: { active: false, done: 0, total: 0 } });
  });

  it("renders nothing when no fetch is active", () => {
    useAppStore.setState({ priceFetch: { active: false, done: 0, total: 0 } });
    const { container } = render(<PriceFetchProgressBar />);
    expect(container.querySelector("#price-fetch-progress")).toBeNull();
  });

  it("renders the determinate bar with the current percentage", () => {
    useAppStore.setState({ priceFetch: { active: true, done: 3, total: 4 } });
    const { container } = render(<PriceFetchProgressBar />);
    const bar = container.querySelector("#price-fetch-progress");
    expect(bar).not.toBeNull();
    expect(bar?.getAttribute("aria-valuenow")).toBe("75");
    expect(bar?.getAttribute("role")).toBe("progressbar");
  });

  it("renders 0% at task start without dividing by zero", () => {
    useAppStore.setState({ priceFetch: { active: true, done: 0, total: 0 } });
    const { container } = render(<PriceFetchProgressBar />);
    expect(container.querySelector("#price-fetch-progress")?.getAttribute("aria-valuenow")).toBe(
      "0",
    );
  });
});
