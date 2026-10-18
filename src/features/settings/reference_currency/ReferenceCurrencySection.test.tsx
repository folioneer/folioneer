import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ReferenceCurrencySection } from "./ReferenceCurrencySection";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

const state = vi.hoisted(() => ({
  currency: "CHF" as string | null,
  choices: ["CHF", "EUR"],
  isFetchingHistory: false,
  isBusy: false,
  error: null as { key: string } | null,
  choose: vi.fn(),
}));
vi.mock("./useReferenceCurrency", () => ({ useReferenceCurrency: () => state }));

describe("ReferenceCurrencySection (FXR-129, FXR-130)", () => {
  it("offers each currency by its code and name, the chosen one selected", () => {
    render(<ReferenceCurrencySection />);
    const select = document.getElementById("settings-reference-currency") as HTMLSelectElement;
    expect(select.value).toBe("CHF");
    expect(screen.getByRole("option", { name: /^CHF — Swiss franc$/i })).toBeInTheDocument();
    expect(screen.getByText("settings.reference_currency_help")).toBeInTheDocument();
    expect(document.getElementById("settings-reference-currency-fetching")).toBeNull();
  });

  it("says while the rate histories are fetched", () => {
    state.isFetchingHistory = true;
    render(<ReferenceCurrencySection />);
    expect(document.getElementById("settings-reference-currency-fetching")).not.toBeNull();
    state.isFetchingHistory = false;
  });
});
