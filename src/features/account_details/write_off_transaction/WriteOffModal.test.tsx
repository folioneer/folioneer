import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WriteOffModal } from "./WriteOffModal";

const { mockUseWriteOff } = vi.hoisted(() => ({ mockUseWriteOff: vi.fn() }));

vi.mock("./useWriteOff", () => ({
  WRITE_OFF_REASONS: ["NetworkFee", "Dust", "Lost", "Other"],
  useWriteOff: (...args: unknown[]) => mockUseWriteOff(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() } }));

const target = {
  assetId: "asset-btc",
  assetName: "Bitcoin",
  assetReference: "BTC",
  quantityMicro: 500_000,
  averagePriceMicro: 20_000_000_000,
};

const hookReturn = (over: Record<string, unknown> = {}) => ({
  formData: { date: "2024-06-15", reason: "NetworkFee", quantity: "0.1", note: "" },
  refusal: null,
  isSubmitting: false,
  isFormValid: true,
  heldMicro: 500_000,
  overHolding: false,
  estimatedCostMicro: 2_000_000_000,
  handleChange: vi.fn(),
  handleSubmit: vi.fn((e: Event) => e.preventDefault()),
  ...over,
});

const renderModal = () =>
  render(
    <WriteOffModal
      isOpen
      onClose={vi.fn()}
      accountId="acc-1"
      target={target}
      onSubmitSuccess={vi.fn()}
    />,
  );

describe("WriteOffModal (WOF-051/052/053/055)", () => {
  beforeEach(() => mockUseWriteOff.mockReset());

  // WOF-052 — the estimate line; for Lost, the one saying it is shown apart.
  it("gives the estimate, worded for a lost write-off", () => {
    mockUseWriteOff.mockReturnValue(hookReturn());
    const view = renderModal();
    expect(document.querySelector("#write-off-estimate")?.textContent).toBe("write_off.estimate");
    view.unmount();
    mockUseWriteOff.mockReturnValue(
      hookReturn({
        formData: { date: "2024-06-15", reason: "Lost", quantity: "0.1", note: "" },
      }),
    );
    renderModal();
    expect(document.querySelector("#write-off-estimate")?.textContent).toBe(
      "write_off.estimate_lost",
    );
  });

  // WOF-053 — over the quantity held: refused under the field, the action inactive.
  it("refuses a quantity above the quantity held under the field", () => {
    mockUseWriteOff.mockReturnValue(
      hookReturn({ overHolding: true, isFormValid: false, estimatedCostMicro: null }),
    );
    renderModal();
    expect(document.querySelector("#write-off-quantity-error")?.textContent).toBe(
      "write_off.error_over_holding",
    );
    expect(document.querySelector("#write-off-estimate")).toBeNull();
    expect(document.querySelector("#write-off-submit")).toBeDisabled();
  });

  // WOF-055 — a refusal below the form keeps the action inactive.
  it("shows a refusal below the form and keeps the action inactive", () => {
    mockUseWriteOff.mockReturnValue(
      hookReturn({ refusal: { message: { key: "write_off.error_archived" }, onQuantity: false } }),
    );
    renderModal();
    expect(screen.getByRole("alert")).toHaveTextContent("write_off.error_archived");
    expect(document.querySelector("#write-off-submit")).toBeDisabled();
  });

  it("submits the form and passes field changes to the hook", () => {
    const handleChange = vi.fn();
    const handleSubmit = vi.fn((e: Event) => e.preventDefault());
    mockUseWriteOff.mockReturnValue(hookReturn({ handleChange, handleSubmit }));
    renderModal();
    fireEvent.change(document.querySelector("#write-off-reason") as Element, {
      target: { value: "Dust" },
    });
    expect(handleChange).toHaveBeenCalledWith("reason", "Dust");
    fireEvent.submit(document.querySelector("#write-off-form") as Element);
    expect(handleSubmit).toHaveBeenCalled();
  });
});
