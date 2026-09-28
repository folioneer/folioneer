import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Account, Asset } from "@/bindings";
import { useAppStore } from "@/lib/store";
import { AddTransactionModal } from "./AddTransactionModal";

// CSH-018 / TRX-064 — the asset combobox offers the core's list of non-cash assets, so the
// user cannot record a purchase against a system Cash Asset (Deposit/Withdrawal owns that
// flow). Mock ComboboxField so we can inspect the items prop it receives.
vi.mock("../shared/useNonCashAssets", () => ({
  // TRX-064 — the core's list: every asset but the Cash Assets.
  useNonCashAssets: () => [
    { id: "asset-stock-1", name: "Apple", class: "Stocks", is_archived: false, currency: "USD" },
    { id: "asset-bond-1", name: "Bond", class: "Bonds", is_archived: false, currency: "EUR" },
  ],
}));

vi.mock("@/ui/components/field/ComboboxField", () => ({
  ComboboxField: ({ id, items }: { id: string; items: { id: string; name: string }[] }) => (
    <div data-testid={`combobox-${id}`} data-item-ids={items.map((i) => i.id).join(",")} />
  ),
}));

vi.mock("./useAddTransaction", () => ({
  useAddTransaction: vi.fn(() => ({
    formData: {
      assetId: "",
      accountId: "",
      date: "",
      quantity: "",
      unitPrice: "",
      exchangeRate: "",
      fees: "",
      note: "",
    },
    totalAmountDisplay: "",
    error: null,
    isSubmitting: false,
    isFormValid: false,
    showArchivedConfirm: false,
    recordPrice: false,
    setRecordPrice: vi.fn(),
    handleChange: vi.fn(),
    handleSubmit: vi.fn(),
    handleConfirmArchived: vi.fn(),
    handleCancelArchived: vi.fn(),
  })),
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

describe("AddTransactionModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      assets: [
        {
          id: "asset-stock-1",
          name: "Apple",
          class: "Stocks",
          is_archived: false,
          currency: "USD",
        },
        { id: "asset-bond-1", name: "Bond", class: "Bonds", is_archived: false, currency: "EUR" },
        {
          id: "system-cash-eur",
          name: "Cash EUR",
          class: "Cash",
          is_archived: false,
          currency: "EUR",
        },
      ] as Asset[],
      accounts: [{ id: "account-1", name: "My Account", currency: "EUR" }] as Account[],
    });
  });

  // CSH-018 — Cash Assets are filtered out of the asset combobox.
  // CSH-018 / TRX-064 — the combobox offers the core's list of non-cash assets as is
  it("offers the core's list of non-cash assets", () => {
    render(<AddTransactionModal isOpen onClose={() => {}} prefillAccountId="account-1" />);
    const combobox = screen.getByTestId("combobox-trx-asset");
    const itemIds = combobox.getAttribute("data-item-ids")?.split(",") ?? [];
    expect(itemIds).toEqual(["asset-stock-1", "asset-bond-1"]);
  });
});
