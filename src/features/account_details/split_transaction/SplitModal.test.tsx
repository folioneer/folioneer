import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SplitModal } from "./SplitModal";

// ── Mock the hook that drives the component ────────────────────────────────────
const { mockUseSplitTransaction } = vi.hoisted(() => ({
  mockUseSplitTransaction: vi.fn(),
}));

vi.mock("./useSplitTransaction", () => ({
  useSplitTransaction: (...args: unknown[]) => mockUseSplitTransaction(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "en" },
  }),
}));

vi.mock("@/lib/logger", () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

// ── Shared fixtures ────────────────────────────────────────────────────────────
const TODAY = new Date().toISOString().slice(0, 10);

const target = {
  assetId: "asset-equity-1",
  assetName: "Alphabet Inc",
  holdingQuantityMicro: 10_000_000,
  averagePriceMicro: 150_000_000,
  currentPriceMicro: 150_000_000,
};

const makeHookReturn = (overrides: Record<string, unknown> = {}) => ({
  formData: {
    date: TODAY,
    ratioNew: "2",
    ratioOld: "1",
    factor: "",
    note: "",
  },
  preview: {
    oldQuantity: "10",
    oldAveragePrice: "150,00",
    newQuantity: "20",
    newAveragePrice: "75,00",
    newQuantityMicro: 20_000_000,
  },
  collapsesPosition: false,
  ratioError: null,
  error: null,
  isSubmitting: false,
  isFormValid: true,
  isEditMode: false,
  hasCurrentPrice: true,
  recordPrice: true,
  setRecordPrice: vi.fn(),
  priceInput: "75.000",
  handlePriceChange: vi.fn(),
  handleChange: vi.fn(),
  handleSubmit: vi.fn(),
  ...overrides,
});

const BASE_PROPS = {
  isOpen: true,
  onClose: vi.fn(),
  accountId: "account-1",
  target,
  onSubmitSuccess: vi.fn(),
};

// ── Tests ──────────────────────────────────────────────────────────────────────
describe("SplitModal (SPL-061/040)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseSplitTransaction.mockReturnValue(makeHookReturn());
  });

  // SPL-061 — date, ratio pair, price checkbox, and note fields (F25 stable ids)
  it("renders the date, ratio pair, record-price checkbox, price, and note fields", () => {
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.getByTestId("split-trx-date")).toBeInTheDocument();
    expect(screen.getByTestId("split-trx-ratio-new")).toBeInTheDocument();
    expect(screen.getByTestId("split-trx-ratio-old")).toBeInTheDocument();
    expect(screen.getByTestId("split-trx-record-price")).toBeInTheDocument();
    expect(screen.getByTestId("split-trx-price")).toBeInTheDocument();
    expect(screen.getByTestId("split-trx-note")).toBeInTheDocument();
  });

  // SPL-061 — the split asset is fixed by the originating holding row
  it("shows the target asset name", () => {
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.getByText("Alphabet Inc")).toBeInTheDocument();
  });

  // SPL-061 — read-only preview of the rescaled position
  it("renders the preview line when the hook computes one", () => {
    const { container } = render(<SplitModal {...BASE_PROPS} />);
    expect(container.querySelector("#split-trx-preview")).toHaveTextContent("split.preview");
  });

  it("omits the preview line when the hook returns none", () => {
    mockUseSplitTransaction.mockReturnValue(makeHookReturn({ preview: null }));
    const { container } = render(<SplitModal {...BASE_PROPS} />);
    expect(container.querySelector("#split-trx-preview")).toBeNull();
  });

  // SPL-011/021 — submit disabled while the form is invalid
  it("disables the submit button when isFormValid is false", () => {
    mockUseSplitTransaction.mockReturnValue(makeHookReturn({ isFormValid: false }));
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.getByTestId("split-trx-submit")).toBeDisabled();
  });

  it("enables the submit button when isFormValid is true", () => {
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.getByTestId("split-trx-submit")).not.toBeDisabled();
  });

  // SPL-011 — inline ratio rejection rendered as role="alert" (F27)
  it("renders the ratio error as an alert when set", () => {
    mockUseSplitTransaction.mockReturnValue(
      makeHookReturn({
        ratioError: { key: "transaction.error_validation_split_ratio" },
        isFormValid: false,
      }),
    );
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toHaveTextContent("transaction.error_validation_split_ratio");
  });

  // F27 — backend rejection rendered inline as role="alert"
  it("renders the submit error as an alert when set", () => {
    mockUseSplitTransaction.mockReturnValue(
      makeHookReturn({ error: { key: "error.SplitOnCashAsset" } }),
    );
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.getByRole("alert")).toHaveTextContent("error.SplitOnCashAsset");
  });

  // SPL-040 — the price field follows the checkbox
  it("hides the price field when the record-price checkbox is unchecked", () => {
    mockUseSplitTransaction.mockReturnValue(makeHookReturn({ recordPrice: false }));
    render(<SplitModal {...BASE_PROPS} />);
    expect(screen.queryByTestId("split-trx-price")).not.toBeInTheDocument();
  });

  it("toggling the record-price checkbox calls setRecordPrice", async () => {
    const setRecordPrice = vi.fn();
    mockUseSplitTransaction.mockReturnValue(makeHookReturn({ setRecordPrice }));
    render(<SplitModal {...BASE_PROPS} />);
    await userEvent.click(screen.getByTestId("split-trx-record-price"));
    expect(setRecordPrice).toHaveBeenCalledWith(false);
  });

  // UI → hook: ratio fields fire handleChange
  it("fires handleChange for the ratio fields", () => {
    const handleChange = vi.fn();
    mockUseSplitTransaction.mockReturnValue(makeHookReturn({ handleChange }));
    render(<SplitModal {...BASE_PROPS} />);

    fireEvent.change(screen.getByTestId("split-trx-ratio-new"), { target: { value: "3" } });
    fireEvent.change(screen.getByTestId("split-trx-ratio-old"), { target: { value: "2" } });
    expect(handleChange).toHaveBeenCalledWith("ratioNew", "3");
    expect(handleChange).toHaveBeenCalledWith("ratioOld", "2");
  });

  // Form submit routes through the hook (E3 stable form id)
  it("calls handleSubmit when the form is submitted", () => {
    const handleSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    mockUseSplitTransaction.mockReturnValue(makeHookReturn({ handleSubmit }));
    const { container } = render(<SplitModal {...BASE_PROPS} />);

    const form = container.querySelector("#split-trx-form");
    if (!form) throw new Error("expected #split-trx-form to be in the DOM");
    fireEvent.submit(form);

    expect(handleSubmit).toHaveBeenCalled();
  });

  // Cancel button calls onClose
  it("calls onClose when the cancel button is clicked", async () => {
    const onClose = vi.fn();
    render(<SplitModal {...BASE_PROPS} onClose={onClose} />);
    await userEvent.click(screen.getByRole("button", { name: /action\.cancel/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // SPL-030 — edit mode: single factor field, no ratio pair, no price checkbox
  it("renders the factor field and hides ratio + price surfaces in edit mode", () => {
    mockUseSplitTransaction.mockReturnValue(
      makeHookReturn({ isEditMode: true, preview: null, recordPrice: false }),
    );
    render(
      <SplitModal
        {...BASE_PROPS}
        editMode={{
          transactionId: "tx-spl-1",
          lockedAssetId: "asset-equity-1",
          lockedAssetName: "Alphabet Inc",
        }}
      />,
    );
    expect(screen.getByTestId("split-trx-factor")).toBeInTheDocument();
    expect(screen.queryByTestId("split-trx-ratio-new")).not.toBeInTheDocument();
    expect(screen.queryByTestId("split-trx-ratio-old")).not.toBeInTheDocument();
    expect(screen.queryByTestId("split-trx-record-price")).not.toBeInTheDocument();
    expect(screen.queryByTestId("split-trx-price")).not.toBeInTheDocument();
  });
});
