import { fireEvent, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PriceSourceSection } from "./PriceSourceSection";
import type { UsePriceSourceResult } from "./usePriceSource";

const { usePriceSource } = vi.hoisted(() => ({ usePriceSource: vi.fn() }));

vi.mock("./usePriceSource", () => ({ SOURCE_OFF: "off", usePriceSource }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const YAHOO = "yahoo_finance";
const state = (overrides: Partial<UsePriceSourceResult> = {}): UsePriceSourceResult => ({
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

describe("PriceSourceSection (MKT-215)", () => {
  beforeEach(() => usePriceSource.mockReturnValue(state()));

  it("offers Off and each provider the build offers, with no notice", () => {
    render(<PriceSourceSection />);

    const values = Array.from(
      (element("settings-price-source") as HTMLSelectElement).options,
      (option) => option.value,
    );
    expect(values).toEqual(["off", YAHOO]);
    expect(element("settings-price-source-notice")).toBeNull();
  });

  it("choosing in the list selects", () => {
    const current = state();
    usePriceSource.mockReturnValue(current);
    render(<PriceSourceSection />);

    fireEvent.change(element("settings-price-source") as HTMLElement, {
      target: { value: YAHOO },
    });

    expect(current.select).toHaveBeenCalledWith(YAHOO);
  });

  it("shows the notice and acknowledges through its checkbox", () => {
    const current = state({ selected: YAHOO, awaitingAcknowledgement: YAHOO });
    usePriceSource.mockReturnValue(current);
    render(<PriceSourceSection />);

    expect(element("settings-price-source-notice")).toHaveTextContent(
      `settings.price_source_notice.${YAHOO}`,
    );
    const understood = element("settings-price-source-understood") as HTMLInputElement;
    expect(understood.checked).toBe(false);

    fireEvent.click(understood);
    expect(current.acknowledge).toHaveBeenCalledTimes(1);
  });

  it("with a source on, recalls what it is instead of asking", () => {
    usePriceSource.mockReturnValue(
      state({ choice: { chosen: YAHOO, offered: [YAHOO] }, selected: YAHOO }),
    );
    render(<PriceSourceSection />);

    expect(element("settings-price-source-note")).toHaveTextContent(
      `settings.price_source_note.${YAHOO}`,
    );
    expect(element("settings-price-source-understood")).toBeNull();
  });

  it("says it is saving, and locks the list", () => {
    usePriceSource.mockReturnValue(state({ isSaving: true }));
    render(<PriceSourceSection />);

    expect(element("settings-price-source-pending")).toHaveTextContent(
      "settings.price_source_saving",
    );
    expect(element("settings-price-source")).toBeDisabled();
  });

  it("shows a refusal", () => {
    usePriceSource.mockReturnValue(state({ error: { key: "error.price_provider.NotSaved" } }));
    render(<PriceSourceSection />);

    expect(element("settings-price-source-error")).toHaveTextContent(
      "error.price_provider.NotSaved",
    );
  });
});
