import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as gateway from "../gateway";

vi.mock("../gateway");

// DateField stores ISO (YYYY-MM-DD) but renders + parses the locale display
// format. With the mocked i18n language pinned to "en", the effective locale is
// en-US (M/D/YYYY). Driving the field through a single `change` event with the
// display string avoids the controlled-sync churn that per-keystroke `type`
// would trigger. `2026-06-01` (ISO) ↔ `06/01/2026` (en-US display).
const setDate = (displayValue: string) =>
  fireEvent.change(screen.getByTestId("record-rate-date"), { target: { value: displayValue } });

const mockShowSnackbar = vi.hoisted(() => vi.fn());
vi.mock("@/ui/components/snackbar/snackbarStore", () => ({
  useSnackbar: () => mockShowSnackbar,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

const { RecordRateModal } = await import("./RecordRateModal");

// ---------------------------------------------------------------------------
// Create mode (no initialRate prop)
// ---------------------------------------------------------------------------

describe("RecordRateModal — create mode (FXR-025/027/028/029)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FXR-027 — in-flight: submit disabled while request is in flight
  it("disables submit while recordCurrencyRate is in flight (FXR-027)", async () => {
    let resolveGateway!: (v: unknown) => void;
    vi.mocked(gateway.recordCurrencyRate).mockReturnValue(
      new Promise((r) => {
        resolveGateway = r as typeof resolveGateway;
      }) as ReturnType<typeof gateway.recordCurrencyRate>,
    );

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    setDate("06/01/2026");
    await userEvent.type(screen.getByTestId("record-rate-rate"), "0.92");
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(screen.getByTestId("record-rate-submit")).toBeDisabled();

    resolveGateway({
      status: "ok",
      data: {
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
        rate: 920_000,
        source: "Manual",
      },
    });
  });

  // FXR-028 — success: snackbar shown, onSuccess called, modal closes
  it("calls onSuccess and shows snackbar after successful recordCurrencyRate (FXR-028)", async () => {
    vi.mocked(gateway.recordCurrencyRate).mockResolvedValue({
      status: "ok",
      data: {
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
        rate: 920_000,
        source: "Manual",
      },
    });
    const onSuccess = vi.fn();

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={onSuccess}
      />,
    );

    setDate("06/01/2026");
    await userEvent.type(screen.getByTestId("record-rate-rate"), "0.92");
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(await screen.findByTestId("record-rate-submit")).toBeDefined();
    expect(onSuccess).toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalled();
  });

  // FXR-029 — error: inline error shown, modal stays open
  it("renders inline error and keeps modal open on NotPositive error (FXR-029)", async () => {
    vi.mocked(gateway.recordCurrencyRate).mockResolvedValue({
      status: "error",
      error: { code: "NotPositive" },
    });
    const onClose = vi.fn();

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={onClose}
        onSuccess={vi.fn()}
      />,
    );

    setDate("06/01/2026");
    await userEvent.type(screen.getByTestId("record-rate-rate"), "0");
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(await screen.findByTestId("record-rate-error")).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  // FXR-029 — DateInFuture inline error
  it("renders inline error on DateInFuture (FXR-029)", async () => {
    vi.mocked(gateway.recordCurrencyRate).mockResolvedValue({
      status: "error",
      error: { code: "DateInFuture" },
    });

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    setDate("12/31/2099");
    await userEvent.type(screen.getByTestId("record-rate-rate"), "0.92");
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(await screen.findByTestId("record-rate-error")).toBeInTheDocument();
  });

  // FXR-021 — inline non-blocking hint renders when the typed rate is invalid
  it("renders an inline rate hint when the typed rate is not positive (FXR-021)", async () => {
    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    await userEvent.type(screen.getByTestId("record-rate-rate"), "0");

    expect(screen.getByTestId("record-rate-rate-hint")).toBeInTheDocument();
  });

  // FXR-022 — inline non-blocking hint renders for a future date. DateField only
  // ever emits a well-formed ISO date (or ""), so the malformed-date branch is no
  // longer reachable through the field; a future date still trips the hint.
  it("renders an inline date hint when the selected date is in the future (FXR-022)", () => {
    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    setDate("12/31/2099");

    expect(screen.getByTestId("record-rate-date-hint")).toBeInTheDocument();
  });

  // FXR-021/029 — the inline hint does NOT block submit: gateway is still called
  it("still calls the gateway on submit even when an inline hint is showing (FXR-029)", async () => {
    vi.mocked(gateway.recordCurrencyRate).mockResolvedValue({
      status: "error",
      error: { code: "NotPositive" },
    });

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    setDate("06/01/2026");
    await userEvent.type(screen.getByTestId("record-rate-rate"), "0");
    // Inline hint visible, but submit must still round-trip to the gateway.
    expect(screen.getByTestId("record-rate-rate-hint")).toBeInTheDocument();
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(gateway.recordCurrencyRate).toHaveBeenCalledWith("USD", "EUR", "2026-06-01", 0);
  });

  // gateway call — recordCurrencyRate invoked with correct positional args (F27)
  it("calls recordCurrencyRate with fromCurrency, toCurrency, date, rate as positional args", async () => {
    vi.mocked(gateway.recordCurrencyRate).mockResolvedValue({
      status: "ok",
      data: {
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
        rate: 920_000,
        source: "Manual",
      },
    });

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    setDate("06/01/2026");
    await userEvent.type(screen.getByTestId("record-rate-rate"), "0.92");
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    await screen.findByTestId("record-rate-submit");
    expect(gateway.recordCurrencyRate).toHaveBeenCalledWith("USD", "EUR", "2026-06-01", 0.92);
  });
});

// ---------------------------------------------------------------------------
// Edit mode (initialRate prop provided — FXR-052)
// ---------------------------------------------------------------------------

describe("RecordRateModal — edit mode (FXR-052)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const initialRate = {
    from_currency: "USD",
    to_currency: "EUR",
    date: "2026-06-01",
    rate: 920_000,
    source: "Manual" as const,
  };

  // FXR-052 — edit mode pre-fills from/to/date/rate from initialRate
  it("pre-fills date and rate fields from initialRate in edit mode (FXR-052)", () => {
    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        initialRate={initialRate}
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    // DateField renders the stored ISO date in the locale display format (en-US).
    expect((screen.getByTestId("record-rate-date") as HTMLInputElement).value).toBe("6/1/2026");
    // Rate shown as human-readable decimal pre-fill
    expect((screen.getByTestId("record-rate-rate") as HTMLInputElement).value).not.toBe("");
  });

  // FXR-052 — edit mode calls updateCurrencyRate (not recordCurrencyRate)
  it("calls updateCurrencyRate with originalDate when in edit mode (FXR-052)", async () => {
    vi.mocked(gateway.updateCurrencyRate).mockResolvedValue({ status: "ok", data: null });

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        initialRate={initialRate}
        onClose={vi.fn()}
        onSuccess={vi.fn()}
      />,
    );

    // Change the date to trigger a new ISO value (en-US display → ISO 2026-06-02)
    setDate("06/02/2026");
    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(gateway.updateCurrencyRate).toHaveBeenCalledWith(
      "USD",
      "EUR",
      "2026-06-01",
      "2026-06-02",
      expect.any(Number),
    );
    expect(gateway.recordCurrencyRate).not.toHaveBeenCalled();
  });

  // FXR-028 — edit success: onSuccess called, snackbar shown
  it("calls onSuccess and shows snackbar after successful updateCurrencyRate (FXR-028)", async () => {
    vi.mocked(gateway.updateCurrencyRate).mockResolvedValue({ status: "ok", data: null });
    const onSuccess = vi.fn();

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        initialRate={initialRate}
        onClose={vi.fn()}
        onSuccess={onSuccess}
      />,
    );

    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(await screen.findByTestId("record-rate-submit")).toBeDefined();
    expect(onSuccess).toHaveBeenCalled();
    expect(mockShowSnackbar).toHaveBeenCalled();
  });

  // FXR-052/029 — edit RateNotFound: inline error, modal stays open
  it("renders inline RateNotFound error and keeps modal open (FXR-052/029)", async () => {
    vi.mocked(gateway.updateCurrencyRate).mockResolvedValue({
      status: "error",
      error: {
        code: "RateNotFound",
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
      },
    });
    const onClose = vi.fn();

    render(
      <RecordRateModal
        isOpen
        fromCurrency="USD"
        toCurrency="EUR"
        initialRate={initialRate}
        onClose={onClose}
        onSuccess={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByTestId("record-rate-submit"));

    expect(await screen.findByTestId("record-rate-error")).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Delete confirmation (FXR-053)
// ---------------------------------------------------------------------------

const { DeleteRateConfirmation } = await import("./RecordRateModal");

describe("DeleteRateConfirmation (FXR-053)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const rateToDelete = {
    from_currency: "USD",
    to_currency: "EUR",
    date: "2026-06-01",
    rate: 920_000,
    source: "Manual" as const,
  };

  // FXR-053 — calls deleteCurrencyRate with correct args
  it("calls deleteCurrencyRate with fromCurrency, toCurrency, date on confirm (FXR-053)", async () => {
    vi.mocked(gateway.deleteCurrencyRate).mockResolvedValue({ status: "ok", data: null });
    const onSuccess = vi.fn();

    render(
      <DeleteRateConfirmation isOpen rate={rateToDelete} onClose={vi.fn()} onSuccess={onSuccess} />,
    );

    await userEvent.click(screen.getByTestId("delete-rate-confirm"));

    expect(gateway.deleteCurrencyRate).toHaveBeenCalledWith("USD", "EUR", "2026-06-01");
    expect(onSuccess).toHaveBeenCalled();
  });

  // FXR-053 — RateNotFound inline error, dialog stays open
  it("renders inline error on RateNotFound and keeps dialog open (FXR-053)", async () => {
    vi.mocked(gateway.deleteCurrencyRate).mockResolvedValue({
      status: "error",
      error: {
        code: "RateNotFound",
        from_currency: "USD",
        to_currency: "EUR",
        date: "2026-06-01",
      },
    });
    const onClose = vi.fn();

    render(
      <DeleteRateConfirmation isOpen rate={rateToDelete} onClose={onClose} onSuccess={vi.fn()} />,
    );

    await userEvent.click(screen.getByTestId("delete-rate-confirm"));

    expect(await screen.findByTestId("delete-rate-error")).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  // Cancel — onClose called without calling gateway
  it("calls onClose without calling deleteCurrencyRate when cancel is clicked", async () => {
    const onClose = vi.fn();

    render(
      <DeleteRateConfirmation isOpen rate={rateToDelete} onClose={onClose} onSuccess={vi.fn()} />,
    );

    await userEvent.click(screen.getByTestId("delete-rate-cancel"));

    expect(gateway.deleteCurrencyRate).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
