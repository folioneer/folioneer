import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CurrencyRateEditMount } from "./CurrencyRateEditMount";

const navigateMock = vi.fn();
let searchValue: Record<string, unknown> = {};

vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => navigateMock,
  useSearch: () => searchValue,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

// Stub the RecordRateModal from the currency feature
vi.mock("@/features/currency/record_rate/RecordRateModal", () => ({
  RecordRateModal: ({
    isOpen,
    fromCurrency,
    toCurrency,
    onClose,
  }: {
    isOpen: boolean;
    fromCurrency: string;
    toCurrency: string;
    onClose: () => void;
    onSuccess: () => void;
  }) =>
    isOpen ? (
      <div data-testid="record-rate-modal-stub" data-from={fromCurrency} data-to={toCurrency}>
        <button type="button" data-testid="stub-close-button" onClick={onClose}>
          close
        </button>
      </div>
    ) : null,
}));

describe("CurrencyRateEditMount (FXR-012)", () => {
  beforeEach(() => {
    navigateMock.mockClear();
    searchValue = {};
  });

  // Renders nothing when modal param is absent
  it("renders nothing when modal param is absent", () => {
    searchValue = {};
    render(<CurrencyRateEditMount />);
    expect(screen.queryByTestId("record-rate-modal-stub")).toBeNull();
  });

  // Renders nothing when modal param is not 'record-fx-rate'
  it("renders nothing when modal param is not 'record-fx-rate'", () => {
    searchValue = { modal: "edit-asset", fxFrom: "USD", fxTo: "EUR" };
    render(<CurrencyRateEditMount />);
    expect(screen.queryByTestId("record-rate-modal-stub")).toBeNull();
  });

  // Renders nothing when fxFrom or fxTo is absent
  it("renders nothing when fxFrom is absent", () => {
    searchValue = { modal: "record-fx-rate", fxTo: "EUR" };
    render(<CurrencyRateEditMount />);
    expect(screen.queryByTestId("record-rate-modal-stub")).toBeNull();
  });

  it("renders nothing when fxTo is absent", () => {
    searchValue = { modal: "record-fx-rate", fxFrom: "USD" };
    render(<CurrencyRateEditMount />);
    expect(screen.queryByTestId("record-rate-modal-stub")).toBeNull();
  });

  // FXR-012 — mounts RecordRateModal pre-filled with fxFrom/fxTo
  it("mounts RecordRateModal pre-filled with fxFrom and fxTo when params present (FXR-012)", () => {
    searchValue = { modal: "record-fx-rate", fxFrom: "USD", fxTo: "EUR" };
    render(<CurrencyRateEditMount />);

    const modal = screen.getByTestId("record-rate-modal-stub");
    expect(modal.dataset.from).toBe("USD");
    expect(modal.dataset.to).toBe("EUR");
  });

  // Close clears modal/fxFrom/fxTo params via navigate-replace
  it("close clears modal/fxFrom/fxTo via navigate-replace", () => {
    searchValue = { modal: "record-fx-rate", fxFrom: "USD", fxTo: "EUR" };
    render(<CurrencyRateEditMount />);

    fireEvent.click(screen.getByTestId("stub-close-button"));

    expect(navigateMock).toHaveBeenCalledTimes(1);
    const call = navigateMock.mock.calls[0];
    if (!call) throw new Error("expected navigate to be called");
    const arg = call[0] as { search: (prev: object) => object; replace: boolean };
    expect(arg.replace).toBe(true);
    expect(arg.search({ existing: "kept" })).toMatchObject({
      existing: "kept",
      modal: undefined,
      fxFrom: undefined,
      fxTo: undefined,
    });
  });
});
