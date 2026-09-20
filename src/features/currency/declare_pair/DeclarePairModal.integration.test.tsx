import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as gateway from "../gateway";

vi.mock("../gateway");

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

const { DeclarePairModal } = await import("./DeclarePairModal");

describe("DeclarePairModal (FXR-054/055)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-055 — submit disabled when from or to is empty
  it("submit button is disabled when fromCurrency is empty (FXR-055)", () => {
    render(<DeclarePairModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} />);

    const submitButton = screen.getByTestId("declare-pair-submit");
    expect(submitButton).toBeDisabled();
  });

  it("submit button is disabled when fromCurrency equals toCurrency (FXR-055/023)", async () => {
    render(<DeclarePairModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} />);

    await userEvent.type(screen.getByTestId("declare-pair-from"), "EUR");
    await userEvent.type(screen.getByTestId("declare-pair-to"), "EUR");

    expect(screen.getByTestId("declare-pair-submit")).toBeDisabled();
  });

  // FXR-054 — calling declareCurrencyPair with from/to on submit
  it("calls declareCurrencyPair with fromCurrency and toCurrency on submit (FXR-054)", async () => {
    vi.mocked(gateway.declareCurrencyPair).mockResolvedValue({
      status: "ok",
      data: { from_currency: "USD", to_currency: "EUR" },
    });
    const onSuccess = vi.fn();

    render(<DeclarePairModal isOpen onClose={vi.fn()} onSuccess={onSuccess} />);

    await userEvent.type(screen.getByTestId("declare-pair-from"), "USD");
    await userEvent.type(screen.getByTestId("declare-pair-to"), "EUR");
    await userEvent.click(screen.getByTestId("declare-pair-submit"));

    expect(gateway.declareCurrencyPair).toHaveBeenCalledWith("USD", "EUR");
  });

  // FXR-054 — idempotent success: existing pair returned, not duplicated; onSuccess called
  it("calls onSuccess after declareCurrencyPair succeeds (FXR-054)", async () => {
    vi.mocked(gateway.declareCurrencyPair).mockResolvedValue({
      status: "ok",
      data: { from_currency: "USD", to_currency: "EUR" },
    });
    const onSuccess = vi.fn();

    render(<DeclarePairModal isOpen onClose={vi.fn()} onSuccess={onSuccess} />);

    await userEvent.type(screen.getByTestId("declare-pair-from"), "USD");
    await userEvent.type(screen.getByTestId("declare-pair-to"), "EUR");
    await userEvent.click(screen.getByTestId("declare-pair-submit"));

    expect(await screen.findByTestId("declare-pair-submit")).toBeDefined();
    expect(onSuccess).toHaveBeenCalled();
  });

  // FXR-029 / FXR-023 — inline error on InvalidCurrency; form stays open
  it("shows inline error and keeps modal open on InvalidCurrency (FXR-029)", async () => {
    vi.mocked(gateway.declareCurrencyPair).mockResolvedValue({
      status: "error",
      error: { code: "InvalidCurrency", currency: "XYZ" },
    });
    const onClose = vi.fn();

    render(<DeclarePairModal isOpen onClose={onClose} onSuccess={vi.fn()} />);

    await userEvent.type(screen.getByTestId("declare-pair-from"), "USD");
    await userEvent.type(screen.getByTestId("declare-pair-to"), "XYZ");
    await userEvent.click(screen.getByTestId("declare-pair-submit"));

    expect(await screen.findByTestId("declare-pair-error")).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  // FXR-027 — in-flight: submit button disabled during request
  it("disables submit while declareCurrencyPair is in flight (FXR-027)", async () => {
    let resolveGateway!: (v: unknown) => void;
    vi.mocked(gateway.declareCurrencyPair).mockReturnValue(
      new Promise((r) => {
        resolveGateway = r as typeof resolveGateway;
      }) as ReturnType<typeof gateway.declareCurrencyPair>,
    );

    render(<DeclarePairModal isOpen onClose={vi.fn()} onSuccess={vi.fn()} />);

    await userEvent.type(screen.getByTestId("declare-pair-from"), "USD");
    await userEvent.type(screen.getByTestId("declare-pair-to"), "EUR");
    await userEvent.click(screen.getByTestId("declare-pair-submit"));

    expect(screen.getByTestId("declare-pair-submit")).toBeDisabled();

    resolveGateway({ status: "ok", data: { from_currency: "USD", to_currency: "EUR" } });
  });
});
