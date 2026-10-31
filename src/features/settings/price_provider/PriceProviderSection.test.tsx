import { fireEvent, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PriceProviderSection } from "./PriceProviderSection";
import type { UsePriceProviderResult } from "./usePriceProvider";

const { usePriceProvider } = vi.hoisted(() => ({ usePriceProvider: vi.fn() }));

vi.mock("./usePriceProvider", () => ({ PROVIDER_OFF: "off", usePriceProvider }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const YAHOO = "yahoo_finance";
const state = (overrides: Partial<UsePriceProviderResult> = {}): UsePriceProviderResult => ({
  choice: { chosen: null, offered: [YAHOO] },
  selected: "off",
  awaitingAcknowledgement: null,
  isLoading: false,
  isSaving: false,
  error: null,
  select: vi.fn(),
  acknowledge: vi.fn(),
  ...overrides,
});
const element = (id: string) => document.getElementById(id);

describe("PriceProviderSection (MKT-215)", () => {
  beforeEach(() => usePriceProvider.mockReturnValue(state()));

  it("offers Off and each provider the build offers, with no notice", () => {
    render(<PriceProviderSection />);

    const values = Array.from(
      (element("settings-price-provider") as HTMLSelectElement).options,
      (option) => option.value,
    );
    expect(values).toEqual(["off", YAHOO]);
    expect(element("settings-price-provider-notice")).toBeNull();
  });

  it("choosing in the list selects", () => {
    const current = state();
    usePriceProvider.mockReturnValue(current);
    render(<PriceProviderSection />);

    fireEvent.change(element("settings-price-provider") as HTMLElement, {
      target: { value: YAHOO },
    });

    expect(current.select).toHaveBeenCalledWith(YAHOO);
  });

  it("shows the notice and acknowledges through its checkbox", () => {
    const current = state({ selected: YAHOO, awaitingAcknowledgement: YAHOO });
    usePriceProvider.mockReturnValue(current);
    render(<PriceProviderSection />);

    expect(element("settings-price-provider-notice")).toHaveTextContent(
      `settings.price_provider_notice.${YAHOO}`,
    );
    const understood = element("settings-price-provider-understood") as HTMLInputElement;
    expect(understood.checked).toBe(false);

    fireEvent.click(understood);
    expect(current.acknowledge).toHaveBeenCalledTimes(1);
  });

  it("with a source on, recalls what it is instead of asking", () => {
    usePriceProvider.mockReturnValue(
      state({ choice: { chosen: YAHOO, offered: [YAHOO] }, selected: YAHOO }),
    );
    render(<PriceProviderSection />);

    expect(element("settings-price-provider-note")).toHaveTextContent(
      `settings.price_provider_note.${YAHOO}`,
    );
    expect(element("settings-price-provider-understood")).toBeNull();
  });

  it("says it is saving, and locks the list", () => {
    usePriceProvider.mockReturnValue(state({ isSaving: true }));
    render(<PriceProviderSection />);

    expect(element("settings-price-provider-pending")).toHaveTextContent(
      "settings.price_provider_saving",
    );
    expect(element("settings-price-provider")).toBeDisabled();
  });

  it("locks the checkbox while saving", () => {
    usePriceProvider.mockReturnValue(
      state({ selected: YAHOO, awaitingAcknowledgement: YAHOO, isSaving: true }),
    );
    render(<PriceProviderSection />);

    expect(element("settings-price-provider-understood")).toBeDisabled();
  });

  it("shows only the error when the choice cannot be read", () => {
    usePriceProvider.mockReturnValue(
      state({ choice: null, error: { key: "settings.price_provider_load_failed" } }),
    );
    render(<PriceProviderSection />);

    expect(element("settings-price-provider")).toBeNull();
    expect(element("settings-price-provider-error")).toHaveTextContent(
      "settings.price_provider_load_failed",
    );
  });

  it("shows a refusal", () => {
    usePriceProvider.mockReturnValue(state({ error: { key: "error.price_provider.NotSaved" } }));
    render(<PriceProviderSection />);

    expect(element("settings-price-provider-error")).toHaveTextContent(
      "error.price_provider.NotSaved",
    );
  });
});
