import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TransferModal } from "./TransferModal";

const { mockUseTransfer } = vi.hoisted(() => ({ mockUseTransfer: vi.fn() }));

vi.mock("./useTransfer", () => ({
  useTransfer: (...args: unknown[]) => mockUseTransfer(...args),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: "en" } }),
}));

vi.mock("@/lib/logger", () => ({ logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() } }));

const target = {
  assetId: "asset-btc",
  assetName: "Bitcoin",
  assetReference: "BTC",
  heldQuantity: "0.5",
  averagePriceMicro: 20_000_000_000,
  accountName: "Exchange",
  accountCurrency: "EUR",
};

const destinations = [
  { id: "acc-wallet", name: "Wallet", currency: "EUR" },
  { id: "acc-usd", name: "Broker", currency: "USD" },
];

const hookReturn = (over: Record<string, unknown> = {}) => ({
  formData: {
    date: "2024-06-15",
    toAccountId: "acc-wallet",
    sent: "0.2",
    received: "0.19",
    rate: "",
    note: "",
  },
  destination: destinations[0],
  betweenCurrencies: false,
  storedRate: "loading",
  refusal: null,
  isSubmitting: false,
  isFormValid: true,
  heldQuantity: "0.5",
  overHolding: false,
  receivedAboveSent: false,
  summary: {
    costMicro: 3_800_000_000,
    convertedMicro: null,
    rateMicro: 1_000_000,
    fee: "0.01",
    feeCostMicro: 200_000_000,
  },
  handleChange: vi.fn(),
  handleSubmit: vi.fn((e: Event) => e.preventDefault()),
  ...over,
});

const renderModal = () =>
  render(
    <TransferModal
      isOpen
      onClose={vi.fn()}
      accountId="acc-exchange"
      target={target}
      destinations={destinations}
      onSubmitSuccess={vi.fn()}
    />,
  );

describe("TransferModal (TRF-051 to TRF-055)", () => {
  beforeEach(() => mockUseTransfer.mockReset());

  // TRF-051 — the other accounts to choose from, an empty choice first while none is chosen.
  it("offers the other accounts, none chosen by default", () => {
    mockUseTransfer.mockReturnValue(
      hookReturn({
        formData: { ...hookReturn().formData, toAccountId: "" },
        destination: null,
        summary: null,
      }),
    );
    renderModal();
    const options = [...document.querySelectorAll("#transfer-to-account option")];
    expect(options.map((o) => o.getAttribute("value"))).toEqual(["", "acc-wallet", "acc-usd"]);
    expect(document.querySelector("#transfer-rate")).toBeNull();
  });

  // TRF-052 / TRF-053 — within one currency: no rate; the summary and the fee line.
  it("sums up the transfer and its fee without a rate within one currency", () => {
    mockUseTransfer.mockReturnValue(hookReturn());
    renderModal();
    expect(document.querySelector("#transfer-rate")).toBeNull();
    expect(document.querySelector("#transfer-summary")?.textContent).toContain("transfer.summary");
    expect(document.querySelector("#transfer-summary-fee")?.textContent).toBe(
      "transfer.summary_fee",
    );
  });

  // TRF-052 — between currencies: the rate field, and the line saying where it comes from.
  it("asks the rate between currencies and says whether one is stored", () => {
    const usd = { betweenCurrencies: true, destination: destinations[1] };
    mockUseTransfer.mockReturnValue(hookReturn({ ...usd, storedRate: "stored" }));
    const view = renderModal();
    expect(document.querySelector("#transfer-rate")).not.toBeNull();
    expect(document.querySelector("#transfer-rate-hint")?.textContent).toBe("transfer.rate_stored");
    view.unmount();
    mockUseTransfer.mockReturnValue(hookReturn({ ...usd, storedRate: "none" }));
    const none = renderModal();
    expect(document.querySelector("#transfer-rate-hint")?.textContent).toBe("transfer.rate_none");
    none.unmount();
    mockUseTransfer.mockReturnValue(hookReturn({ ...usd, storedRate: "typed" }));
    renderModal();
    expect(document.querySelector("#transfer-rate-hint")).toBeNull();
  });

  // TRF-054 — figures refused under their fields; the action inactive.
  it("refuses the quantities under their fields", () => {
    mockUseTransfer.mockReturnValue(
      hookReturn({
        overHolding: true,
        receivedAboveSent: true,
        isFormValid: false,
        summary: null,
      }),
    );
    renderModal();
    expect(document.querySelector("#transfer-sent-error")?.textContent).toBe(
      "transfer.error_over_holding",
    );
    expect(document.querySelector("#transfer-received-error")?.textContent).toBe(
      "transfer.error_received_above_sent",
    );
    expect(document.querySelector("#transfer-summary")).toBeNull();
    expect(document.querySelector("#transfer-submit")).toBeDisabled();
  });

  // TRF-055 — a lack of units under the quantity sent; another refusal below the form.
  it("places a refusal under the quantity sent or below the form", () => {
    mockUseTransfer.mockReturnValue(
      hookReturn({ refusal: { message: { key: "error.Oversell" }, onQuantity: true } }),
    );
    const view = renderModal();
    expect(document.querySelector("#transfer-sent-error")?.textContent).toBe("error.Oversell");
    expect(screen.queryByRole("alert")).toBeNull();
    view.unmount();
    mockUseTransfer.mockReturnValue(
      hookReturn({
        refusal: { message: { key: "transfer.error_archived" }, onQuantity: false },
      }),
    );
    renderModal();
    expect(screen.getByRole("alert")).toHaveTextContent("transfer.error_archived");
    expect(document.querySelector("#transfer-submit")).toBeDisabled();
  });
});
